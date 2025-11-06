//! Value set code generation for TypeScript.
//!
//! This module provides utilities for generating TypeScript const arrays and
//! type unions from FHIR ValueSet resources with configurable size limits.

use inkgen_core::{extract_codes_from_valueset, should_generate_valueset};
use serde_json::Value;

/// Information about a value set to be generated as TypeScript code.
#[derive(Debug, Clone)]
pub struct ValueSetInfo {
    /// TypeScript type name (e.g., "AdministrativeGender")
    pub type_name: String,
    /// Canonical URL of the value set
    pub canonical_url: String,
    /// List of codes in the value set
    pub codes: Vec<String>,
    /// Optional display names for each code
    pub displays: Vec<Option<String>>,
    /// Title of the value set
    pub title: Option<String>,
    /// Description of the value set
    pub description: Option<String>,
}

impl ValueSetInfo {
    /// Creates a new ValueSetInfo by extracting codes from a FHIR ValueSet resource.
    ///
    /// # Arguments
    ///
    /// * `value_set_json` - The ValueSet resource as JSON
    /// * `type_name` - The TypeScript type name to use
    /// * `max_codes` - Optional maximum number of codes allowed for generation
    ///
    /// # Returns
    ///
    /// Result containing the ValueSetInfo with extracted codes, or None if the value set
    /// exceeds the maximum size or has no codes
    pub fn from_valueset(
        value_set_json: &Value,
        type_name: String,
        max_codes: Option<usize>,
    ) -> Result<Option<Self>, String> {
        // Extract ALL codes first (no limit) to check total count
        let resolved = extract_codes_from_valueset(value_set_json, None)
            .map_err(|e| format!("Failed to extract codes: {}", e))?;

        // Check if we should generate based on code count and max limit
        if let Some(max) = max_codes {
            if !should_generate_valueset(resolved.codes.len(), max) {
                return Ok(None);
            }
        } else if resolved.codes.is_empty() {
            return Ok(None);
        }

        let displays: Vec<Option<String>> = resolved
            .codes
            .iter()
            .map(|code| resolved.displays.get(code).cloned())
            .collect();

        Ok(Some(Self {
            type_name,
            canonical_url: resolved.url,
            codes: resolved.codes,
            displays,
            title: resolved.title,
            description: resolved.description,
        }))
    }

    /// Generates TypeScript code for this value set.
    ///
    /// Produces:
    /// - A const array with `as const` suffix
    /// - A type alias using `typeof Values[number]`
    /// - A type guard function for validation
    ///
    /// # Example Output
    ///
    /// ```typescript
    /// export const AdministrativeGenderValues = ["male", "female", "other", "unknown"] as const;
    /// export type AdministrativeGender = typeof AdministrativeGenderValues[number];
    ///
    /// export function isAdministrativeGender(value: string): value is AdministrativeGender {
    ///   return AdministrativeGenderValues.includes(value as any);
    /// }
    /// ```
    pub fn generate_typescript(&self) -> String {
        let values_name = format!("{}Values", self.type_name);
        let guard_name = format!("is{}", self.type_name);

        let mut output = String::new();

        // Add JSDoc comment if we have title or description
        if self.title.is_some() || self.description.is_some() {
            output.push_str("/**\n");
            if let Some(title) = &self.title {
                output.push_str(&format!(" * {}\n", title));
            }
            if let Some(desc) = &self.description {
                if self.title.is_some() {
                    output.push_str(" *\n");
                }
                output.push_str(&format!(" * {}\n", desc));
            }
            output.push_str(&format!(" * @see {}\n", self.canonical_url));
            output.push_str(" */\n");
        }

        // Generate const array
        let codes_str = self
            .codes
            .iter()
            .map(|code| format!("\"{}\"", escape_string(code)))
            .collect::<Vec<_>>()
            .join(", ");

        output.push_str(&format!(
            "export const {} = [{}] as const;\n\n",
            values_name, codes_str
        ));

        // Generate type alias
        output.push_str(&format!(
            "export type {} = typeof {}[number];\n\n",
            self.type_name, values_name
        ));

        // Generate type guard
        output.push_str(&format!(
            "export function {}(value: string): value is {} {{\n",
            guard_name, self.type_name
        ));
        output.push_str(&format!(
            "  return {}.includes(value as any);\n",
            values_name
        ));
        output.push_str("}\n");

        output
    }

    /// Generates a documentation comment with code display names.
    ///
    /// Useful for generating detailed JSDoc comments that list all codes with their meanings.
    pub fn generate_codes_doc(&self) -> String {
        let mut doc = String::from("/**\n * Valid codes:\n");

        for (code, display_opt) in self.codes.iter().zip(self.displays.iter()) {
            if let Some(display) = display_opt {
                doc.push_str(&format!(" * - `{}`: {}\n", code, display));
            } else {
                doc.push_str(&format!(" * - `{}`\n", code));
            }
        }

        doc.push_str(" */\n");
        doc
    }
}

/// Escapes special characters in strings for TypeScript string literals.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Converts a canonical URL to a TypeScript type name.
///
/// Extracts the last segment of the URL and converts it to PascalCase.
///
/// # Examples
///
/// ```
/// # use inkgen_typescript::valuesets::url_to_type_name;
/// assert_eq!(
///     url_to_type_name("http://hl7.org/fhir/ValueSet/administrative-gender"),
///     "AdministrativeGender"
/// );
/// assert_eq!(
///     url_to_type_name("http://hl7.org/fhir/ValueSet/contact-point-use"),
///     "ContactPointUse"
/// );
/// ```
pub fn url_to_type_name(url: &str) -> String {
    let segment = url
        .trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("ValueSet");

    // Convert kebab-case or snake_case to PascalCase
    segment
        .split(&['-', '_'][..])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_url_to_type_name() {
        assert_eq!(
            url_to_type_name("http://hl7.org/fhir/ValueSet/administrative-gender"),
            "AdministrativeGender"
        );
        assert_eq!(
            url_to_type_name("http://hl7.org/fhir/ValueSet/contact-point-use"),
            "ContactPointUse"
        );
        assert_eq!(
            url_to_type_name("http://example.org/fhir/ValueSet/my_value_set"),
            "MyValueSet"
        );
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("simple"), "simple");
        assert_eq!(escape_string("with\"quote"), "with\\\"quote");
        assert_eq!(escape_string("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_string("with\nnewline"), "with\\nnewline");
    }

    #[test]
    fn test_valueset_info_from_valueset() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://hl7.org/fhir/ValueSet/administrative-gender",
            "title": "Administrative Gender",
            "description": "The gender of a person used for administrative purposes",
            "expansion": {
                "contains": [
                    {"code": "male", "display": "Male"},
                    {"code": "female", "display": "Female"},
                    {"code": "other", "display": "Other"},
                    {"code": "unknown", "display": "Unknown"}
                ]
            }
        });

        let info = ValueSetInfo::from_valueset(&valueset, "AdministrativeGender".to_string(), None)
            .unwrap()
            .unwrap();

        assert_eq!(info.type_name, "AdministrativeGender");
        assert_eq!(
            info.canonical_url,
            "http://hl7.org/fhir/ValueSet/administrative-gender"
        );
        assert_eq!(info.codes.len(), 4);
        assert_eq!(info.codes[0], "male");
        assert_eq!(info.displays[0], Some("Male".to_string()));
    }

    #[test]
    fn test_valueset_info_exceeds_max() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://example.org/ValueSet/large",
            "expansion": {
                "contains": [
                    {"code": "code1"},
                    {"code": "code2"},
                    {"code": "code3"},
                    {"code": "code4"},
                    {"code": "code5"},
                    {"code": "code6"}
                ]
            }
        });

        // With max_codes=5, should return None because we have 6 codes
        let result = ValueSetInfo::from_valueset(&valueset, "Large".to_string(), Some(5)).unwrap();
        assert!(result.is_none());

        // With max_codes=10, should return Some
        let result =
            ValueSetInfo::from_valueset(&valueset, "Large".to_string(), Some(10)).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_generate_typescript() {
        let info = ValueSetInfo {
            type_name: "Gender".to_string(),
            canonical_url: "http://example.org/ValueSet/gender".to_string(),
            codes: vec!["male".to_string(), "female".to_string()],
            displays: vec![Some("Male".to_string()), Some("Female".to_string())],
            title: Some("Gender".to_string()),
            description: Some("Gender codes".to_string()),
        };

        let output = info.generate_typescript();

        assert!(output.contains("export const GenderValues = [\"male\", \"female\"] as const;"));
        assert!(output.contains("export type Gender = typeof GenderValues[number];"));
        assert!(output.contains("export function isGender(value: string): value is Gender"));
        assert!(output.contains("return GenderValues.includes(value as any);"));
        assert!(output.contains("/**"));
        assert!(output.contains("* Gender"));
        assert!(output.contains("* Gender codes"));
        assert!(output.contains("@see http://example.org/ValueSet/gender"));
    }

    #[test]
    fn test_generate_codes_doc() {
        let info = ValueSetInfo {
            type_name: "Status".to_string(),
            canonical_url: "http://example.org/ValueSet/status".to_string(),
            codes: vec![
                "active".to_string(),
                "inactive".to_string(),
                "pending".to_string(),
            ],
            displays: vec![
                Some("Active".to_string()),
                Some("Inactive".to_string()),
                None,
            ],
            title: None,
            description: None,
        };

        let doc = info.generate_codes_doc();

        assert!(doc.contains("* Valid codes:"));
        assert!(doc.contains("* - `active`: Active"));
        assert!(doc.contains("* - `inactive`: Inactive"));
        assert!(doc.contains("* - `pending`")); // No display
    }

    #[test]
    fn test_generate_typescript_escapes_strings() {
        let info = ValueSetInfo {
            type_name: "Special".to_string(),
            canonical_url: "http://example.org/ValueSet/special".to_string(),
            codes: vec!["code\"with\"quotes".to_string()],
            displays: vec![None],
            title: None,
            description: None,
        };

        let output = info.generate_typescript();

        assert!(output.contains("\"code\\\"with\\\"quotes\""));
    }
}
