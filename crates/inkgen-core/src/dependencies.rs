//! Dependency analyzer for cross-package resource tracking.
//!
//! This module provides tree-shaking support by analyzing resource dependencies
//! across FHIR packages. It tracks which resources reference other resources
//! and determines what should be generated based on filter modes.

use std::collections::{HashMap, HashSet};

use crate::config::FilterMode;
use crate::ir::ResourceDefinition;

/// Tracks cross-package dependencies for smart tree-shaking.
///
/// The analyzer performs three-pass analysis:
/// 1. **Registration**: Map resources to their packages
/// 2. **Analysis**: Detect cross-package references
/// 3. **Filtering**: Determine what should be generated
///
/// # Example
///
/// ```no_run
/// use inkgen_core::DependencyAnalyzer;
/// use inkgen_core::FilterMode;
///
/// let mut analyzer = DependencyAnalyzer::new();
///
/// // Register packages and their resources
/// analyzer.register_package("hl7.fhir.r4.core", vec![
///     "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
///     "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
/// ]);
///
/// analyzer.register_package("hl7.fhir.us.core", vec![
///     "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
/// ]);
///
/// // Check if a resource should be generated
/// let should_gen = analyzer.should_generate(
///     "hl7.fhir.r4.core",
///     "http://hl7.org/fhir/StructureDefinition/Patient",
///     FilterMode::Dependencies,
/// );
/// ```
#[derive(Debug, Clone, Default)]
pub struct DependencyAnalyzer {
    /// Package → Set of URLs explicitly requested
    requested: HashMap<String, HashSet<String>>,

    /// Package → Set of URLs needed as dependencies from OTHER packages
    /// This tracks cross-package dependencies only
    dependencies: HashMap<String, HashSet<String>>,

    /// URL → Package mapping (for resolution)
    url_to_package: HashMap<String, String>,
}

impl DependencyAnalyzer {
    /// Creates a new dependency analyzer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requested: HashMap::new(),
            dependencies: HashMap::new(),
            url_to_package: HashMap::new(),
        }
    }

    /// Register which package owns which URLs.
    ///
    /// This must be called for all packages before analysis.
    ///
    /// # Arguments
    ///
    /// * `package` - The package ID (e.g., "hl7.fhir.r4.core")
    /// * `urls` - List of canonical URLs owned by this package
    pub fn register_package(&mut self, package: &str, urls: Vec<String>) {
        for url in urls {
            self.url_to_package.insert(url, package.to_string());
        }
    }

    /// Mark a resource as explicitly requested by a package.
    ///
    /// # Arguments
    ///
    /// * `package` - The requesting package ID
    /// * `url` - The canonical URL being requested
    pub fn request(&mut self, package: &str, url: &str) {
        self.requested
            .entry(package.to_string())
            .or_default()
            .insert(url.to_string());
    }

    /// Analyze a resource and track its cross-package dependencies.
    ///
    /// This examines:
    /// - Element type references (`element.type.code`)
    /// - Profile base definitions (`baseDefinition`)
    /// - Profile references in element types
    /// - Target profiles for Reference types
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to analyze
    /// * `package` - The package that owns this resource
    pub fn analyze(&mut self, resource: &ResourceDefinition, package: &str) {
        // Analyze all element types for cross-package references
        for element in &resource.elements {
            // Analyze element types (including profiles and target profiles)
            self.analyze_element_types(element, package);

            // Recursively analyze children
            self.analyze_children(&element.children, package);
        }

        // Analyze profile base definitions
        if let Some(base) = &resource.lineage.base_definition {
            self.track_dependency(base, package);
        }
    }

    /// Recursively analyze child elements for type references.
    fn analyze_children(&mut self, children: &[crate::ir::ElementDefinition], package: &str) {
        for child in children {
            self.analyze_element_types(child, package);
            self.analyze_children(&child.children, package);
        }
    }

    /// Analyze element types and track cross-package dependencies.
    fn analyze_element_types(&mut self, element: &crate::ir::ElementDefinition, package: &str) {
        for elem_type in &element.types {
            // Track the type code itself
            self.track_dependency(&elem_type.code, package);

            // Track profile URLs
            for profile in &elem_type.profiles {
                self.track_dependency(profile, package);
            }

            // Track target profiles (for Reference types)
            for target_profile in &elem_type.target_profiles {
                self.track_dependency(target_profile, package);
            }
        }

        // Track content references
        if let Some(content_ref) = &element.content_reference {
            self.track_dependency(content_ref, package);
        }
    }

    /// Track a dependency if it's from a different package.
    fn track_dependency(&mut self, url: &str, package: &str) {
        if let Some(owner_package) = self.url_to_package.get(url)
            && owner_package != package
        {
            // This is a cross-package dependency
            self.dependencies
                .entry(package.to_string())
                .or_default()
                .insert(url.to_string());
        }
    }

    /// Check if a resource should be generated for a package based on filter mode.
    ///
    /// # Arguments
    ///
    /// * `package` - The package being generated
    /// * `url` - The canonical URL of the resource
    /// * `filter` - The filter mode for this package
    ///
    /// # Returns
    ///
    /// `true` if the resource should be generated, `false` otherwise.
    ///
    /// # Filter Mode Behavior
    ///
    /// - `All`: Always generate
    /// - `None`: Never generate
    /// - `Dependencies`: Generate if ANY other package depends on this resource
    /// - `Include`/`Exclude`: Handled by caller (config-based filtering)
    #[must_use]
    pub fn should_generate(&self, package: &str, url: &str, filter: FilterMode) -> bool {
        match filter {
            FilterMode::All => true,
            FilterMode::None => false,
            FilterMode::Dependencies => {
                // Check if any OTHER package depends on this resource
                self.is_depended_upon_by_others(package, url)
            }
            FilterMode::Include | FilterMode::Exclude => {
                // These modes are handled by config-based filtering
                // We return true here to let the caller decide based on config
                true
            }
        }
    }

    /// Check if a resource is depended upon by other packages.
    ///
    /// # Arguments
    ///
    /// * `package` - The package that owns the resource
    /// * `url` - The canonical URL of the resource
    ///
    /// # Returns
    ///
    /// `true` if any OTHER package (not the owner) depends on this resource.
    #[must_use]
    pub fn is_depended_upon_by_others(&self, package: &str, url: &str) -> bool {
        self.dependencies
            .iter()
            .filter(|(pkg, _)| pkg.as_str() != package)
            .any(|(_, deps)| deps.contains(url))
    }

    /// Get all dependencies for a specific package.
    ///
    /// # Arguments
    ///
    /// * `package` - The package ID
    ///
    /// # Returns
    ///
    /// A set of canonical URLs that this package depends on.
    #[must_use]
    pub fn get_dependencies(&self, package: &str) -> Option<&HashSet<String>> {
        self.dependencies.get(package)
    }

    /// Get the package that owns a specific URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The canonical URL
    ///
    /// # Returns
    ///
    /// The package ID that owns this URL, if registered.
    #[must_use]
    pub fn get_package_for_url(&self, url: &str) -> Option<&str> {
        self.url_to_package.get(url).map(String::as_str)
    }

    /// Get statistics about the dependency graph.
    ///
    /// # Returns
    ///
    /// A tuple of (total packages, total resources, total cross-package dependencies).
    #[must_use]
    pub fn statistics(&self) -> (usize, usize, usize) {
        let total_packages = self
            .url_to_package
            .values()
            .collect::<HashSet<_>>()
            .len();
        let total_resources = self.url_to_package.len();
        let total_dependencies = self
            .dependencies
            .values()
            .map(HashSet::len)
            .sum();

        (total_packages, total_resources, total_dependencies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ElementCardinality, ElementDefinition, ElementMax, ElementType, ProfileLineage, ResourceKind};

    fn create_test_resource(url: &str, base_def: Option<String>) -> ResourceDefinition {
        ResourceDefinition {
            id: "test".to_string(),
            url: url.to_string(),
            name: Some("Test".to_string()),
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: None,
            date: None,
            lineage: ProfileLineage {
                base_definition: base_def,
                base_id: None,
                derivation: None,
                type_name: None,
            },
            elements: vec![],
            extensions: vec![],
            invariants: vec![],
        }
    }

    fn create_element_with_type(type_code: &str) -> ElementDefinition {
        ElementDefinition {
            id: "test".to_string(),
            path: "Test".to_string(),
            slice_name: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Finite(1),
            },
            types: vec![ElementType {
                code: type_code.to_string(),
                profiles: vec![],
                target_profiles: vec![],
                aggregation: vec![],
                versioning: None,
            }],
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
            parent_path: None,
            depth: 0,
            is_backbone: false,
        }
    }

    #[test]
    fn test_register_package() {
        let mut analyzer = DependencyAnalyzer::new();
        let urls = vec![
            "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
        ];

        analyzer.register_package("hl7.fhir.r4.core", urls.clone());

        assert_eq!(
            analyzer.get_package_for_url("http://hl7.org/fhir/StructureDefinition/Patient"),
            Some("hl7.fhir.r4.core")
        );
        assert_eq!(
            analyzer.get_package_for_url("http://hl7.org/fhir/StructureDefinition/Observation"),
            Some("hl7.fhir.r4.core")
        );
    }

    #[test]
    fn test_analyze_base_definition() {
        let mut analyzer = DependencyAnalyzer::new();

        // Register packages
        analyzer.register_package(
            "hl7.fhir.r4.core",
            vec!["http://hl7.org/fhir/StructureDefinition/Patient".to_string()],
        );
        analyzer.register_package(
            "hl7.fhir.us.core",
            vec!["http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string()],
        );

        // Create a profile that extends Patient
        let profile = create_test_resource(
            "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient",
            Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
        );

        analyzer.analyze(&profile, "hl7.fhir.us.core");

        // US Core should depend on R4's Patient
        let deps = analyzer.get_dependencies("hl7.fhir.us.core").unwrap();
        assert!(deps.contains("http://hl7.org/fhir/StructureDefinition/Patient"));
    }

    #[test]
    fn test_analyze_element_types() {
        let mut analyzer = DependencyAnalyzer::new();

        // Register packages
        analyzer.register_package(
            "hl7.fhir.r4.core",
            vec![
                "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                "http://hl7.org/fhir/StructureDefinition/HumanName".to_string(),
            ],
        );
        analyzer.register_package(
            "hl7.fhir.us.core",
            vec!["http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string()],
        );

        // Create a resource with HumanName type
        let mut resource = create_test_resource(
            "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient",
            None,
        );
        resource.elements = vec![create_element_with_type(
            "http://hl7.org/fhir/StructureDefinition/HumanName",
        )];

        analyzer.analyze(&resource, "hl7.fhir.us.core");

        // US Core should depend on HumanName
        let deps = analyzer.get_dependencies("hl7.fhir.us.core").unwrap();
        assert!(deps.contains("http://hl7.org/fhir/StructureDefinition/HumanName"));
    }

    #[test]
    fn test_should_generate_all_mode() {
        let analyzer = DependencyAnalyzer::new();
        assert!(analyzer.should_generate(
            "hl7.fhir.r4.core",
            "http://hl7.org/fhir/StructureDefinition/Patient",
            FilterMode::All
        ));
    }

    #[test]
    fn test_should_generate_none_mode() {
        let analyzer = DependencyAnalyzer::new();
        assert!(!analyzer.should_generate(
            "hl7.fhir.r4.core",
            "http://hl7.org/fhir/StructureDefinition/Patient",
            FilterMode::None
        ));
    }

    #[test]
    fn test_should_generate_dependencies_mode() {
        let mut analyzer = DependencyAnalyzer::new();

        // Register packages
        analyzer.register_package(
            "hl7.fhir.r4.core",
            vec![
                "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
            ],
        );
        analyzer.register_package(
            "hl7.fhir.us.core",
            vec!["http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string()],
        );

        // Create a profile that depends on Patient
        let profile = create_test_resource(
            "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient",
            Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
        );

        analyzer.analyze(&profile, "hl7.fhir.us.core");

        // Patient should be generated (depended upon by US Core)
        assert!(analyzer.should_generate(
            "hl7.fhir.r4.core",
            "http://hl7.org/fhir/StructureDefinition/Patient",
            FilterMode::Dependencies
        ));

        // Observation should NOT be generated (no dependencies)
        assert!(!analyzer.should_generate(
            "hl7.fhir.r4.core",
            "http://hl7.org/fhir/StructureDefinition/Observation",
            FilterMode::Dependencies
        ));
    }

    #[test]
    fn test_statistics() {
        let mut analyzer = DependencyAnalyzer::new();

        analyzer.register_package(
            "hl7.fhir.r4.core",
            vec![
                "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                "http://hl7.org/fhir/StructureDefinition/HumanName".to_string(),
            ],
        );
        analyzer.register_package(
            "hl7.fhir.us.core",
            vec!["http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string()],
        );

        let (packages, resources, deps) = analyzer.statistics();
        assert_eq!(packages, 2);
        assert_eq!(resources, 3);
        assert_eq!(deps, 0); // No dependencies yet
    }

    #[test]
    fn test_no_self_dependency() {
        let mut analyzer = DependencyAnalyzer::new();

        // Register package
        analyzer.register_package(
            "hl7.fhir.r4.core",
            vec![
                "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                "http://hl7.org/fhir/StructureDefinition/HumanName".to_string(),
            ],
        );

        // Create a resource that references another resource in the SAME package
        let mut resource = create_test_resource(
            "http://hl7.org/fhir/StructureDefinition/Patient",
            None,
        );
        resource.elements = vec![create_element_with_type(
            "http://hl7.org/fhir/StructureDefinition/HumanName",
        )];

        analyzer.analyze(&resource, "hl7.fhir.r4.core");

        // Should have no cross-package dependencies
        assert!(analyzer.get_dependencies("hl7.fhir.r4.core").is_none()
            || analyzer.get_dependencies("hl7.fhir.r4.core").unwrap().is_empty());
    }
}
