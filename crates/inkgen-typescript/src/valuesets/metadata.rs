//! Rich metadata extraction for ValueSets by linking to CodeSystem resources.
//!
//! This module enhances ValueSet generation by loading corresponding CodeSystem
//! resources and extracting additional metadata like concept definitions and comments.

use indexmap::IndexMap;
use serde_json::Value;
use std::fmt::Write;

use super::escape_string;

/// Enhanced code information with CodeSystem metadata
#[derive(Debug, Clone)]
pub struct EnhancedCodeInfo {
    /// The code value
    pub code: String,
    /// Display name for the code
    pub display: Option<String>,
    /// Definition/description from CodeSystem
    pub definition: Option<String>,
    /// Additional comments from extensions
    pub comments: Vec<String>,
}

/// Enhanced ValueSet information with CodeSystem metadata
#[derive(Debug, Clone)]
pub struct EnhancedValueSetInfo {
    /// TypeScript type name (e.g., "AdministrativeGender")
    pub type_name: String,
    /// Canonical URL of the ValueSet
    pub valueset_url: String,
    /// CodeSystem URL (if linked)
    pub system_url: Option<String>,
    /// Enhanced code information with definitions
    pub codes: Vec<EnhancedCodeInfo>,
    /// Title of the ValueSet
    pub title: Option<String>,
    /// Description of the ValueSet
    pub description: Option<String>,
    /// Whether the CodeSystem is case-sensitive
    pub case_sensitive: Option<bool>,
}

/// Extract system URL from a ValueSet resource
///
/// Attempts to find the CodeSystem URL from:
/// 1. compose.include[0].system (most common)
/// 2. expansion.contains[0].system (fallback)
pub fn extract_system_url(valueset_json: &Value) -> Option<String> {
    // Try compose.include first
    if let Some(compose) = valueset_json.get("compose")
        && let Some(include) = compose.get("include").and_then(|v| v.as_array())
        && let Some(first_include) = include.first()
        && let Some(system) = first_include.get("system").and_then(|v| v.as_str())
    {
        return Some(system.to_string());
    }

    // Fallback to expansion
    if let Some(expansion) = valueset_json.get("expansion")
        && let Some(contains) = expansion.get("contains").and_then(|v| v.as_array())
        && let Some(first_contain) = contains.first()
        && let Some(system) = first_contain.get("system").and_then(|v| v.as_str())
    {
        return Some(system.to_string());
    }

    None
}

/// Load CodeSystem resource from JSON
///
/// Extracts concept definitions and metadata from a CodeSystem resource
pub fn load_codesystem_metadata(codesystem_json: &Value) -> CodeSystemMetadata {
    let url = codesystem_json
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let case_sensitive = codesystem_json
        .get("caseSensitive")
        .and_then(|v| v.as_bool());

    // Extract concepts into a map
    let mut concepts = IndexMap::new();
    if let Some(concept_array) = codesystem_json.get("concept").and_then(|v| v.as_array()) {
        for concept in concept_array {
            if let Some(code) = concept.get("code").and_then(|v| v.as_str()) {
                let display = concept
                    .get("display")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let definition = concept
                    .get("definition")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Extract comments from extensions
                let mut comments = Vec::new();
                if let Some(extensions) = concept.get("extension").and_then(|v| v.as_array()) {
                    for ext in extensions {
                        if let Some(ext_url) = ext.get("url").and_then(|v| v.as_str())
                            && ext_url.contains("codesystem-concept-comments")
                            && let Some(comment) = ext.get("valueString").and_then(|v| v.as_str())
                        {
                            comments.push(comment.to_string());
                        }
                    }
                }

                concepts.insert(
                    code.to_string(),
                    ConceptInfo {
                        code: code.to_string(),
                        display,
                        definition,
                        comments,
                    },
                );
            }
        }
    }

    CodeSystemMetadata {
        url,
        case_sensitive,
        concepts,
    }
}

/// Metadata extracted from a CodeSystem resource
#[derive(Debug, Clone)]
pub struct CodeSystemMetadata {
    /// CodeSystem URL
    pub url: Option<String>,
    /// Whether codes are case-sensitive
    pub case_sensitive: Option<bool>,
    /// Map of code → concept info
    pub concepts: IndexMap<String, ConceptInfo>,
}

/// Information about a single concept from CodeSystem
#[derive(Debug, Clone)]
pub struct ConceptInfo {
    /// The code value
    pub code: String,
    /// Display name
    pub display: Option<String>,
    /// Definition/description
    pub definition: Option<String>,
    /// Additional comments
    pub comments: Vec<String>,
}

/// Enhance ValueSet codes with CodeSystem metadata
///
/// Merges basic code information from ValueSet with rich metadata from CodeSystem
pub fn enhance_codes(
    basic_codes: &[(String, Option<String>)], // (code, display)
    codesystem_meta: &CodeSystemMetadata,
) -> Vec<EnhancedCodeInfo> {
    basic_codes
        .iter()
        .map(|(code, display)| {
            // Look up concept in CodeSystem
            if let Some(concept) = codesystem_meta.concepts.get(code) {
                EnhancedCodeInfo {
                    code: code.clone(),
                    // Prefer ValueSet display, fallback to CodeSystem
                    display: display.clone().or_else(|| concept.display.clone()),
                    definition: concept.definition.clone(),
                    comments: concept.comments.clone(),
                }
            } else {
                // Code not found in CodeSystem, use basic info
                EnhancedCodeInfo {
                    code: code.clone(),
                    display: display.clone(),
                    definition: None,
                    comments: Vec::new(),
                }
            }
        })
        .collect()
}

/// Attempt to find CodeSystem URL from ValueSet URL
///
/// Uses common FHIR URL patterns:
/// - http://hl7.org/fhir/ValueSet/foo → http://hl7.org/fhir/foo
/// - Works for most standard FHIR ValueSets
pub fn infer_codesystem_url_from_valueset(valueset_url: &str) -> Option<String> {
    // Pattern: http://hl7.org/fhir/ValueSet/name → http://hl7.org/fhir/name
    if let Some(pos) = valueset_url.rfind("/ValueSet/") {
        let base = &valueset_url[..pos];
        let name = &valueset_url[pos + 10..]; // Skip "/ValueSet/"
        return Some(format!("{}/{}", base, name));
    }

    None
}

/// Render a TypeScript metadata block for a ValueSet.
pub fn render_metadata_block(
    type_name: &str,
    canonical_url: &str,
    system_url: Option<&str>,
    case_sensitive: Option<bool>,
    codes: &[EnhancedCodeInfo],
) -> Option<String> {
    if codes.is_empty() {
        return None;
    }

    let mut output = String::new();

    writeln!(
        output,
        "/**\n * Metadata for {} codes\n * Includes display names, definitions, and code system information\n */",
        type_name
    )
    .ok()?;
    writeln!(output, "export const {}Metadata = {{", type_name).ok()?;
    writeln!(output, "  canonical: \"{}\",", escape_string(canonical_url)).ok()?;

    if let Some(system_url) = system_url {
        writeln!(output, "  system: \"{}\",", escape_string(system_url)).ok()?;
    }

    if let Some(case_sensitive) = case_sensitive {
        writeln!(output, "  caseSensitive: {},", case_sensitive).ok()?;
    }

    output.push_str("  codes: {\n");

    for code in codes {
        writeln!(output, "    \"{}\": {{", escape_string(&code.code)).ok()?;
        writeln!(output, "      code: \"{}\",", escape_string(&code.code)).ok()?;

        if let Some(display) = &code.display {
            writeln!(output, "      display: \"{}\",", escape_string(display)).ok()?;
        }

        if let Some(definition) = &code.definition {
            writeln!(
                output,
                "      definition: \"{}\",",
                escape_string(definition)
            )
            .ok()?;
        }

        if !code.comments.is_empty() {
            output.push_str("      comments: [");
            for (idx, comment) in code.comments.iter().enumerate() {
                if idx > 0 {
                    output.push_str(", ");
                }
                write!(output, "\"{}\"", escape_string(comment)).ok()?;
            }
            output.push_str("],\n");
        }

        output.push_str("    },\n");
    }

    output.push_str("  }\n} as const;\n");

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_system_url_from_compose() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://hl7.org/fhir/ValueSet/administrative-gender",
            "compose": {
                "include": [{
                    "system": "http://hl7.org/fhir/administrative-gender"
                }]
            }
        });

        let system = extract_system_url(&valueset);
        assert_eq!(
            system,
            Some("http://hl7.org/fhir/administrative-gender".to_string())
        );
    }

    #[test]
    fn test_extract_system_url_from_expansion() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://hl7.org/fhir/ValueSet/administrative-gender",
            "expansion": {
                "contains": [{
                    "system": "http://hl7.org/fhir/administrative-gender",
                    "code": "male"
                }]
            }
        });

        let system = extract_system_url(&valueset);
        assert_eq!(
            system,
            Some("http://hl7.org/fhir/administrative-gender".to_string())
        );
    }

    #[test]
    fn test_load_codesystem_metadata() {
        let codesystem = json!({
            "resourceType": "CodeSystem",
            "url": "http://hl7.org/fhir/administrative-gender",
            "caseSensitive": true,
            "concept": [
                {
                    "code": "male",
                    "display": "Male",
                    "definition": "Male.",
                    "extension": [{
                        "url": "http://hl7.org/fhir/StructureDefinition/codesystem-concept-comments",
                        "valueString": "Male gender"
                    }]
                },
                {
                    "code": "female",
                    "display": "Female",
                    "definition": "Female."
                }
            ]
        });

        let metadata = load_codesystem_metadata(&codesystem);

        assert_eq!(
            metadata.url,
            Some("http://hl7.org/fhir/administrative-gender".to_string())
        );
        assert_eq!(metadata.case_sensitive, Some(true));
        assert_eq!(metadata.concepts.len(), 2);

        let male_concept = metadata.concepts.get("male").unwrap();
        assert_eq!(male_concept.display, Some("Male".to_string()));
        assert_eq!(male_concept.definition, Some("Male.".to_string()));
        assert_eq!(male_concept.comments, vec!["Male gender"]);
    }

    #[test]
    fn test_enhance_codes() {
        let codesystem_meta = CodeSystemMetadata {
            url: Some("http://example.org/CodeSystem/test".to_string()),
            case_sensitive: Some(true),
            concepts: indexmap::indexmap! {
                "code1".to_string() => ConceptInfo {
                    code: "code1".to_string(),
                    display: Some("Display 1".to_string()),
                    definition: Some("Definition 1".to_string()),
                    comments: vec!["Comment 1".to_string()],
                },
            },
        };

        let basic_codes = vec![
            ("code1".to_string(), Some("ValueSet Display".to_string())),
            ("code2".to_string(), None),
        ];

        let enhanced = enhance_codes(&basic_codes, &codesystem_meta);

        assert_eq!(enhanced.len(), 2);

        // First code should have CodeSystem metadata
        assert_eq!(enhanced[0].code, "code1");
        assert_eq!(enhanced[0].display, Some("ValueSet Display".to_string())); // ValueSet display preferred
        assert_eq!(enhanced[0].definition, Some("Definition 1".to_string()));
        assert_eq!(enhanced[0].comments, vec!["Comment 1"]);

        // Second code not in CodeSystem
        assert_eq!(enhanced[1].code, "code2");
        assert_eq!(enhanced[1].display, None);
        assert_eq!(enhanced[1].definition, None);
        assert!(enhanced[1].comments.is_empty());
    }

    #[test]
    fn test_infer_codesystem_url() {
        let valueset_url = "http://hl7.org/fhir/ValueSet/administrative-gender";
        let inferred = infer_codesystem_url_from_valueset(valueset_url);

        assert_eq!(
            inferred,
            Some("http://hl7.org/fhir/administrative-gender".to_string())
        );
    }

    #[test]
    fn test_render_metadata_block() {
        let codes = vec![EnhancedCodeInfo {
            code: "active".to_string(),
            display: Some("Active".to_string()),
            definition: Some("The thing is active.".to_string()),
            comments: vec!["First comment".to_string(), "Second comment".to_string()],
        }];

        let rendered = render_metadata_block(
            "TestStatus",
            "http://example.org/ValueSet/test",
            Some("http://example.org/CodeSystem/test"),
            Some(true),
            &codes,
        )
        .expect("metadata rendered");

        assert!(rendered.contains("export const TestStatusMetadata"));
        assert!(rendered.contains("canonical: \"http://example.org/ValueSet/test\""));
        assert!(rendered.contains("system: \"http://example.org/CodeSystem/test\""));
        assert!(rendered.contains("caseSensitive: true"));
        assert!(rendered.contains("\"active\""));
        assert!(rendered.contains("comments: [\"First comment\", \"Second comment\"]"));
    }
}
