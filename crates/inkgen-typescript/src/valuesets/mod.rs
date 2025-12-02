//! Value set code generation for TypeScript.
//!
//! This module provides utilities for generating TypeScript const arrays and
//! type unions from FHIR ValueSet resources with configurable size limits.

pub mod helpers;
pub mod metadata;

use inkgen_core::{extract_codes_from_valueset, should_generate_valueset};
use serde::Serialize;
use serde_json::Value;

/// Information about a single code in a ValueSet.
#[derive(Debug, Clone, Serialize)]
pub struct CodeInfo {
    /// The code value
    pub code: String,
    /// Optional display name for the code
    pub display: Option<String>,
    /// Optional definition/description of the code
    pub definition: Option<String>,
}

/// Binding strength for ValueSet bindings (determines if ValueSet is open or closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BindingStrength {
    /// Required: Only codes from this ValueSet are allowed (closed)
    Required,
    /// Extensible: Codes from this ValueSet should be used, but others allowed if needed (closed)
    Extensible,
    /// Preferred: Codes from this ValueSet are recommended but not enforced (open)
    Preferred,
    /// Example: Codes are just examples, any code allowed (open)
    Example,
}

/// Information about a value set to be generated as TypeScript code.
#[derive(Debug, Clone)]
pub struct ValueSetInfo {
    /// TypeScript type name (e.g., "AdministrativeGender")
    pub type_name: String,
    /// Canonical URL of the value set
    pub canonical_url: String,
    /// List of codes with their details
    pub code_info: Vec<CodeInfo>,
    /// Title of the value set
    pub title: Option<String>,
    /// Description of the value set
    pub description: Option<String>,
    /// Binding strength (determines open vs closed union types)
    pub binding_strength: Option<BindingStrength>,
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

        // Build CodeInfo structures with display (definition not available from expansion)
        let code_info: Vec<CodeInfo> = resolved
            .codes
            .iter()
            .map(|code| CodeInfo {
                code: code.clone(),
                display: resolved.displays.get(code).cloned(),
                definition: None, // Definitions not available in ValueSet expansion
            })
            .collect();

        Ok(Some(Self {
            type_name,
            canonical_url: resolved.url,
            code_info,
            title: resolved.title,
            description: resolved.description,
            binding_strength: None, // Will be set by caller if binding info is available
        }))
    }

    /// Returns whether this ValueSet should generate a closed union type.
    ///
    /// Closed types only allow the exact codes in the ValueSet.
    /// Open types allow the specified codes plus any custom string.
    ///
    /// Required and Extensible bindings are closed.
    /// Preferred and Example bindings are open.
    pub fn is_closed(&self) -> bool {
        matches!(
            self.binding_strength,
            Some(BindingStrength::Required) | Some(BindingStrength::Extensible)
        )
    }

    /// Helper method to get just the code values
    pub fn codes(&self) -> Vec<String> {
        self.code_info.iter().map(|c| c.code.clone()).collect()
    }

    /// Generates TypeScript code for this value set.
    ///
    /// Produces:
    /// - A const array with `as const` suffix
    /// - A type alias (closed or open based on binding strength)
    /// - A type guard function for validation
    /// - A code definitions object with display and definition info
    ///
    /// # Example Output (Closed)
    ///
    /// ```typescript
    /// export const AdministrativeGenderValues = ["male", "female", "other", "unknown"] as const;
    /// export type AdministrativeGender = typeof AdministrativeGenderValues[number];
    ///
    /// export function isAdministrativeGender(value: string): value is AdministrativeGender {
    ///   return AdministrativeGenderValues.includes(value as any);
    /// }
    ///
    /// export const AdministrativeGenderDefinitions = {
    ///   male: { code: "male", display: "Male" },
    ///   female: { code: "female", display: "Female" },
    /// } as const;
    /// ```
    ///
    /// # Example Output (Open)
    ///
    /// ```typescript
    /// export type AdministrativeGender = typeof AdministrativeGenderValues[number] | (string & {});
    /// ```
    pub fn generate_typescript(&self) -> String {
        let values_name = format!("{}Values", self.type_name);
        let guard_name = format!("is{}", self.type_name);
        let definitions_name = format!("{}Definitions", self.type_name);

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
            .code_info
            .iter()
            .map(|info| format!("\"{}\"", escape_string(&info.code)))
            .collect::<Vec<_>>()
            .join(", ");

        output.push_str(&format!(
            "export const {} = [{}] as const;\n\n",
            values_name, codes_str
        ));

        // Generate type alias (closed or open)
        if self.is_closed() {
            // Closed: only exact codes allowed
            output.push_str(&format!(
                "export type {} = typeof {}[number];\n\n",
                self.type_name, values_name
            ));
        } else {
            // Open: codes + any custom string (using `string & {}` trick for better autocomplete)
            output.push_str("// Open valueset - allows custom codes\n");
            output.push_str(&format!(
                "export type {} = typeof {}[number] | (string & {{}});\n\n",
                self.type_name, values_name
            ));
        }

        // Generate type guard
        output.push_str(&format!("/**\n * Type guard for {}\n */\n", self.type_name));
        output.push_str(&format!(
            "export function {}(value: string): value is {} {{\n",
            guard_name, self.type_name
        ));
        output.push_str(&format!(
            "  return {}.includes(value as any);\n",
            values_name
        ));
        output.push_str("}\n");

        // Generate code definitions object if we have display or definition info
        if self
            .code_info
            .iter()
            .any(|c| c.display.is_some() || c.definition.is_some())
        {
            output.push_str("\n/**\n * Code definitions\n */\n");
            output.push_str(&format!("export const {} = {{\n", definitions_name));

            for (idx, code_info) in self.code_info.iter().enumerate() {
                output.push_str(&format!("  \"{}\": {{\n", escape_string(&code_info.code)));
                output.push_str(&format!(
                    "    code: \"{}\",\n",
                    escape_string(&code_info.code)
                ));

                if let Some(display) = &code_info.display {
                    output.push_str(&format!("    display: \"{}\",\n", escape_string(display)));
                }

                if let Some(definition) = &code_info.definition {
                    output.push_str(&format!(
                        "    definition: \"{}\",\n",
                        escape_string(definition)
                    ));
                }

                let comma = if idx < self.code_info.len() - 1 {
                    ","
                } else {
                    ""
                };
                output.push_str(&format!("  }}{}\n", comma));
            }

            output.push_str("} as const;\n");
        }

        output
    }

    /// Generates a documentation comment with code display names.
    ///
    /// Useful for generating detailed JSDoc comments that list all codes with their meanings.
    pub fn generate_codes_doc(&self) -> String {
        let mut doc = String::from("/**\n * Valid codes:\n");

        for code_info in &self.code_info {
            if let Some(display) = &code_info.display {
                doc.push_str(&format!(" * - `{}`: {}\n", code_info.code, display));
            } else {
                doc.push_str(&format!(" * - `{}`\n", code_info.code));
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
        .next_back()
        .unwrap_or("ValueSet");

    // Convert kebab-case or snake_case to PascalCase
    segment
        .split(&['-', '_'][..])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
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
        assert_eq!(info.code_info.len(), 4);
        assert_eq!(info.code_info[0].code, "male");
        assert_eq!(info.code_info[0].display, Some("Male".to_string()));
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
        let result = ValueSetInfo::from_valueset(&valueset, "Large".to_string(), Some(10)).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_generate_typescript() {
        let info = ValueSetInfo {
            type_name: "Gender".to_string(),
            canonical_url: "http://example.org/ValueSet/gender".to_string(),
            code_info: vec![
                CodeInfo {
                    code: "male".to_string(),
                    display: Some("Male".to_string()),
                    definition: None,
                },
                CodeInfo {
                    code: "female".to_string(),
                    display: Some("Female".to_string()),
                    definition: None,
                },
            ],
            title: Some("Gender".to_string()),
            description: Some("Gender codes".to_string()),
            binding_strength: Some(BindingStrength::Required), // Closed valueset for this test
        };

        let output = info.generate_typescript();

        assert!(output.contains("export const GenderValues = [\"male\", \"female\"] as const;"));
        assert!(output.contains("export type Gender = typeof GenderValues[number];"));
        assert!(!output.contains("| (string & {})")); // Should be closed, not open
        assert!(output.contains("export function isGender(value: string): value is Gender"));
        assert!(output.contains("return GenderValues.includes(value as any);"));
        assert!(output.contains("/**"));
        assert!(output.contains("* Gender"));
        assert!(output.contains("* Gender codes"));
        assert!(output.contains("@see http://example.org/ValueSet/gender"));
        assert!(output.contains("export const GenderDefinitions"));
    }

    #[test]
    fn test_generate_codes_doc() {
        let info = ValueSetInfo {
            type_name: "Status".to_string(),
            canonical_url: "http://example.org/ValueSet/status".to_string(),
            code_info: vec![
                CodeInfo {
                    code: "active".to_string(),
                    display: Some("Active".to_string()),
                    definition: None,
                },
                CodeInfo {
                    code: "inactive".to_string(),
                    display: Some("Inactive".to_string()),
                    definition: None,
                },
                CodeInfo {
                    code: "pending".to_string(),
                    display: None,
                    definition: None,
                },
            ],
            title: None,
            description: None,
            binding_strength: None,
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
            code_info: vec![CodeInfo {
                code: "code\"with\"quotes".to_string(),
                display: None,
                definition: None,
            }],
            title: None,
            description: None,
            binding_strength: None,
        };

        let output = info.generate_typescript();

        assert!(output.contains("\"code\\\"with\\\"quotes\""));
    }

    #[test]
    fn test_closed_valueset() {
        let mut info = ValueSetInfo {
            type_name: "Status".to_string(),
            canonical_url: "http://example.org/ValueSet/status".to_string(),
            code_info: vec![CodeInfo {
                code: "active".to_string(),
                display: Some("Active".to_string()),
                definition: None,
            }],
            title: None,
            description: None,
            binding_strength: Some(BindingStrength::Required),
        };

        assert!(info.is_closed());

        let output = info.generate_typescript();
        assert!(output.contains("export type Status = typeof StatusValues[number];"));
        assert!(!output.contains("| (string & {})"));

        // Test Extensible is also closed
        info.binding_strength = Some(BindingStrength::Extensible);
        assert!(info.is_closed());
    }

    #[test]
    fn test_open_valueset() {
        let mut info = ValueSetInfo {
            type_name: "Status".to_string(),
            canonical_url: "http://example.org/ValueSet/status".to_string(),
            code_info: vec![CodeInfo {
                code: "active".to_string(),
                display: Some("Active".to_string()),
                definition: None,
            }],
            title: None,
            description: None,
            binding_strength: Some(BindingStrength::Preferred),
        };

        assert!(!info.is_closed());

        let output = info.generate_typescript();
        assert!(output.contains("// Open valueset - allows custom codes"));
        assert!(
            output.contains("export type Status = typeof StatusValues[number] | (string & {});")
        );

        // Test Example is also open
        info.binding_strength = Some(BindingStrength::Example);
        assert!(!info.is_closed());

        // Test None defaults to open
        info.binding_strength = None;
        assert!(!info.is_closed());
    }

    #[test]
    fn test_code_definitions_object() {
        let info = ValueSetInfo {
            type_name: "Status".to_string(),
            canonical_url: "http://example.org/ValueSet/status".to_string(),
            code_info: vec![
                CodeInfo {
                    code: "active".to_string(),
                    display: Some("Active".to_string()),
                    definition: Some("The resource is currently active".to_string()),
                },
                CodeInfo {
                    code: "inactive".to_string(),
                    display: Some("Inactive".to_string()),
                    definition: None,
                },
            ],
            title: None,
            description: None,
            binding_strength: None,
        };

        let output = info.generate_typescript();

        assert!(output.contains("export const StatusDefinitions = {"));
        assert!(output.contains("\"active\": {"));
        assert!(output.contains("code: \"active\","));
        assert!(output.contains("display: \"Active\","));
        assert!(output.contains("definition: \"The resource is currently active\","));
        assert!(output.contains("\"inactive\": {"));
        assert!(output.contains("display: \"Inactive\","));
        assert!(!output.contains("definition: \"Inactive\",")); // inactive has no definition
    }
}
