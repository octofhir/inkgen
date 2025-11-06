//! Profile constraint handling for TypeScript generation.
//!
//! This module provides utilities for generating TypeScript interfaces from FHIR profiles
//! that constrain base resources with additional rules like mustSupport, fixed values,
//! and tightened cardinality.

use inkgen_core::ir::{Derivation, ElementDefinition, ResourceDefinition};

/// Information about a FHIR profile for TypeScript generation.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// TypeScript type name for the profile
    pub type_name: String,
    /// Canonical URL of the profile
    pub canonical_url: String,
    /// Base resource type (e.g., "Patient", "Observation")
    pub base_type: String,
    /// Profile title
    pub title: Option<String>,
    /// Profile description
    pub description: Option<String>,
    /// Elements with mustSupport flag
    pub must_support_elements: Vec<String>,
    /// Elements with fixed values
    pub fixed_elements: Vec<FixedElement>,
    /// Elements with tightened cardinality
    pub constrained_elements: Vec<ConstrainedElement>,
}

/// Represents an element with a fixed value in a profile.
#[derive(Debug, Clone)]
pub struct FixedElement {
    /// Element path (e.g., "Patient.active")
    pub path: String,
    /// TypeScript field name
    pub field_name: String,
    /// Fixed value as TypeScript literal
    pub fixed_value: String,
    /// Type of the fixed value
    pub value_type: String,
}

/// Represents an element with tightened cardinality in a profile.
#[derive(Debug, Clone)]
pub struct ConstrainedElement {
    /// Element path
    pub path: String,
    /// TypeScript field name
    pub field_name: String,
    /// Minimum cardinality (0 or 1+)
    pub min: u32,
    /// Maximum cardinality
    pub max: String,
    /// Whether this makes an optional field required
    pub makes_required: bool,
}

impl ProfileInfo {
    /// Creates a ProfileInfo from a ResourceDefinition that represents a profile.
    ///
    /// Returns None if the resource is not a profile (constraint derivation).
    pub fn from_resource_definition(definition: &ResourceDefinition) -> Option<Self> {
        // Only process profiles (derivation = constraint)
        if !matches!(
            definition.lineage.derivation,
            Some(Derivation::Constraint)
        ) {
            return None;
        }

        let base_type = definition
            .lineage
            .base_id
            .clone()
            .or_else(|| definition.lineage.type_name.clone())?;

        let type_name = definition
            .name
            .clone()
            .unwrap_or_else(|| definition.id.clone());

        let mut must_support_elements = Vec::new();
        let mut fixed_elements = Vec::new();
        let mut constrained_elements = Vec::new();

        extract_constraints(
            &definition.elements,
            &base_type,
            &mut must_support_elements,
            &mut fixed_elements,
            &mut constrained_elements,
        );

        Some(Self {
            type_name,
            canonical_url: definition.url.clone(),
            base_type,
            title: definition.title.clone(),
            description: definition.description.clone(),
            must_support_elements,
            fixed_elements,
            constrained_elements,
        })
    }

    /// Generates TypeScript code for this profile.
    ///
    /// Produces an interface that extends the base type with profile constraints.
    pub fn generate_typescript(&self) -> String {
        let mut output = String::new();

        // Add JSDoc comment
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
            output.push_str(&format!(" * @profile {}\n", self.canonical_url));
            output.push_str(" */\n");
        }

        // Generate interface extending base type
        output.push_str(&format!(
            "export interface {} extends {} {{\n",
            self.type_name, self.base_type
        ));

        // Add __profileUrl metadata field
        output.push_str("  /** Profile URL for runtime validation */\n");
        output.push_str(&format!(
            "  readonly __profileUrl: \"{}\";\n",
            self.canonical_url
        ));

        // Add fixed value fields (override with specific literals)
        for fixed in &self.fixed_elements {
            output.push_str(&format!(
                "  /** Fixed value: {} */\n",
                fixed.fixed_value
            ));
            output.push_str(&format!(
                "  {}: {};\n",
                fixed.field_name, fixed.fixed_value
            ));
        }

        // Add constrained fields (override cardinality)
        for constrained in &self.constrained_elements {
            if constrained.makes_required {
                output.push_str(&format!(
                    "  /** Required by profile (min: {}) */\n",
                    constrained.min
                ));
                // Remove optional marker by redefining as required
                output.push_str(&format!("  {}: ", constrained.field_name));
                // Get the type from base, removing optional marker
                output.push_str("NonNullable<");
                output.push_str(&format!("{}['{}']", self.base_type, constrained.field_name));
                output.push_str(">;\n");
            }
        }

        output.push_str("}\n\n");

        // Generate type guard
        output.push_str(&format!(
            "export function is{}(value: {}): value is {} {{\n",
            self.type_name, self.base_type, self.type_name
        ));
        output.push_str(&format!(
            "  return '__profileUrl' in value && value.__profileUrl === '{}';\n",
            self.canonical_url
        ));
        output.push_str("}\n");

        output
    }

    /// Returns true if this profile has any constraints worth generating.
    pub fn has_constraints(&self) -> bool {
        !self.fixed_elements.is_empty()
            || !self.constrained_elements.is_empty()
            || !self.must_support_elements.is_empty()
    }
}

/// Extracts profile constraints from element definitions.
fn extract_constraints(
    elements: &[ElementDefinition],
    base_type: &str,
    must_support: &mut Vec<String>,
    fixed: &mut Vec<FixedElement>,
    constrained: &mut Vec<ConstrainedElement>,
) {
    for element in elements {
        // Skip the root element
        if element.path == base_type {
            continue;
        }

        // Extract mustSupport elements
        if element.must_support {
            must_support.push(element.path.clone());
        }

        // Extract fixed values
        if let Some(fixed_value) = &element.fixed {
            if let Some(ts_value) = json_to_typescript_literal(fixed_value) {
                let field_name = element
                    .path
                    .split('.')
                    .last()
                    .unwrap_or(&element.path)
                    .to_string();

                fixed.push(FixedElement {
                    path: element.path.clone(),
                    field_name,
                    fixed_value: ts_value.clone(),
                    value_type: infer_type_from_value(&ts_value),
                });
            }
        }

        // Extract tightened cardinality (min > 0 makes optional required)
        if element.cardinality.min > 0 {
            let field_name = element
                .path
                .split('.')
                .last()
                .unwrap_or(&element.path)
                .to_string();

            let max = match element.cardinality.max {
                inkgen_core::ir::ElementMax::Unbounded => "*".to_string(),
                inkgen_core::ir::ElementMax::Finite(n) => n.to_string(),
            };

            constrained.push(ConstrainedElement {
                path: element.path.clone(),
                field_name,
                min: element.cardinality.min,
                max,
                makes_required: element.cardinality.min > 0,
            });
        }

        // Recursively process children
        extract_constraints(
            &element.children,
            base_type,
            must_support,
            fixed,
            constrained,
        );
    }
}

/// Converts a JSON value to a TypeScript literal.
fn json_to_typescript_literal(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(format!("\"{}\"", escape_string(s))),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => Some("null".to_string()),
        serde_json::Value::Array(arr) if arr.is_empty() => Some("[]".to_string()),
        serde_json::Value::Object(obj) if obj.is_empty() => Some("{}".to_string()),
        _ => None, // Complex values not supported as literals
    }
}

/// Infers TypeScript type from a literal value string.
fn infer_type_from_value(value: &str) -> String {
    if value.starts_with('"') {
        "string".to_string()
    } else if value == "true" || value == "false" {
        "boolean".to_string()
    } else if value == "null" {
        "null".to_string()
    } else if value.parse::<f64>().is_ok() {
        "number".to_string()
    } else {
        "unknown".to_string()
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

/// Extracts profile type name from a canonical URL.
///
/// # Examples
///
/// ```
/// # use inkgen_typescript::profiles::profile_url_to_type_name;
/// assert_eq!(
///     profile_url_to_type_name("http://hl7.org/fhir/StructureDefinition/us-core-patient"),
///     "UsCorePatient"
/// );
/// ```
pub fn profile_url_to_type_name(url: &str) -> String {
    let segment = url
        .trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("Profile");

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
    use inkgen_core::ir::{ElementCardinality, ElementMax, ProfileLineage, ResourceKind};
    use indexmap::IndexMap;
    use serde_json::json;

    #[test]
    fn test_profile_url_to_type_name() {
        assert_eq!(
            profile_url_to_type_name("http://hl7.org/fhir/StructureDefinition/us-core-patient"),
            "UsCorePatient"
        );
        assert_eq!(
            profile_url_to_type_name("http://example.org/fhir/StructureDefinition/my_profile"),
            "MyProfile"
        );
    }

    #[test]
    fn test_json_to_typescript_literal() {
        assert_eq!(
            json_to_typescript_literal(&json!("test")),
            Some("\"test\"".to_string())
        );
        assert_eq!(
            json_to_typescript_literal(&json!(42)),
            Some("42".to_string())
        );
        assert_eq!(
            json_to_typescript_literal(&json!(true)),
            Some("true".to_string())
        );
        assert_eq!(
            json_to_typescript_literal(&json!(null)),
            Some("null".to_string())
        );
    }

    #[test]
    fn test_infer_type_from_value() {
        assert_eq!(infer_type_from_value("\"test\""), "string");
        assert_eq!(infer_type_from_value("42"), "number");
        assert_eq!(infer_type_from_value("true"), "boolean");
        assert_eq!(infer_type_from_value("false"), "boolean");
        assert_eq!(infer_type_from_value("null"), "null");
    }

    #[test]
    fn test_profile_info_from_non_profile() {
        let definition = ResourceDefinition {
            id: "Patient".to_string(),
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            name: Some("Patient".to_string()),
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: Some("Patient".to_string()),
            date: None,
            lineage: ProfileLineage {
                base_definition: None,
                base_id: None,
                derivation: Some(Derivation::Specialization),
                type_name: None,
            },
            elements: vec![],
            extensions: vec![],
            invariants: vec![],
        };

        let profile = ProfileInfo::from_resource_definition(&definition);
        assert!(profile.is_none());
    }

    #[test]
    fn test_profile_info_from_constraint_profile() {
        let definition = ResourceDefinition {
            id: "us-core-patient".to_string(),
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
            name: Some("USCorePatientProfile".to_string()),
            title: Some("US Core Patient Profile".to_string()),
            description: Some("Defines constraints on Patient resource".to_string()),
            version: None,
            status: None,
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
            elements: vec![
                create_test_element("Patient", 0, 1, false, None),
                create_test_element("Patient.identifier", 1, usize::MAX, true, None),
                create_test_element("Patient.active", 0, 1, false, Some(json!(true))),
            ],
            extensions: vec![],
            invariants: vec![],
        };

        let profile = ProfileInfo::from_resource_definition(&definition).unwrap();
        assert_eq!(profile.type_name, "USCorePatientProfile");
        assert_eq!(profile.base_type, "Patient");
        assert_eq!(profile.title, Some("US Core Patient Profile".to_string()));
        assert!(profile.must_support_elements.contains(&"Patient.identifier".to_string()));
        assert_eq!(profile.fixed_elements.len(), 1);
        assert_eq!(profile.fixed_elements[0].field_name, "active");
        assert_eq!(profile.fixed_elements[0].fixed_value, "true");
        assert_eq!(profile.constrained_elements.len(), 1);
        assert!(profile.constrained_elements[0].makes_required);
    }

    #[test]
    fn test_generate_typescript() {
        let profile = ProfileInfo {
            type_name: "USCorePatient".to_string(),
            canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
                .to_string(),
            base_type: "Patient".to_string(),
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec!["Patient.identifier".to_string()],
            fixed_elements: vec![FixedElement {
                path: "Patient.active".to_string(),
                field_name: "active".to_string(),
                fixed_value: "true".to_string(),
                value_type: "boolean".to_string(),
            }],
            constrained_elements: vec![ConstrainedElement {
                path: "Patient.name".to_string(),
                field_name: "name".to_string(),
                min: 1,
                max: "*".to_string(),
                makes_required: true,
            }],
        };

        let output = profile.generate_typescript();

        assert!(output.contains("export interface USCorePatient extends Patient"));
        assert!(output.contains("readonly __profileUrl"));
        assert!(output.contains("active: true"));
        assert!(output.contains("NonNullable<Patient['name']>"));
        assert!(output.contains("export function isUSCorePatient"));
    }

    fn create_test_element(
        path: &str,
        min: u32,
        max: usize,
        must_support: bool,
        fixed: Option<serde_json::Value>,
    ) -> ElementDefinition {
        ElementDefinition {
            id: path.to_string(),
            path: path.to_string(),
            slice_name: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min,
                max: if max == usize::MAX {
                    ElementMax::Unbounded
                } else {
                    ElementMax::Finite(max as u32)
                },
            },
            types: vec![],
            content_reference: None,
            binding: None,
            invariants: vec![],
            fixed,
            pattern: None,
            default_value: None,
            example_values: vec![],
            must_support,
            is_summary: false,
            slicing: None,
            extension: vec![],
            additional_fields: IndexMap::new(),
            children: vec![],
            parent_path: None,
            depth: 0,
            is_backbone: false,
        }
    }
}
