//! Terminology helper function generation for code systems and value sets.
//!
//! This module handles:
//! - Generating typed helper functions for code system and value set access
//! - Creating Coding/CodeableConcept builders with compile-time safety
//! - Generating code system constant definitions
//! - Creating value set binding helper functions

use indexmap::IndexMap;
use inkgen_core::ir::{BindingStrength, ResourceDefinition};

/// Metadata for a code system constant or literal.
#[derive(Debug, Clone)]
pub struct CodeSystemConstant {
    /// Constant name (e.g., "ACTIVE", "PENDING")
    pub constant_name: String,
    /// Code value (e.g., "active", "pending")
    pub code_value: String,
    /// Human-readable display text
    pub display: Option<String>,
    /// Code system URL this belongs to
    pub code_system_url: String,
}

/// Metadata for a code system with constants.
#[derive(Debug, Clone)]
pub struct CodeSystemDefinition {
    /// Code system URL
    pub url: String,
    /// Code system name in PascalCase (e.g., "AdministrativeGender")
    pub type_name: String,
    /// Constants/codes in this system
    pub constants: Vec<CodeSystemConstant>,
    /// Description
    pub description: Option<String>,
}

/// Metadata for a value set binding on an element.
#[derive(Debug, Clone)]
pub struct ValueSetBindingInfo {
    /// Value set URL
    pub value_set_url: String,
    /// Value set name in PascalCase
    pub type_name: String,
    /// Binding strength (required, extensible, preferred, example)
    pub strength: BindingStrength,
    /// Whether additional codes beyond the value set are allowed
    pub extensible: bool,
    /// Description
    pub description: Option<String>,
}

/// Helper function for accessing a code as a Coding object.
#[derive(Debug, Clone)]
pub struct CodeAccessorFunction {
    /// Function name (e.g., "getAdministrativeGenderActive")
    pub function_name: String,
    /// Return type (e.g., "Coding")
    pub return_type: String,
    /// Code system URL
    pub code_system_url: String,
    /// Code value being accessed
    pub code_value: String,
    /// Human-readable display text
    pub display: Option<String>,
}

/// Builder function for creating CodeableConcept with type safety.
#[derive(Debug, Clone)]
pub struct CodeableConceptBuilder {
    /// Function name (e.g., "createAdministrativeGenderCoding")
    pub function_name: String,
    /// Code system name
    pub code_system_name: String,
    /// Code system URL
    pub code_system_url: String,
    /// Whether the builder accepts multiple codes
    pub is_union: bool,
}

/// Collection of all terminology helpers for a resource.
#[derive(Debug, Clone)]
pub struct TerminologyHelpers {
    /// Code systems with constants
    pub code_systems: IndexMap<String, CodeSystemDefinition>,
    /// Value set bindings on elements
    pub value_set_bindings: IndexMap<String, ValueSetBindingInfo>,
    /// Code accessor functions
    pub code_accessors: Vec<CodeAccessorFunction>,
    /// CodeableConcept builders
    pub builders: Vec<CodeableConceptBuilder>,
}

/// Extract all terminology helpers from a resource definition.
pub fn extract_terminology_helpers(resource: &ResourceDefinition) -> TerminologyHelpers {
    let mut code_systems: IndexMap<String, CodeSystemDefinition> = IndexMap::new();
    let mut value_set_bindings: IndexMap<String, ValueSetBindingInfo> = IndexMap::new();

    // Scan all elements for bindings
    for element in &resource.elements {
        if let Some(binding) = &element.binding
            && let Some(vs_url) = &binding.value_set
        {
            let is_extensible = matches!(binding.strength, BindingStrength::Extensible);

            let type_name = valueset_url_to_type_name(vs_url);

            value_set_bindings.insert(
                vs_url.clone(),
                ValueSetBindingInfo {
                    value_set_url: vs_url.clone(),
                    type_name,
                    strength: binding.strength,
                    extensible: is_extensible,
                    description: binding.description.clone(),
                },
            );

            // Extract code system from additional field if available
            if let Some(cs_value) = binding.additional.get("codesystem")
                && let Some(cs_url) = cs_value.as_str()
                && !code_systems.contains_key(cs_url)
            {
                let cs_type_name = code_system_url_to_type_name(cs_url);
                code_systems.insert(
                    cs_url.to_string(),
                    CodeSystemDefinition {
                        url: cs_url.to_string(),
                        type_name: cs_type_name,
                        constants: Vec::new(), // Will be populated from codes
                        description: None,
                    },
                );
            }
        }
    }

    // Generate code accessors and builders from value set bindings
    let code_accessors = Vec::new();
    let mut builders = Vec::new();

    for (vs_url, vs_binding) in &value_set_bindings {
        // Create a builder for this value set
        if let Some(cs_url) = extract_code_system_from_valueset(vs_url) {
            let cs_name = code_system_url_to_type_name(&cs_url);
            builders.push(CodeableConceptBuilder {
                function_name: format!("create{}", vs_binding.type_name),
                code_system_name: cs_name,
                code_system_url: cs_url,
                is_union: vs_binding.extensible,
            });
        }
    }

    TerminologyHelpers {
        code_systems,
        value_set_bindings,
        code_accessors,
        builders,
    }
}

/// Convert a value set URL to a TypeScript type name in PascalCase.
///
/// Examples:
/// - `http://hl7.org/fhir/ValueSet/administrative-gender`
///   -> `AdministrativeGender`
/// - `http://example.org/fhir/ValueSet/my-codes`
///   -> `MyCodes`
pub fn valueset_url_to_type_name(url: &str) -> String {
    // Extract the last component after ValueSet/ or final /
    let last_component = url.split('/').next_back().unwrap_or("ValueSet");

    // Convert kebab-case to PascalCase
    last_component
        .split('-')
        .map(|word| {
            if word.is_empty() {
                String::new()
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        })
        .collect()
}

/// Convert a code system URL to a TypeScript type name.
///
/// Examples:
/// - `http://hl7.org/fhir/administrative-gender`
///   -> `AdministrativeGender`
/// - `http://example.org/codes/status`
///   -> `Status`
pub fn code_system_url_to_type_name(url: &str) -> String {
    let last_component = url.split('/').next_back().unwrap_or("CodeSystem");

    // Convert kebab-case to PascalCase, handling common prefixes
    last_component
        .split('-')
        .map(|word| {
            if word.is_empty() {
                String::new()
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        })
        .collect()
}

/// Extract code system URL from a value set URL (heuristic).
///
/// This is a simplified heuristic - real implementation would use manifest data.
fn extract_code_system_from_valueset(valueset_url: &str) -> Option<String> {
    // Convert ValueSet/ to CodeSystem/
    if valueset_url.contains("/ValueSet/") {
        return Some(valueset_url.replace("/ValueSet/", "/CodeSystem/"));
    }

    // Try replacing valueset name with common code system naming patterns
    let last_part = valueset_url.split('/').next_back()?;
    if last_part.starts_with("valueset-") {
        let code_part = last_part.strip_prefix("valueset-")?;
        let base = valueset_url.rsplit_once('/')?;
        return Some(format!("{}/CodeSystem/{}", base.0, code_part));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valueset_url_to_type_name_simple() {
        assert_eq!(
            valueset_url_to_type_name("http://hl7.org/fhir/ValueSet/administrative-gender"),
            "AdministrativeGender"
        );
    }

    #[test]
    fn test_valueset_url_to_type_name_simple_name() {
        assert_eq!(
            valueset_url_to_type_name("http://example.org/fhir/ValueSet/my-codes"),
            "MyCodes"
        );
    }

    #[test]
    fn test_code_system_url_to_type_name() {
        assert_eq!(
            code_system_url_to_type_name("http://hl7.org/fhir/administrative-gender"),
            "AdministrativeGender"
        );
    }

    #[test]
    fn test_code_system_url_to_type_name_simple() {
        assert_eq!(
            code_system_url_to_type_name("http://example.org/codes/status"),
            "Status"
        );
    }

    #[test]
    fn test_extract_code_system_from_valueset_valueset_path() {
        let result =
            extract_code_system_from_valueset("http://hl7.org/fhir/ValueSet/administrative-gender");
        assert_eq!(
            result,
            Some("http://hl7.org/fhir/CodeSystem/administrative-gender".to_string())
        );
    }

    #[test]
    fn test_extract_code_system_from_valueset_valueset_prefix() {
        let result = extract_code_system_from_valueset("http://example.org/fhir/valueset-my-codes");
        assert_eq!(
            result,
            Some("http://example.org/fhir/CodeSystem/my-codes".to_string())
        );
    }

    #[test]
    fn test_code_system_constant_generation() {
        let constant = CodeSystemConstant {
            constant_name: "ACTIVE".to_string(),
            code_value: "active".to_string(),
            display: Some("Active".to_string()),
            code_system_url: "http://hl7.org/fhir/administrative-gender".to_string(),
        };

        assert_eq!(constant.constant_name, "ACTIVE");
        assert_eq!(constant.code_value, "active");
        assert!(constant.display.is_some());
    }

    #[test]
    fn test_code_system_definition() {
        let cs = CodeSystemDefinition {
            url: "http://hl7.org/fhir/administrative-gender".to_string(),
            type_name: "AdministrativeGender".to_string(),
            constants: vec![
                CodeSystemConstant {
                    constant_name: "MALE".to_string(),
                    code_value: "male".to_string(),
                    display: Some("Male".to_string()),
                    code_system_url: "http://hl7.org/fhir/administrative-gender".to_string(),
                },
                CodeSystemConstant {
                    constant_name: "FEMALE".to_string(),
                    code_value: "female".to_string(),
                    display: Some("Female".to_string()),
                    code_system_url: "http://hl7.org/fhir/administrative-gender".to_string(),
                },
            ],
            description: Some("Administrative Gender".to_string()),
        };

        assert_eq!(cs.constants.len(), 2);
        assert_eq!(cs.type_name, "AdministrativeGender");
    }

    #[test]
    fn test_valueset_binding_info() {
        let binding = ValueSetBindingInfo {
            value_set_url: "http://hl7.org/fhir/ValueSet/administrative-gender".to_string(),
            type_name: "AdministrativeGender".to_string(),
            strength: BindingStrength::Required,
            extensible: false,
            description: Some("The gender of a person".to_string()),
        };

        assert_eq!(binding.strength, BindingStrength::Required);
        assert!(!binding.extensible);
    }

    #[test]
    fn test_code_accessor_function() {
        let accessor = CodeAccessorFunction {
            function_name: "getAdministrativeGenderMale".to_string(),
            return_type: "Coding".to_string(),
            code_system_url: "http://hl7.org/fhir/administrative-gender".to_string(),
            code_value: "male".to_string(),
            display: Some("Male".to_string()),
        };

        assert_eq!(accessor.function_name, "getAdministrativeGenderMale");
        assert!(accessor.display.is_some());
    }

    #[test]
    fn test_codeable_concept_builder() {
        let builder = CodeableConceptBuilder {
            function_name: "createAdministrativeGenderCoding".to_string(),
            code_system_name: "AdministrativeGender".to_string(),
            code_system_url: "http://hl7.org/fhir/administrative-gender".to_string(),
            is_union: false,
        };

        assert_eq!(builder.function_name, "createAdministrativeGenderCoding");
        assert!(!builder.is_union);
    }
}
