//! FHIR Bundle traversal and manipulation utilities.
//!
//! This module generates TypeScript helpers for working with FHIR Bundles,
//! including resource extraction, reference resolution, and graph traversal.

use serde::Serialize;

/// Configuration for Bundle utility generation
#[derive(Debug, Clone, Serialize)]
pub struct BundleConfig {
    /// Generate resource extraction helpers
    pub extraction: bool,
    /// Generate reference resolution helpers
    pub resolution: bool,
    /// Generate graph traversal helpers
    pub graph_traversal: bool,
}

impl Default for BundleConfig {
    fn default() -> Self {
        Self {
            extraction: true,
            resolution: true,
            graph_traversal: true,
        }
    }
}

/// Bundle utility generation helper
#[derive(Debug, Clone, Serialize)]
pub struct BundleHelpers {
    /// Whether to generate extraction helpers
    pub has_extraction: bool,
    /// Whether to generate resolution helpers
    pub has_resolution: bool,
    /// Whether to generate graph traversal
    pub has_graph_traversal: bool,
}

impl BundleHelpers {
    /// Creates bundle helpers with configuration
    pub fn new(config: &BundleConfig) -> Self {
        Self {
            has_extraction: config.extraction,
            has_resolution: config.resolution,
            has_graph_traversal: config.graph_traversal,
        }
    }

    /// Generate resource extraction by type
    pub fn generate_resource_extraction() -> String {
        r#"/**
 * Extract all resources of a specific type from a Bundle
 * @template T - The resource type
 * @param bundle - The FHIR Bundle
 * @param resourceType - The resource type to extract
 * @returns Array of resources of the specified type
 */
export function getResourcesOfType<T extends Resource>(
  bundle: Bundle,
  resourceType: T['resourceType']
): T[] {
  return (bundle.entry ?? [])
    .map(e => e.resource)
    .filter((r): r is T => r?.resourceType === resourceType);
}

/**
 * Get the first resource of a specific type from a Bundle
 * @template T - The resource type
 * @param bundle - The FHIR Bundle
 * @param resourceType - The resource type to find
 * @returns The first resource of the type, or undefined if not found
 */
export function getFirstResourceOfType<T extends Resource>(
  bundle: Bundle,
  resourceType: T['resourceType']
): T | undefined {
  return bundle.entry
    ?.map(e => e.resource)
    .find((r): r is T => r?.resourceType === resourceType);
}"#
        .to_string()
    }

    /// Generate reference resolution helpers
    pub fn generate_reference_resolution() -> String {
        r#"/**
 * Resolve a Reference to its resource within a Bundle
 * @template T - The resource type
 * @param bundle - The FHIR Bundle
 * @param reference - The Reference to resolve
 * @returns The resolved resource or undefined if not found
 */
export function resolveReferenceInBundle<T extends Resource>(
  bundle: Bundle,
  reference: Reference
): T | undefined {
  if (!reference.reference) {
    return undefined;
  }

  // Handle both relative references (ResourceType/id) and absolute URLs
  const refStr = reference.reference;
  const relativeMatch = refStr.match(/^([A-Z][a-zA-Z]+)\/(.+)$/);

  if (relativeMatch) {
    const [, resourceType, id] = relativeMatch;
    return bundle.entry
      ?.map(e => e.resource)
      .find(
        (r): r is T =>
          r?.resourceType === resourceType && r.id === id
      );
  }

  // Try to match by fullUrl or absolute URL
  return bundle.entry
    ?.map(e => e.resource)
    .find((r): r is T => {
      const entry = bundle.entry?.find(e => e.resource === r);
      return entry?.fullUrl === refStr;
    });
}

/**
 * Resolve multiple references in a Bundle
 * @template T - The resource type
 * @param bundle - The FHIR Bundle
 * @param references - Array of References to resolve
 * @returns Array of resolved resources (undefined for unresolved references)
 */
export function resolveReferencesInBundle<T extends Resource>(
  bundle: Bundle,
  references: Reference[]
): (T | undefined)[] {
  return references.map(ref => resolveReferenceInBundle<T>(bundle, ref));
}"#
        .to_string()
    }

    /// Generate graph traversal helpers
    pub fn generate_graph_traversal() -> String {
        r#"/**
 * Build a reference graph from a Bundle
 * Maps resource identifiers to the resources they reference
 * @param bundle - The FHIR Bundle
 * @returns A Map of resource keys to referenced resource keys
 */
export function buildReferenceGraph(
  bundle: Bundle
): Map<string, Set<string>> {
  const graph = new Map<string, Set<string>>();

  for (const entry of bundle.entry ?? []) {
    const resource = entry.resource;
    if (!resource?.resourceType || !resource.id) continue;

    const key = `${resource.resourceType}/${resource.id}`;
    const refs = new Set<string>();

    // Extract all Reference.reference strings from the resource
    extractReferencesFromResource(resource, refs);

    graph.set(key, refs);
  }

  return graph;
}

/**
 * Extract all reference strings from a resource (recursive)
 * @param obj - The object to search
 * @param refs - Set to collect reference strings
 */
function extractReferencesFromResource(
  obj: any,
  refs: Set<string>
): void {
  if (!obj || typeof obj !== 'object') return;

  // Check if this is a Reference object
  if (obj.reference && typeof obj.reference === 'string') {
    refs.add(obj.reference);
  }

  // Recurse into arrays and objects
  if (Array.isArray(obj)) {
    for (const item of obj) {
      extractReferencesFromResource(item, refs);
    }
  } else {
    for (const value of Object.values(obj)) {
      extractReferencesFromResource(value, refs);
    }
  }
}

/**
 * Find all resources that reference a specific resource
 * @param bundle - The FHIR Bundle
 * @param targetResource - The resource to find references to
 * @returns Array of resources that reference the target
 */
export function findReferencingResources(
  bundle: Bundle,
  targetResource: Resource
): Resource[] {
  if (!targetResource.resourceType || !targetResource.id) {
    return [];
  }

  const targetKey = `${targetResource.resourceType}/${targetResource.id}`;
  const referencingResources: Resource[] = [];

  for (const entry of bundle.entry ?? []) {
    const resource = entry.resource;
    if (!resource) continue;

    const refs = new Set<string>();
    extractReferencesFromResource(resource, refs);

    if (refs.has(targetKey)) {
      referencingResources.push(resource);
    }
  }

  return referencingResources;
}"#
        .to_string()
    }

    /// Generate Bundle manipulation helpers
    pub fn generate_bundle_manipulation() -> String {
        r#"/**
 * Add a resource to a Bundle
 * @param bundle - The FHIR Bundle
 * @param resource - The resource to add
 * @param fullUrl - Optional fullUrl for the entry
 * @returns The modified Bundle
 */
export function addResourceToBundle(
  bundle: Bundle,
  resource: Resource,
  fullUrl?: string
): Bundle {
  const entry: BundleEntry = { resource };
  if (fullUrl) {
    entry.fullUrl = fullUrl;
  }

  return {
    ...bundle,
    entry: [...(bundle.entry ?? []), entry],
  };
}

/**
 * Remove a resource from a Bundle by reference
 * @param bundle - The FHIR Bundle
 * @param reference - Reference to the resource to remove
 * @returns The modified Bundle
 */
export function removeResourceFromBundle(
  bundle: Bundle,
  reference: Reference
): Bundle {
  if (!reference.reference) {
    return bundle;
  }

  const refStr = reference.reference;
  const relativeMatch = refStr.match(/^([A-Z][a-zA-Z]+)\/(.+)$/);

  return {
    ...bundle,
    entry: bundle.entry?.filter(entry => {
      const resource = entry.resource;
      if (!resource) return true;

      if (relativeMatch) {
        const [, resourceType, id] = relativeMatch;
        return !(resource.resourceType === resourceType && resource.id === id);
      }

      return entry.fullUrl !== refStr;
    }),
  };
}"#
        .to_string()
    }

    /// Generate all Bundle helper code
    pub fn generate_all(&self) -> String {
        let mut parts = Vec::new();

        if self.has_extraction {
            parts.push("// Resource extraction".to_string());
            parts.push(Self::generate_resource_extraction());
        }

        if self.has_resolution {
            parts.push("\n// Reference resolution".to_string());
            parts.push(Self::generate_reference_resolution());
        }

        if self.has_graph_traversal {
            parts.push("\n// Graph traversal".to_string());
            parts.push(Self::generate_graph_traversal());
        }

        // Always include manipulation helpers
        parts.push("\n// Bundle manipulation".to_string());
        parts.push(Self::generate_bundle_manipulation());

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_config_default() {
        let config = BundleConfig::default();
        assert!(config.extraction);
        assert!(config.resolution);
        assert!(config.graph_traversal);
    }

    #[test]
    fn test_generate_resource_extraction() {
        let code = BundleHelpers::generate_resource_extraction();
        assert!(code.contains("getResourcesOfType"));
        assert!(code.contains("getFirstResourceOfType"));
    }

    #[test]
    fn test_generate_reference_resolution() {
        let code = BundleHelpers::generate_reference_resolution();
        assert!(code.contains("resolveReferenceInBundle"));
        assert!(code.contains("resolveReferencesInBundle"));
    }

    #[test]
    fn test_generate_graph_traversal() {
        let code = BundleHelpers::generate_graph_traversal();
        assert!(code.contains("buildReferenceGraph"));
        assert!(code.contains("findReferencingResources"));
    }

    #[test]
    fn test_generate_all() {
        let config = BundleConfig::default();
        let helpers = BundleHelpers::new(&config);

        let code = helpers.generate_all();

        // Should contain extraction helpers
        assert!(code.contains("getResourcesOfType"));

        // Should contain resolution helpers
        assert!(code.contains("resolveReferenceInBundle"));

        // Should contain graph traversal
        assert!(code.contains("buildReferenceGraph"));

        // Should contain manipulation helpers
        assert!(code.contains("addResourceToBundle"));
    }
}
