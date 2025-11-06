//! Profile lineage resolution and element snapshot merging.
//!
//! This module provides functionality for:
//! - Walking the baseDefinition chain from profiles to their root ancestors
//! - Merging element snapshots from base to derived (specialization and constraint)
//! - Tracking inheritance chains for profile-aware code generation

use indexmap::IndexMap;

use crate::error::CoreResult;
use crate::ir::{ElementDefinition, ResourceDefinition};

/// Represents the inheritance chain for a profile or specialization.
#[derive(Debug, Clone)]
pub struct ProfileChain {
    /// Canonical URL of the profile/resource
    pub canonical_url: String,
    /// Ordered list of ancestors (nearest parent first, root last)
    pub ancestors: Vec<ProfileAncestor>,
}

/// An ancestor in the profile chain.
#[derive(Debug, Clone)]
pub struct ProfileAncestor {
    /// Canonical URL of the ancestor
    pub canonical_url: String,
    /// ID of the ancestor StructureDefinition
    pub id: String,
    /// Type of derivation (constraint or specialization)
    pub derivation_type: Option<String>,
}

impl ProfileChain {
    /// Creates a new profile chain for a resource definition.
    pub fn from_resource(resource: &ResourceDefinition) -> Self {
        let mut ancestors = Vec::new();

        if let Some(base_url) = &resource.lineage.base_definition {
            ancestors.push(ProfileAncestor {
                canonical_url: base_url.clone(),
                id: resource
                    .lineage
                    .base_id
                    .clone()
                    .unwrap_or_else(|| extract_id_from_url(base_url)),
                derivation_type: resource
                    .lineage
                    .derivation
                    .map(|d| format!("{:?}", d).to_lowercase()),
            });
        }

        ProfileChain {
            canonical_url: resource.url.clone(),
            ancestors,
        }
    }

    /// Returns true if this definition has a base (is a profile or specialization).
    pub fn has_base(&self) -> bool {
        !self.ancestors.is_empty()
    }

    /// Returns the immediate parent URL, if any.
    pub fn immediate_parent(&self) -> Option<&str> {
        self.ancestors.first().map(|a| a.canonical_url.as_str())
    }

    /// Returns the root ancestor URL (furthest parent in chain).
    pub fn root_ancestor(&self) -> Option<&str> {
        self.ancestors.last().map(|a| a.canonical_url.as_str())
    }
}

/// Merges element snapshots from base to derived profile.
///
/// This implements a pattern similar to Object.assign in JavaScript:
/// - Start with base elements
/// - Override/add elements from derived profile
/// - Preserve element order from derived profile where present
///
/// # Arguments
///
/// * `base_elements` - Elements from the base StructureDefinition
/// * `derived_elements` - Elements from the derived StructureDefinition
///
/// # Returns
///
/// Merged element list with derived elements taking precedence
pub fn merge_element_snapshots(
    base_elements: &[ElementDefinition],
    derived_elements: &[ElementDefinition],
) -> Vec<ElementDefinition> {
    // Build a map of base elements by path for quick lookup
    let mut base_map: IndexMap<String, ElementDefinition> = base_elements
        .iter()
        .map(|elem| (elem.path.clone(), elem.clone()))
        .collect();

    // Override with derived elements
    for derived_elem in derived_elements {
        base_map.insert(derived_elem.path.clone(), derived_elem.clone());
    }

    // Return elements in stable order (insertion order via IndexMap)
    base_map.into_values().collect()
}

/// Resolves a complete profile chain by walking baseDefinition links.
///
/// This would require access to the package cache to load base definitions.
/// For now, this is a stub that will be enhanced when we integrate with
/// the actual package resolution system.
///
/// # Arguments
///
/// * `resource` - The starting resource definition
///
/// # Returns
///
/// Result containing the complete profile chain
pub fn resolve_full_chain(resource: &ResourceDefinition) -> CoreResult<ProfileChain> {
    // For now, just return the immediate chain
    // In a future enhancement, this would:
    // 1. Look up the base definition from package cache
    // 2. Recursively build the full chain
    // 3. Return all ancestors up to the root
    Ok(ProfileChain::from_resource(resource))
}

/// Extracts the StructureDefinition ID from a canonical URL.
///
/// # Arguments
///
/// * `url` - Canonical URL (e.g., "http://hl7.org/fhir/StructureDefinition/Patient")
///
/// # Returns
///
/// The ID extracted from the URL (e.g., "Patient")
fn extract_id_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Derivation, ElementCardinality, ElementMax, ProfileLineage, ResourceDefinition,
        ResourceKind,
    };

    fn make_test_element(path: &str, short: Option<&str>) -> ElementDefinition {
        ElementDefinition {
            id: path.to_string(),
            path: path.to_string(),
            slice_name: None,
            short: short.map(|s| s.to_string()),
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Unbounded,
            },
            types: Vec::new(),
            content_reference: None,
            binding: None,
            invariants: Vec::new(),
            fixed: None,
            pattern: None,
            default_value: None,
            example_values: Vec::new(),
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: Vec::new(),
            additional_fields: IndexMap::new(),
            children: Vec::new(),
            parent_path: None,
            depth: 0,
            is_backbone: false,
        }
    }

    #[test]
    fn test_profile_chain_from_resource() {
        let resource = ResourceDefinition {
            id: "us-core-patient".to_string(),
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
            name: Some("USCorePatient".to_string()),
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: Some("Patient".to_string()),
            date: None,
            lineage: ProfileLineage {
                base_definition: Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
                base_id: Some("Patient".to_string()),
                derivation: Some(Derivation::Constraint),
                type_name: Some("Patient".to_string()),
            },
            elements: Vec::new(),
            extensions: Vec::new(),
            invariants: Vec::new(),
        };

        let chain = ProfileChain::from_resource(&resource);

        assert_eq!(chain.canonical_url, "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient");
        assert!(chain.has_base());
        assert_eq!(chain.immediate_parent(), Some("http://hl7.org/fhir/StructureDefinition/Patient"));
    }

    #[test]
    fn test_merge_element_snapshots() {
        let base_elements = vec![
            make_test_element("Patient", Some("Base patient")),
            make_test_element("Patient.id", Some("Base id")),
            make_test_element("Patient.name", Some("Base name")),
        ];

        let derived_elements = vec![
            make_test_element("Patient", Some("Derived patient")),
            make_test_element("Patient.id", Some("Derived id")),
            // Patient.name not overridden
            make_test_element("Patient.identifier", Some("New element")),
        ];

        let merged = merge_element_snapshots(&base_elements, &derived_elements);

        assert_eq!(merged.len(), 4);

        // Patient and Patient.id should have derived values
        let patient_elem = merged.iter().find(|e| e.path == "Patient").unwrap();
        assert_eq!(patient_elem.short.as_deref(), Some("Derived patient"));

        let id_elem = merged.iter().find(|e| e.path == "Patient.id").unwrap();
        assert_eq!(id_elem.short.as_deref(), Some("Derived id"));

        // Patient.name should still have base value
        let name_elem = merged.iter().find(|e| e.path == "Patient.name").unwrap();
        assert_eq!(name_elem.short.as_deref(), Some("Base name"));

        // Patient.identifier should be present (new element)
        let identifier_elem = merged.iter().find(|e| e.path == "Patient.identifier").unwrap();
        assert_eq!(identifier_elem.short.as_deref(), Some("New element"));
    }

    #[test]
    fn test_extract_id_from_url() {
        assert_eq!(
            extract_id_from_url("http://hl7.org/fhir/StructureDefinition/Patient"),
            "Patient"
        );
        assert_eq!(
            extract_id_from_url("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"),
            "us-core-patient"
        );
        assert_eq!(extract_id_from_url("Patient"), "Patient");
    }
}
