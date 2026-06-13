//! Typed Reference<T> helper generation for FHIR resources.
//!
//! This module generates TypeScript utilities for working with FHIR References
//! in a type-safe manner, including resource type checking and reference creation.

use serde::Serialize;
use std::collections::HashSet;

/// Configuration for reference helper generation
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceConfig {
    /// Generate type guards for reference checking
    pub type_guards: bool,
    /// Generate factory functions for creating references
    pub factories: bool,
    /// Generate reference resolution helpers
    pub resolution_helpers: bool,
}

impl Default for ReferenceConfig {
    fn default() -> Self {
        Self {
            type_guards: true,
            factories: true,
            resolution_helpers: true,
        }
    }
}

/// Information about resource types for reference generation
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceHelpers {
    /// List of resource type names (e.g., ["Patient", "Observation"])
    pub resource_types: Vec<String>,
    /// Whether to generate type guards
    pub has_type_guards: bool,
    /// Whether to generate factories
    pub has_factories: bool,
    /// Whether to generate resolution helpers
    pub has_resolution: bool,
}

impl ReferenceHelpers {
    /// Creates reference helpers for a set of resource types
    pub fn new(resource_types: Vec<String>, config: &ReferenceConfig) -> Self {
        Self {
            resource_types,
            has_type_guards: config.type_guards,
            has_factories: config.factories,
            has_resolution: config.resolution_helpers,
        }
    }

    /// Generate TypeScript code for typed Reference<T> type definition
    ///
    /// Creates:
    /// ```typescript
    /// export type TypedReference<T extends string> = Reference & {
    ///   reference: `${T}/${string}`;
    ///   type?: T;
    /// };
    /// ```
    pub fn generate_typed_reference_type() -> String {
        r#"/**
 * A FHIR Reference with compile-time resource type checking
 * @template T - The resource type (e.g., "Patient", "Observation")
 */
export type TypedReference<T extends string> = Reference & {
  reference: `${T}/${string}`;
  type?: T;
};"#
        .to_string()
    }

    /// Generate type guard for a specific resource type
    ///
    /// Creates:
    /// ```typescript
    /// export function isPatientReference(
    ///   ref: Reference
    /// ): ref is TypedReference<"Patient"> {
    ///   return ref.reference?.startsWith("Patient/") ?? false;
    /// }
    /// ```
    pub fn generate_type_guard(&self, resource_type: &str) -> String {
        format!(
            r#"/**
 * Check if a Reference points to a {} resource
 * @param ref - The Reference to check
 * @returns true if the reference is a {} reference
 */
export function is{}Reference(
  ref: Reference
): ref is TypedReference<"{}"> {{
  return ref.reference?.startsWith("{}/") ?? false;
}}"#,
            resource_type, resource_type, resource_type, resource_type, resource_type
        )
    }

    /// Generate factory function for creating typed references
    ///
    /// Creates:
    /// ```typescript
    /// export function createReference<T extends string>(
    ///   resourceType: T,
    ///   id: string,
    ///   display?: string
    /// ): TypedReference<T> {
    ///   return {
    ///     reference: `${resourceType}/${id}`,
    ///     type: resourceType,
    ///     display,
    ///   };
    /// }
    /// ```
    pub fn generate_factory() -> String {
        r#"/**
 * Create a typed Reference to a FHIR resource
 * @template T - The resource type
 * @param resourceType - The type of resource being referenced
 * @param id - The resource ID
 * @param display - Optional display text
 * @returns A typed Reference object
 */
export function createReference<T extends string>(
  resourceType: T,
  id: string,
  display?: string
): TypedReference<T> {
  return {
    reference: `${resourceType}/${id}` as `${T}/${string}`,
    type: resourceType as T,
    display: display as FhirString | undefined,
  } as TypedReference<T>;
}"#
        .to_string()
    }

    /// Generate helper to extract resource type from reference string
    ///
    /// Creates:
    /// ```typescript
    /// export function getResourceTypeFromReference(
    ///   reference: string
    /// ): string | undefined {
    ///   const match = reference.match(/^([A-Z][a-zA-Z]+)\//);
    ///   return match?.[1];
    /// }
    /// ```
    pub fn generate_type_extractor() -> String {
        r#"/**
 * Extract the resource type from a reference string
 * @param reference - The reference string (e.g., "Patient/123")
 * @returns The resource type or undefined if invalid
 */
export function getResourceTypeFromReference(
  reference: string
): string | undefined {
  const match = reference.match(/^([A-Z][a-zA-Z]+)\//);
  return match?.[1];
}

/**
 * Extract the resource ID from a reference string
 * @param reference - The reference string (e.g., "Patient/123")
 * @returns The resource ID or undefined if invalid
 */
export function getResourceIdFromReference(
  reference: string
): string | undefined {
  const parts = reference.split('/');
  return parts.length === 2 ? parts[1] : undefined;
}"#
        .to_string()
    }

    /// Generate all reference helper code
    pub fn generate_all(&self) -> String {
        let mut parts = vec![
            "// Type definitions".to_string(),
            Self::generate_typed_reference_type(),
        ];

        if self.has_factories {
            parts.push("\n// Factory functions".to_string());
            parts.push(Self::generate_factory());
            parts.push(Self::generate_type_extractor());
        }

        if self.has_type_guards && !self.resource_types.is_empty() {
            parts.push("\n// Type guards".to_string());
            for resource_type in &self.resource_types {
                parts.push(self.generate_type_guard(resource_type));
            }
        }

        parts.join("\n\n")
    }
}

/// Collect resource types from a list of structure definitions
pub fn collect_resource_types(structure_names: &[String]) -> Vec<String> {
    let mut types = HashSet::new();

    // Common FHIR resource types that should always have helpers
    let base_resources = vec![
        "Patient",
        "Practitioner",
        "Organization",
        "Location",
        "Observation",
        "Condition",
        "Procedure",
        "MedicationRequest",
        "Encounter",
    ];

    for name in base_resources {
        types.insert(name.to_string());
    }

    // Add resource types from structures
    for name in structure_names {
        // Only include types that look like resources (PascalCase, not ending in common suffixes)
        if name.chars().next().is_some_and(|c| c.is_uppercase())
            && !name.ends_with("Type")
            && !name.ends_with("Element")
        {
            types.insert(name.clone());
        }
    }

    let mut result: Vec<String> = types.into_iter().collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_config_default() {
        let config = ReferenceConfig::default();
        assert!(config.type_guards);
        assert!(config.factories);
        assert!(config.resolution_helpers);
    }

    #[test]
    fn test_generate_typed_reference_type() {
        let code = ReferenceHelpers::generate_typed_reference_type();
        assert!(code.contains("TypedReference<T extends string>"));
        assert!(code.contains("reference: `${T}/${string}`"));
    }

    #[test]
    fn test_generate_type_guard() {
        let config = ReferenceConfig::default();
        let helpers = ReferenceHelpers::new(vec!["Patient".to_string()], &config);

        let code = helpers.generate_type_guard("Patient");
        assert!(code.contains("isPatientReference"));
        assert!(code.contains("TypedReference<\"Patient\">"));
        assert!(code.contains("startsWith(\"Patient/\")"));
    }

    #[test]
    fn test_generate_factory() {
        let code = ReferenceHelpers::generate_factory();
        assert!(code.contains("createReference"));
        assert!(code.contains("<T extends string>"));
        assert!(code.contains("TypedReference<T>"));
    }

    #[test]
    fn test_generate_type_extractor() {
        let code = ReferenceHelpers::generate_type_extractor();
        assert!(code.contains("getResourceTypeFromReference"));
        assert!(code.contains("getResourceIdFromReference"));
    }

    #[test]
    fn test_generate_all() {
        let config = ReferenceConfig::default();
        let helpers = ReferenceHelpers::new(
            vec!["Patient".to_string(), "Observation".to_string()],
            &config,
        );

        let code = helpers.generate_all();

        // Should contain type definition
        assert!(code.contains("TypedReference"));

        // Should contain factory
        assert!(code.contains("createReference"));

        // Should contain type guards for both types
        assert!(code.contains("isPatientReference"));
        assert!(code.contains("isObservationReference"));
    }

    #[test]
    fn test_collect_resource_types() {
        let structures = vec![
            "Patient".to_string(),
            "Observation".to_string(),
            "HumanName".to_string(),       // Not a resource
            "CodeableConcept".to_string(), // Not a resource
            "BackboneElement".to_string(), // Not a resource
        ];

        let types = collect_resource_types(&structures);

        assert!(types.contains(&"Patient".to_string()));
        assert!(types.contains(&"Observation".to_string()));

        // Should not include element types
        assert!(!types.contains(&"BackboneElement".to_string()));
    }
}
