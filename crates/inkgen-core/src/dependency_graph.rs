//! Dependency graph for topological ordering of type generation.
//!
//! This module provides dependency tracking and topological sorting to ensure
//! types are generated in the correct order (dependencies before dependents).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::canonical_map::CanonicalTypeMap;
use crate::ir::{ElementDefinition, ResourceDefinition};

/// Tracks dependencies between types for topological ordering.
///
/// This graph maps each type to the set of types it depends on.
/// Used to determine correct generation order where dependencies
/// are generated before the types that use them.
#[derive(Debug, Clone, Default)]
pub struct TypeDependencyGraph {
    /// Maps type name → set of dependency type names
    deps: HashMap<String, HashSet<String>>,
    /// All known types in the graph
    all_types: HashSet<String>,
}

impl TypeDependencyGraph {
    /// Create a new empty dependency graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a structure definition and extract its dependencies.
    ///
    /// This analyzes all element types to find which other types
    /// this structure depends on.
    pub fn add_structure(&mut self, def: &ResourceDefinition, type_map: &CanonicalTypeMap) {
        let type_name = def.name.as_deref().unwrap_or(&def.id);
        self.all_types.insert(type_name.to_string());

        let mut dependencies = HashSet::new();

        // Add base type as dependency if it exists
        if let Some(base_url) = &def.lineage.base_definition {
            if let Some(entry) = type_map.get_by_url(base_url) {
                dependencies.insert(entry.type_name.clone());
            }
        }

        // Extract dependencies from all elements
        self.extract_element_deps(&def.elements, type_map, &mut dependencies);

        // Don't add self-dependencies
        dependencies.remove(type_name);

        self.deps.insert(type_name.to_string(), dependencies);
    }

    /// Add a type manually (for types not loaded via ResourceDefinition).
    pub fn add_type(&mut self, type_name: &str) {
        self.all_types.insert(type_name.to_string());
        self.deps.entry(type_name.to_string()).or_default();
    }

    /// Add a dependency edge: `from_type` depends on `to_type`.
    pub fn add_dependency(&mut self, from_type: &str, to_type: &str) {
        if from_type != to_type {
            self.deps
                .entry(from_type.to_string())
                .or_default()
                .insert(to_type.to_string());
        }
    }

    /// Extract dependencies from element definitions.
    fn extract_element_deps(
        &self,
        elements: &[ElementDefinition],
        type_map: &CanonicalTypeMap,
        deps: &mut HashSet<String>,
    ) {
        for element in elements {
            // Extract type references
            for elem_type in &element.types {
                // The type code might be a simple name or a URL
                self.resolve_type_name(&elem_type.code, type_map, deps);

                // Check profile URLs
                for profile in &elem_type.profiles {
                    if let Some(entry) = type_map.get_by_url(profile) {
                        deps.insert(entry.type_name.clone());
                    }
                }

                // Check target profiles (for Reference types)
                for target in &elem_type.target_profiles {
                    if let Some(entry) = type_map.get_by_url(target) {
                        deps.insert(entry.type_name.clone());
                    }
                }
            }

            // Recurse into children
            self.extract_element_deps(&element.children, type_map, deps);
        }
    }

    /// Resolve a type code to a type name.
    fn resolve_type_name(
        &self,
        type_code: &str,
        type_map: &CanonicalTypeMap,
        deps: &mut HashSet<String>,
    ) {
        // Skip primitive types and built-ins that don't need imports
        if is_primitive_or_builtin(type_code) {
            return;
        }

        // Try as canonical URL first
        if type_code.starts_with("http://") || type_code.starts_with("https://") {
            if let Some(entry) = type_map.get_by_url(type_code) {
                deps.insert(entry.type_name.clone());
                return;
            }
        }

        // Try as type name
        if type_map.contains_name(type_code) {
            deps.insert(type_code.to_string());
        }
    }

    /// Return types in topological order (dependencies first).
    ///
    /// Uses Kahn's algorithm for stable, deterministic ordering.
    /// Returns `None` if there's a cycle in the dependency graph.
    pub fn topological_sort(&self) -> Option<Vec<String>> {
        // Build in-degree map and reverse adjacency list
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize all types with zero in-degree
        for type_name in &self.all_types {
            in_degree.insert(type_name.clone(), 0);
        }

        // Calculate in-degrees (only for dependencies that are in the graph)
        for (type_name, deps) in &self.deps {
            for dep in deps {
                if self.all_types.contains(dep) {
                    *in_degree.get_mut(type_name).unwrap() += 1;
                    dependents
                        .entry(dep.clone())
                        .or_default()
                        .push(type_name.clone());
                }
            }
        }

        // Start with types that have no dependencies
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        // Sort queue for deterministic ordering
        let mut queue_vec: Vec<_> = queue.drain(..).collect();
        queue_vec.sort();
        queue.extend(queue_vec);

        let mut result = Vec::new();

        while let Some(type_name) = queue.pop_front() {
            result.push(type_name.clone());

            // Reduce in-degree for dependents
            if let Some(deps) = dependents.get(&type_name) {
                let mut new_ready = Vec::new();
                for dependent in deps {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            new_ready.push(dependent.clone());
                        }
                    }
                }
                // Sort for deterministic ordering
                new_ready.sort();
                queue.extend(new_ready);
            }
        }

        // Check for cycles
        if result.len() != self.all_types.len() {
            tracing::warn!(
                "Dependency cycle detected! Processed {} of {} types",
                result.len(),
                self.all_types.len()
            );
            // Return what we have, appending remaining types in sorted order
            let remaining: Vec<_> = self
                .all_types
                .iter()
                .filter(|t| !result.contains(t))
                .cloned()
                .collect();
            let mut remaining_sorted = remaining;
            remaining_sorted.sort();
            result.extend(remaining_sorted);
        }

        Some(result)
    }

    /// Get the direct dependencies for a type.
    pub fn get_dependencies(&self, type_name: &str) -> Option<&HashSet<String>> {
        self.deps.get(type_name)
    }

    /// Get all types in the graph.
    pub fn all_types(&self) -> impl Iterator<Item = &str> {
        self.all_types.iter().map(String::as_str)
    }

    /// Get the number of types in the graph.
    pub fn len(&self) -> usize {
        self.all_types.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.all_types.is_empty()
    }
}

/// Check if a type code is a primitive or built-in that doesn't need imports.
fn is_primitive_or_builtin(type_code: &str) -> bool {
    matches!(
        type_code,
        "boolean"
            | "integer"
            | "string"
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
            | "http://hl7.org/fhirpath/System.String"
            | "http://hl7.org/fhirpath/System.Boolean"
            | "http://hl7.org/fhirpath/System.Integer"
            | "http://hl7.org/fhirpath/System.Decimal"
            | "http://hl7.org/fhirpath/System.DateTime"
            | "http://hl7.org/fhirpath/System.Time"
            | "http://hl7.org/fhirpath/System.Date"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort_simple() {
        let mut graph = TypeDependencyGraph::new();

        // A depends on B and C
        // B depends on C
        // C has no dependencies
        graph.add_type("A");
        graph.add_type("B");
        graph.add_type("C");
        graph.add_dependency("A", "B");
        graph.add_dependency("A", "C");
        graph.add_dependency("B", "C");

        let sorted = graph.topological_sort().unwrap();

        // C should come before B, B should come before A
        let c_idx = sorted.iter().position(|x| x == "C").unwrap();
        let b_idx = sorted.iter().position(|x| x == "B").unwrap();
        let a_idx = sorted.iter().position(|x| x == "A").unwrap();

        assert!(c_idx < b_idx);
        assert!(b_idx < a_idx);
    }

    #[test]
    fn test_topological_sort_no_deps() {
        let mut graph = TypeDependencyGraph::new();
        graph.add_type("A");
        graph.add_type("B");
        graph.add_type("C");

        let sorted = graph.topological_sort().unwrap();

        // All types should be present, in alphabetical order (deterministic)
        assert_eq!(sorted, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_topological_sort_with_cycle() {
        let mut graph = TypeDependencyGraph::new();
        graph.add_type("A");
        graph.add_type("B");
        graph.add_dependency("A", "B");
        graph.add_dependency("B", "A"); // Cycle!

        // Should still return a result, even with cycles
        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted.len(), 2);
    }

    #[test]
    fn test_external_deps_ignored() {
        let mut graph = TypeDependencyGraph::new();
        graph.add_type("A");
        graph.add_dependency("A", "ExternalType"); // Not in graph

        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted, vec!["A"]);
    }

    #[test]
    fn test_is_primitive_or_builtin() {
        assert!(is_primitive_or_builtin("string"));
        assert!(is_primitive_or_builtin("boolean"));
        assert!(is_primitive_or_builtin("integer"));
        assert!(is_primitive_or_builtin(
            "http://hl7.org/fhirpath/System.String"
        ));
        assert!(!is_primitive_or_builtin("Extension"));
        assert!(!is_primitive_or_builtin(
            "http://hl7.org/fhir/StructureDefinition/Extension"
        ));
    }
}
