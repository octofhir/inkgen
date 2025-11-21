use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Derivation, InvariantDefinition, ResourceDefinition};

/// Language-agnostic intermediate representation for FHIR profiles.
///
/// A profile is a StructureDefinition that constrains a base resource or data type.
/// This IR extracts profile-specific information for code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDefinition {
    /// Canonical URL of the profile
    pub url: String,
    /// Name identifier for the profile (e.g., "USCorePatientProfile")
    pub name: String,
    /// Base resource type being constrained (e.g., "Patient", "Observation")
    pub base_type: String,
    /// Human-readable description of the profile
    pub description: Option<String>,
    /// Elements that have must-support flag set
    pub must_support: Vec<MustSupportElement>,
    /// Extensions defined or constrained in this profile
    pub extensions: Vec<ProfileExtension>,
    /// Fixed values defined at specific paths
    pub fixed_values: HashMap<String, FixedValue>,
    /// Invariant constraints defined on the profile
    pub invariants: Vec<InvariantDefinition>,
}

impl ProfileDefinition {
    /// Extract a ProfileDefinition from a ResourceDefinition IR.
    ///
    /// Returns None if the ResourceDefinition is not a profile (i.e., not a constraint derivation).
    pub fn from_resource_definition(resource: &ResourceDefinition) -> Option<Self> {
        // Only process constraint derivations (profiles)
        if resource.lineage.derivation != Some(Derivation::Constraint) {
            return None;
        }

        let base_type = resource
            .lineage
            .type_name
            .clone()
            .or_else(|| {
                // Extract from base_definition URL if type_name is not available
                resource
                    .lineage
                    .base_definition
                    .as_ref()
                    .and_then(|url| url.rsplit('/').next().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "Resource".to_string());

        let name = resource.name.clone().unwrap_or_else(|| resource.id.clone());

        // Collect must-support elements
        let must_support = collect_must_support_elements(&resource.elements);

        // Extract extensions from sliced elements
        let extensions = extract_profile_extensions(&resource.elements);

        // Collect fixed values
        let fixed_values = collect_fixed_values(&resource.elements);

        Some(ProfileDefinition {
            url: resource.url.clone(),
            name,
            base_type,
            description: resource.description.clone(),
            must_support,
            extensions,
            fixed_values,
            invariants: resource.invariants.clone(),
        })
    }
}

/// Represents an element marked as must-support in a profile.
///
/// Must-support indicates that systems must be able to populate and
/// meaningfully process this element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MustSupportElement {
    /// Element path (e.g., "Patient.identifier", "Patient.name.family")
    pub path: String,
    /// Minimum cardinality constraint
    pub min: u32,
    /// Maximum cardinality constraint (None means unbounded)
    pub max: Option<u32>,
}

/// Extension metadata extracted from a profile.
///
/// Profiles typically slice the extension element to define which
/// extensions are supported and with what cardinality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileExtension {
    /// Local name for the extension (extracted from slice name or URL)
    pub name: String,
    /// Canonical URL of the extension
    pub url: String,
    /// Cardinality constraints (min, max)
    pub cardinality: (u32, Option<u32>),
    /// Type of value this extension holds
    pub value_type: ExtensionValueType,
    /// Human-readable description
    pub description: Option<String>,
}

/// The type of value an extension can hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtensionValueType {
    /// Simple primitive type (e.g., "string", "integer", "boolean")
    Simple(String),
    /// Complex FHIR type (e.g., "Coding", "CodeableConcept", "Reference")
    Complex(String),
    /// Nested sub-extensions (for complex extensions)
    Nested(Vec<ProfileExtension>),
    /// Choice type - can be one of multiple types
    Choice(Vec<String>),
}

/// A fixed value constraint at a specific element path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedValue {
    /// Element path where the value is fixed
    pub path: String,
    /// The fixed value (JSON representation)
    pub value: serde_json::Value,
}

/// Collect all elements marked as must-support from the element tree.
fn collect_must_support_elements(elements: &[super::ElementDefinition]) -> Vec<MustSupportElement> {
    let mut result = Vec::new();

    fn traverse(elements: &[super::ElementDefinition], result: &mut Vec<MustSupportElement>) {
        for element in elements {
            if element.must_support {
                let max = match element.cardinality.max {
                    super::ElementMax::Finite(n) => Some(n),
                    super::ElementMax::Unbounded => None,
                };

                result.push(MustSupportElement {
                    path: element.path.clone(),
                    min: element.cardinality.min,
                    max,
                });
            }

            // Recursively check children
            traverse(&element.children, result);
        }
    }

    traverse(elements, &mut result);
    result
}

/// Extract extension definitions from sliced extension elements in the profile.
fn extract_profile_extensions(elements: &[super::ElementDefinition]) -> Vec<ProfileExtension> {
    let mut result = Vec::new();

    fn traverse(elements: &[super::ElementDefinition], result: &mut Vec<ProfileExtension>) {
        for element in elements {
            // Look for sliced extension elements
            // These have paths like "Patient.extension" with slice_name set
            if element.path.ends_with(".extension")
                && element.slice_name.is_some()
                && let Some(ext) = parse_extension_from_element(element)
            {
                result.push(ext);
            }

            // Also check for sliced modifierExtension
            if element.path.ends_with(".modifierExtension")
                && element.slice_name.is_some()
                && let Some(ext) = parse_extension_from_element(element)
            {
                result.push(ext);
            }

            // Recursively check children
            traverse(&element.children, result);
        }
    }

    traverse(elements, &mut result);
    result
}

/// Parse an extension from a sliced element definition.
fn parse_extension_from_element(element: &super::ElementDefinition) -> Option<ProfileExtension> {
    let name = element.slice_name.as_ref()?.clone();

    // Extract extension URL from fixed value on the "url" sub-element
    let url = extract_extension_url(element)?;

    let max = match element.cardinality.max {
        super::ElementMax::Finite(n) => Some(n),
        super::ElementMax::Unbounded => None,
    };

    // Determine value type from the extension's value[x] element or children
    let value_type = determine_extension_value_type(element);

    Some(ProfileExtension {
        name,
        url,
        cardinality: (element.cardinality.min, max),
        value_type,
        description: element.short.clone().or_else(|| element.definition.clone()),
    })
}

/// Extract the extension URL from a sliced extension element.
///
/// The URL is typically found as a fixed value on the "url" child element.
fn extract_extension_url(element: &super::ElementDefinition) -> Option<String> {
    // Check if the element itself has a fixed extension URL
    if let Some(fixed) = &element.fixed
        && let Some(url) = fixed.get("url").and_then(|v| v.as_str())
    {
        return Some(url.to_string());
    }

    // Look for url child element with fixed value
    for child in &element.children {
        if child.path.ends_with(".url")
            && let Some(fixed) = &child.fixed
            && let Some(url) = fixed.as_str()
        {
            return Some(url.to_string());
        }
    }

    None
}

/// Determine the value type of an extension from its element definition.
fn determine_extension_value_type(element: &super::ElementDefinition) -> ExtensionValueType {
    // Look for value[x] child element
    for child in &element.children {
        if child.path.ends_with(".value[x]") || child.path.contains(".value") {
            if child.types.len() > 1 {
                // Choice type
                let type_codes: Vec<String> = child.types.iter().map(|t| t.code.clone()).collect();
                return ExtensionValueType::Choice(type_codes);
            } else if let Some(type_info) = child.types.first() {
                // Single type
                let code = &type_info.code;
                return if is_primitive_type(code) {
                    ExtensionValueType::Simple(code.clone())
                } else {
                    ExtensionValueType::Complex(code.clone())
                };
            }
        }
    }

    // Check if it has nested extensions (sub-extensions)
    let has_nested_extensions = element
        .children
        .iter()
        .any(|child| child.path.ends_with(".extension") && child.slice_name.is_some());

    if has_nested_extensions {
        // Extract nested extensions recursively
        let nested = extract_profile_extensions(&element.children);
        return ExtensionValueType::Nested(nested);
    }

    // Default to string if we can't determine
    ExtensionValueType::Simple("string".to_string())
}

/// Check if a FHIR type code represents a primitive type.
fn is_primitive_type(type_code: &str) -> bool {
    matches!(
        type_code,
        "string"
            | "boolean"
            | "integer"
            | "decimal"
            | "uri"
            | "url"
            | "canonical"
            | "base64Binary"
            | "instant"
            | "date"
            | "dateTime"
            | "time"
            | "code"
            | "oid"
            | "id"
            | "markdown"
            | "unsignedInt"
            | "positiveInt"
            | "uuid"
            | "xhtml"
    )
}

/// Collect all fixed values from elements in the tree.
fn collect_fixed_values(elements: &[super::ElementDefinition]) -> HashMap<String, FixedValue> {
    let mut result = HashMap::new();

    fn traverse(elements: &[super::ElementDefinition], result: &mut HashMap<String, FixedValue>) {
        for element in elements {
            if let Some(value) = &element.fixed {
                result.insert(
                    element.path.clone(),
                    FixedValue {
                        path: element.path.clone(),
                        value: value.clone(),
                    },
                );
            }

            // Recursively check children
            traverse(&element.children, result);
        }
    }

    traverse(elements, &mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        ElementCardinality, ElementDefinition, ElementMax, ProfileLineage, ResourceKind,
    };

    #[test]
    fn test_profile_from_constraint_derivation() {
        let resource = ResourceDefinition {
            id: "us-core-patient".to_string(),
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
            name: Some("USCorePatientProfile".to_string()),
            title: Some("US Core Patient Profile".to_string()),
            description: Some("Defines constraints on the Patient resource".to_string()),
            version: Some("5.0.1".to_string()),
            status: Some("active".to_string()),
            kind: ResourceKind::Resource,
            fhir_type: Some("Patient".to_string()),
            date: None,
            lineage: ProfileLineage {
                base_definition: Some(
                    "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                ),
                base_id: Some("Patient".to_string()),
                derivation: Some(Derivation::Constraint),
                type_name: Some("Patient".to_string()),
            },
            elements: vec![],
            extensions: vec![],
            invariants: vec![],
        };

        let profile = ProfileDefinition::from_resource_definition(&resource);
        assert!(profile.is_some());

        let profile = profile.unwrap();
        assert_eq!(
            profile.url,
            "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
        );
        assert_eq!(profile.name, "USCorePatientProfile");
        assert_eq!(profile.base_type, "Patient");
        assert_eq!(
            profile.description,
            Some("Defines constraints on the Patient resource".to_string())
        );
    }

    #[test]
    fn test_non_profile_returns_none() {
        let resource = ResourceDefinition {
            id: "Patient".to_string(),
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            name: Some("Patient".to_string()),
            title: Some("Patient".to_string()),
            description: None,
            version: Some("4.0.1".to_string()),
            status: Some("active".to_string()),
            kind: ResourceKind::Resource,
            fhir_type: Some("Patient".to_string()),
            date: None,
            lineage: ProfileLineage {
                base_definition: Some(
                    "http://hl7.org/fhir/StructureDefinition/DomainResource".to_string(),
                ),
                base_id: None,
                derivation: Some(Derivation::Specialization), // Not a constraint
                type_name: Some("Patient".to_string()),
            },
            elements: vec![],
            extensions: vec![],
            invariants: vec![],
        };

        let profile = ProfileDefinition::from_resource_definition(&resource);
        assert!(profile.is_none());
    }

    #[test]
    fn test_must_support_collection() {
        let elements = vec![
            ElementDefinition {
                id: "Patient.identifier".to_string(),
                path: "Patient.identifier".to_string(),
                slice_name: None,
                short: None,
                definition: None,
                comment: None,
                requirements: None,
                cardinality: ElementCardinality {
                    min: 1,
                    max: ElementMax::Unbounded,
                },
                types: vec![],
                content_reference: None,
                binding: None,
                invariants: vec![],
                fixed: None,
                pattern: None,
                default_value: None,
                example_values: vec![],
                must_support: true,
                is_summary: false,
                slicing: None,
                extension: vec![],
                additional_fields: Default::default(),
                children: vec![],
                parent_path: Some("Patient".to_string()),
                depth: 1,
                is_backbone: false,
            },
            ElementDefinition {
                id: "Patient.name".to_string(),
                path: "Patient.name".to_string(),
                slice_name: None,
                short: None,
                definition: None,
                comment: None,
                requirements: None,
                cardinality: ElementCardinality {
                    min: 1,
                    max: ElementMax::Finite(1),
                },
                types: vec![],
                content_reference: None,
                binding: None,
                invariants: vec![],
                fixed: None,
                pattern: None,
                default_value: None,
                example_values: vec![],
                must_support: false,
                is_summary: false,
                slicing: None,
                extension: vec![],
                additional_fields: Default::default(),
                children: vec![],
                parent_path: Some("Patient".to_string()),
                depth: 1,
                is_backbone: false,
            },
        ];

        let must_support = collect_must_support_elements(&elements);
        assert_eq!(must_support.len(), 1);
        assert_eq!(must_support[0].path, "Patient.identifier");
        assert_eq!(must_support[0].min, 1);
        assert_eq!(must_support[0].max, None); // Unbounded
    }

    #[test]
    fn test_fixed_value_collection() {
        let elements = vec![ElementDefinition {
            id: "Patient.active".to_string(),
            path: "Patient.active".to_string(),
            slice_name: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min: 1,
                max: ElementMax::Finite(1),
            },
            types: vec![],
            content_reference: None,
            binding: None,
            invariants: vec![],
            fixed: Some(serde_json::json!(true)),
            pattern: None,
            default_value: None,
            example_values: vec![],
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: vec![],
            additional_fields: Default::default(),
            children: vec![],
            parent_path: Some("Patient".to_string()),
            depth: 1,
            is_backbone: false,
        }];

        let fixed_values = collect_fixed_values(&elements);
        assert_eq!(fixed_values.len(), 1);
        assert!(fixed_values.contains_key("Patient.active"));
        assert_eq!(
            fixed_values["Patient.active"].value,
            serde_json::json!(true)
        );
    }

    #[test]
    fn test_primitive_type_detection() {
        assert!(is_primitive_type("string"));
        assert!(is_primitive_type("boolean"));
        assert!(is_primitive_type("integer"));
        assert!(is_primitive_type("dateTime"));
        assert!(!is_primitive_type("Coding"));
        assert!(!is_primitive_type("CodeableConcept"));
        assert!(!is_primitive_type("Reference"));
    }
}
