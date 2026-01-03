//! FHIR type registry for discovering types from packages at runtime.
//!
//! This module provides a registry that classifies FHIR types based on their
//! StructureDefinition kind, eliminating the need for hardcoded type lists.

use std::collections::{HashMap, HashSet};

use crate::{StructureKind, StructureSummary};

/// Information about a discovered FHIR type.
#[derive(Debug, Clone)]
pub struct FhirTypeInfo {
    /// The type name (e.g., "Patient", "Address", "string").
    pub name: String,
    /// The canonical URL of the StructureDefinition.
    pub url: String,
    /// The classification of this type.
    pub kind: StructureKind,
    /// The type code (for profiles, this is the base type).
    pub type_code: Option<String>,
    /// The package this type comes from.
    pub package_name: String,
}

/// Registry of all discovered FHIR types from loaded packages.
///
/// Built from `StructureSummary` data during package discovery,
/// this registry provides runtime type classification without hardcoded lists.
///
/// This is different from the import `TypeRegistry` which tracks file locations
/// for generating import statements.
#[derive(Debug, Clone, Default)]
pub struct FhirTypeRegistry {
    /// Map from type name to type info.
    types: HashMap<String, FhirTypeInfo>,
    /// Map from canonical URL to type name.
    by_url: HashMap<String, String>,
    /// Types grouped by kind.
    by_kind: HashMap<StructureKind, HashSet<String>>,
}

impl FhirTypeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a registry from a collection of structure summaries.
    pub fn from_summaries(summaries: &[StructureSummary]) -> Self {
        let mut registry = Self::new();

        for summary in summaries {
            // Use type_code if available, otherwise extract from URL or use name
            let type_name = summary
                .type_code
                .clone()
                .or_else(|| summary.name.clone())
                .unwrap_or_else(|| {
                    // Extract name from URL as fallback
                    summary
                        .canonical_url
                        .rsplit('/')
                        .next()
                        .unwrap_or("Unknown")
                        .to_string()
                });

            let info = FhirTypeInfo {
                name: type_name.clone(),
                url: summary.canonical_url.clone(),
                kind: summary.kind,
                type_code: summary.type_code.clone(),
                package_name: summary.package.name.clone(),
            };

            // Only insert if not already present (first package wins)
            if !registry.types.contains_key(&type_name) {
                registry.types.insert(type_name.clone(), info);
                registry
                    .by_url
                    .insert(summary.canonical_url.clone(), type_name.clone());
                registry
                    .by_kind
                    .entry(summary.kind)
                    .or_default()
                    .insert(type_name);
            }
        }

        registry
    }

    /// Merge another registry into this one.
    /// Existing entries are not overwritten.
    pub fn merge(&mut self, other: &FhirTypeRegistry) {
        for (name, info) in &other.types {
            if !self.types.contains_key(name) {
                self.types.insert(name.clone(), info.clone());
                self.by_url.insert(info.url.clone(), name.clone());
                self.by_kind
                    .entry(info.kind)
                    .or_default()
                    .insert(name.clone());
            }
        }
    }

    /// Check if a type name is a primitive type.
    pub fn is_primitive(&self, name: &str) -> bool {
        self.types
            .get(name)
            .map(|info| info.kind == StructureKind::PrimitiveType)
            .unwrap_or(false)
    }

    /// Check if a type name is a complex type.
    pub fn is_complex_type(&self, name: &str) -> bool {
        self.types
            .get(name)
            .map(|info| info.kind == StructureKind::ComplexType)
            .unwrap_or(false)
    }

    /// Check if a type name is a base resource.
    pub fn is_base_resource(&self, name: &str) -> bool {
        self.types
            .get(name)
            .map(|info| info.kind == StructureKind::BaseResource)
            .unwrap_or(false)
    }

    /// Get type info by name.
    pub fn get(&self, name: &str) -> Option<&FhirTypeInfo> {
        self.types.get(name)
    }

    /// Get type info by canonical URL.
    pub fn get_by_url(&self, url: &str) -> Option<&FhirTypeInfo> {
        self.by_url.get(url).and_then(|name| self.types.get(name))
    }

    /// Get the canonical URL for a type name.
    pub fn get_url(&self, name: &str) -> Option<&str> {
        self.types.get(name).map(|info| info.url.as_str())
    }

    /// Iterate over all primitive type names.
    pub fn primitives(&self) -> impl Iterator<Item = &str> {
        self.by_kind
            .get(&StructureKind::PrimitiveType)
            .into_iter()
            .flat_map(|set| set.iter().map(String::as_str))
    }

    /// Iterate over all complex type names.
    pub fn complex_types(&self) -> impl Iterator<Item = &str> {
        self.by_kind
            .get(&StructureKind::ComplexType)
            .into_iter()
            .flat_map(|set| set.iter().map(String::as_str))
    }

    /// Iterate over all base resource names.
    pub fn base_resources(&self) -> impl Iterator<Item = &str> {
        self.by_kind
            .get(&StructureKind::BaseResource)
            .into_iter()
            .flat_map(|set| set.iter().map(String::as_str))
    }

    /// Iterate over all types of a specific kind.
    pub fn types_of_kind(&self, kind: StructureKind) -> impl Iterator<Item = &str> {
        self.by_kind
            .get(&kind)
            .into_iter()
            .flat_map(|set| set.iter().map(String::as_str))
    }

    /// Get all type names in the registry.
    pub fn all_types(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(String::as_str)
    }

    /// Check if the registry contains a type.
    pub fn contains(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Get the number of types in the registry.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Get counts by kind.
    pub fn counts_by_kind(&self) -> HashMap<StructureKind, usize> {
        self.by_kind
            .iter()
            .map(|(kind, set)| (*kind, set.len()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PackageId;

    fn make_summary(name: &str, url: &str, kind: StructureKind) -> StructureSummary {
        StructureSummary {
            canonical_url: url.to_string(),
            name: Some(name.to_string()),
            type_code: Some(name.to_string()),
            title: None,
            version: None,
            status: None,
            package: PackageId::new("test-package", "1.0.0"),
            kind,
        }
    }

    #[test]
    fn test_from_summaries() {
        let summaries = vec![
            make_summary(
                "string",
                "http://hl7.org/fhir/StructureDefinition/string",
                StructureKind::PrimitiveType,
            ),
            make_summary(
                "Address",
                "http://hl7.org/fhir/StructureDefinition/Address",
                StructureKind::ComplexType,
            ),
            make_summary(
                "Patient",
                "http://hl7.org/fhir/StructureDefinition/Patient",
                StructureKind::BaseResource,
            ),
        ];

        let registry = FhirTypeRegistry::from_summaries(&summaries);

        assert_eq!(registry.len(), 3);
        assert!(registry.is_primitive("string"));
        assert!(!registry.is_primitive("Address"));
        assert!(registry.is_complex_type("Address"));
        assert!(registry.is_base_resource("Patient"));
    }

    #[test]
    fn test_primitives_iterator() {
        let summaries = vec![
            make_summary(
                "string",
                "http://hl7.org/fhir/StructureDefinition/string",
                StructureKind::PrimitiveType,
            ),
            make_summary(
                "boolean",
                "http://hl7.org/fhir/StructureDefinition/boolean",
                StructureKind::PrimitiveType,
            ),
            make_summary(
                "Address",
                "http://hl7.org/fhir/StructureDefinition/Address",
                StructureKind::ComplexType,
            ),
        ];

        let registry = FhirTypeRegistry::from_summaries(&summaries);
        let primitives: HashSet<_> = registry.primitives().collect();

        assert_eq!(primitives.len(), 2);
        assert!(primitives.contains("string"));
        assert!(primitives.contains("boolean"));
        assert!(!primitives.contains("Address"));
    }

    #[test]
    fn test_get_url() {
        let summaries = vec![make_summary(
            "Patient",
            "http://hl7.org/fhir/StructureDefinition/Patient",
            StructureKind::BaseResource,
        )];

        let registry = FhirTypeRegistry::from_summaries(&summaries);

        assert_eq!(
            registry.get_url("Patient"),
            Some("http://hl7.org/fhir/StructureDefinition/Patient")
        );
        assert_eq!(registry.get_url("Unknown"), None);
    }

    #[test]
    fn test_merge() {
        let summaries1 = vec![make_summary(
            "string",
            "http://hl7.org/fhir/StructureDefinition/string",
            StructureKind::PrimitiveType,
        )];
        let summaries2 = vec![make_summary(
            "Address",
            "http://hl7.org/fhir/StructureDefinition/Address",
            StructureKind::ComplexType,
        )];

        let mut registry = FhirTypeRegistry::from_summaries(&summaries1);
        let other = FhirTypeRegistry::from_summaries(&summaries2);
        registry.merge(&other);

        assert_eq!(registry.len(), 2);
        assert!(registry.is_primitive("string"));
        assert!(registry.is_complex_type("Address"));
    }
}
