//! Nested type generation for BackboneElements.
//!
//! This module provides utilities for collecting nested types from FHIR StructureDefinitions
//! and generating dedicated TypeScript interfaces for BackboneElements.

use inkgen_core::ir::{ElementDefinition, ResourceDefinition};

/// Information about a nested type to be generated.
#[derive(Debug, Clone)]
pub struct NestedTypeInfo {
    /// Generated TypeScript type name (e.g., "PatientContact")
    pub type_name: String,
    /// Element path in the FHIR structure (e.g., "Patient.contact")
    pub element_path: String,
    /// Base type (typically "BackboneElement" or "Element")
    pub base_type: String,
    /// Child elements that become fields in this type
    pub children: Vec<ElementDefinition>,
    /// Documentation comment from the element
    pub doc_comment: Option<String>,
    /// Nesting depth (0 for root-level)
    pub depth: usize,
}

/// Collector for nested types within a resource definition.
pub struct NestedTypeCollector<'a> {
    resource_name: String,
    definition: &'a ResourceDefinition,
}

impl<'a> NestedTypeCollector<'a> {
    /// Creates a new nested type collector for the given resource.
    pub fn new(definition: &'a ResourceDefinition) -> Self {
        let resource_name = definition
            .name
            .clone()
            .unwrap_or_else(|| definition.id.clone());
        Self {
            resource_name,
            definition,
        }
    }

    /// Collects all nested types (BackboneElements) from the resource definition.
    ///
    /// Returns a list of nested type information structures, sorted by depth
    /// (shallowest first) to ensure parent types are generated before children.
    pub fn collect(&self) -> Vec<NestedTypeInfo> {
        let mut nested_types = Vec::new();

        // Traverse all top-level elements and their children
        for element in &self.definition.elements {
            self.collect_from_element(element, &mut nested_types);
        }

        // Sort by depth to ensure parent types are defined before children
        nested_types.sort_by_key(|info| info.depth);

        nested_types
    }

    /// Recursively collects nested types from an element and its children.
    fn collect_from_element(&self, element: &ElementDefinition, acc: &mut Vec<NestedTypeInfo>) {
        // Skip the root element - it's the main structure, not a nested type
        let is_root = element.path == self.resource_name || element.path.split('.').count() == 1;

        // If this element is a BackboneElement with children, it needs a dedicated type
        if !is_root && element.is_backbone && !element.children.is_empty() {
            let type_name = build_composite_name(&self.resource_name, &element.path);
            let base_type = element
                .types
                .first()
                .map(|t| t.code.clone())
                .unwrap_or_else(|| "BackboneElement".to_string());

            let doc_comment = element.short.clone().or_else(|| element.definition.clone());

            acc.push(NestedTypeInfo {
                type_name,
                element_path: element.path.clone(),
                base_type,
                children: element.children.clone(),
                doc_comment,
                depth: element.depth,
            });
        }

        // Recursively process children
        for child in &element.children {
            self.collect_from_element(child, acc);
        }
    }
}

/// Builds a composite TypeScript type name from a resource name and element path.
///
/// The function removes the resource prefix from the path and converts each segment
/// to PascalCase, then concatenates them.
///
/// # Examples
///
/// ```
/// # use inkgen_typescript::nested::build_composite_name;
/// assert_eq!(
///     build_composite_name("Patient", "Patient.contact"),
///     "PatientContact"
/// );
/// assert_eq!(
///     build_composite_name("Observation", "Observation.component.referenceRange"),
///     "ObservationComponentReferenceRange"
/// );
/// ```
pub fn build_composite_name(_resource_name: &str, element_path: &str) -> String {
    let segments: Vec<&str> = element_path.split('.').collect();

    // Skip the resource prefix and convert remaining segments to PascalCase
    let type_segments: Vec<String> = segments
        .into_iter()
        .map(|segment| {
            // Handle [x] suffix in choice types
            let segment = segment.trim_end_matches("[x]");
            pascal_case(segment)
        })
        .collect();

    type_segments.join("")
}

/// Converts a string to PascalCase by splitting on non-alphanumeric characters.
fn pascal_case(value: &str) -> String {
    split_tokens(value)
        .into_iter()
        .map(|token| {
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

/// Splits a string into alphanumeric tokens.
fn split_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if tokens.is_empty() {
        tokens.push(value.to_string());
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use inkgen_core::ir::{ElementCardinality, ElementMax, ProfileLineage, ResourceKind};

    #[test]
    fn test_build_composite_name_simple() {
        assert_eq!(
            build_composite_name("Patient", "Patient.contact"),
            "PatientContact"
        );
    }

    #[test]
    fn test_build_composite_name_nested() {
        assert_eq!(
            build_composite_name("Observation", "Observation.component.referenceRange"),
            "ObservationComponentReferenceRange"
        );
    }

    #[test]
    fn test_build_composite_name_with_choice() {
        assert_eq!(
            build_composite_name(
                "MedicationRequest",
                "MedicationRequest.dosageInstruction[x]"
            ),
            "MedicationRequestDosageInstruction"
        );
    }

    #[test]
    fn test_build_composite_name_snake_case() {
        assert_eq!(
            build_composite_name(
                "StructureDefinition",
                "StructureDefinition.snapshot_element"
            ),
            "StructureDefinitionSnapshotElement"
        );
    }

    #[test]
    fn test_collect_no_nested_types() {
        let definition = ResourceDefinition {
            id: "SimpleResource".to_string(),
            url: "http://example.org/SimpleResource".to_string(),
            name: Some("SimpleResource".to_string()),
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: Some("SimpleResource".to_string()),
            date: None,
            lineage: ProfileLineage::default(),
            elements: vec![ElementDefinition {
                id: "SimpleResource.field".to_string(),
                path: "SimpleResource.field".to_string(),
                slice_name: None,
                short: Some("A simple field".to_string()),
                definition: None,
                comment: None,
                requirements: None,
                cardinality: ElementCardinality {
                    min: 0,
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
                additional_fields: IndexMap::new(),
                children: vec![],
                parent_path: Some("SimpleResource".to_string()),
                depth: 1,
                is_backbone: false,
            }],
            extensions: vec![],
            invariants: vec![],
        };

        let collector = NestedTypeCollector::new(&definition);
        let nested = collector.collect();

        assert_eq!(nested.len(), 0);
    }

    #[test]
    fn test_collect_single_backbone() {
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
            lineage: ProfileLineage::default(),
            elements: vec![ElementDefinition {
                id: "Patient.contact".to_string(),
                path: "Patient.contact".to_string(),
                slice_name: None,
                short: Some("A contact party for the patient".to_string()),
                definition: None,
                comment: None,
                requirements: None,
                cardinality: ElementCardinality {
                    min: 0,
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
                must_support: false,
                is_summary: false,
                slicing: None,
                extension: vec![],
                additional_fields: IndexMap::new(),
                children: vec![ElementDefinition {
                    id: "Patient.contact.name".to_string(),
                    path: "Patient.contact.name".to_string(),
                    slice_name: None,
                    short: Some("Name of the contact".to_string()),
                    definition: None,
                    comment: None,
                    requirements: None,
                    cardinality: ElementCardinality {
                        min: 0,
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
                    additional_fields: IndexMap::new(),
                    children: vec![],
                    parent_path: Some("Patient.contact".to_string()),
                    depth: 2,
                    is_backbone: false,
                }],
                parent_path: Some("Patient".to_string()),
                depth: 1,
                is_backbone: true,
            }],
            extensions: vec![],
            invariants: vec![],
        };

        let collector = NestedTypeCollector::new(&definition);
        let nested = collector.collect();

        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].type_name, "PatientContact");
        assert_eq!(nested[0].element_path, "Patient.contact");
        assert_eq!(nested[0].children.len(), 1);
        assert_eq!(nested[0].children[0].path, "Patient.contact.name");
        assert_eq!(nested[0].depth, 1);
    }

    #[test]
    fn test_collect_nested_backbone() {
        let definition = ResourceDefinition {
            id: "Observation".to_string(),
            url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
            name: Some("Observation".to_string()),
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: Some("Observation".to_string()),
            date: None,
            lineage: ProfileLineage::default(),
            elements: vec![ElementDefinition {
                id: "Observation.component".to_string(),
                path: "Observation.component".to_string(),
                slice_name: None,
                short: Some("Component results".to_string()),
                definition: None,
                comment: None,
                requirements: None,
                cardinality: ElementCardinality {
                    min: 0,
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
                must_support: false,
                is_summary: false,
                slicing: None,
                extension: vec![],
                additional_fields: IndexMap::new(),
                children: vec![ElementDefinition {
                    id: "Observation.component.referenceRange".to_string(),
                    path: "Observation.component.referenceRange".to_string(),
                    slice_name: None,
                    short: Some("Reference range for component".to_string()),
                    definition: None,
                    comment: None,
                    requirements: None,
                    cardinality: ElementCardinality {
                        min: 0,
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
                    must_support: false,
                    is_summary: false,
                    slicing: None,
                    extension: vec![],
                    additional_fields: IndexMap::new(),
                    children: vec![ElementDefinition {
                        id: "Observation.component.referenceRange.low".to_string(),
                        path: "Observation.component.referenceRange.low".to_string(),
                        slice_name: None,
                        short: Some("Low bound".to_string()),
                        definition: None,
                        comment: None,
                        requirements: None,
                        cardinality: ElementCardinality {
                            min: 0,
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
                        additional_fields: IndexMap::new(),
                        children: vec![],
                        parent_path: Some("Observation.component.referenceRange".to_string()),
                        depth: 3,
                        is_backbone: false,
                    }],
                    parent_path: Some("Observation.component".to_string()),
                    depth: 2,
                    is_backbone: true,
                }],
                parent_path: Some("Observation".to_string()),
                depth: 1,
                is_backbone: true,
            }],
            extensions: vec![],
            invariants: vec![],
        };

        let collector = NestedTypeCollector::new(&definition);
        let nested = collector.collect();

        // Should collect both "Observation.component" and "Observation.component.referenceRange"
        assert_eq!(nested.len(), 2);

        // First should be component (depth 1)
        assert_eq!(nested[0].type_name, "ObservationComponent");
        assert_eq!(nested[0].element_path, "Observation.component");
        assert_eq!(nested[0].depth, 1);

        // Second should be referenceRange (depth 2)
        assert_eq!(nested[1].type_name, "ObservationComponentReferenceRange");
        assert_eq!(
            nested[1].element_path,
            "Observation.component.referenceRange"
        );
        assert_eq!(nested[1].depth, 2);
    }
}
