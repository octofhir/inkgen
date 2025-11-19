//! Backend registry for managing language-specific code generators.
//!
//! This module provides a pluggable architecture for registering and discovering
//! language backends without hardcoding specific implementations in the core.

use std::collections::HashMap;

use crate::generator::LanguageGenerator;
use crate::StructureDefinitionProvider;

/// Registry for managing language backends.
///
/// The `BackendRegistry` allows dynamic registration of language-specific code generators,
/// enabling a pluggable architecture where new languages can be added without modifying
/// core infrastructure code.
///
/// # Examples
///
/// ```ignore
/// use inkgen_core::backends::BackendRegistry;
/// use inkgen_typescript::TypescriptBackend;
///
/// let mut registry = BackendRegistry::new();
///
/// // Register TypeScript backend
/// registry.register(Box::new(TypescriptBackend::new(config)));
///
/// // Look up a backend by name
/// if let Some(backend) = registry.get("typescript") {
///     println!("Found backend: {}", backend.description());
/// }
///
/// // List all available backends
/// for name in registry.list() {
///     println!("Available: {}", name);
/// }
/// ```
pub struct BackendRegistry<S>
where
    S: StructureDefinitionProvider + Sync + Send,
{
    backends: HashMap<String, Box<dyn LanguageBackend<S>>>,
}

impl<S> BackendRegistry<S>
where
    S: StructureDefinitionProvider + Sync + Send,
{
    /// Create a new empty backend registry.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    /// Register a language backend.
    ///
    /// The backend's `name()` method will be used as the lookup key.
    /// If a backend with the same name already exists, it will be replaced.
    pub fn register(&mut self, backend: Box<dyn LanguageBackend<S>>) {
        let name = backend.name().to_string();
        self.backends.insert(name, backend);
    }

    /// Get a backend by name.
    ///
    /// Returns `None` if no backend with the given name is registered.
    pub fn get(&self, name: &str) -> Option<&dyn LanguageBackend<S>> {
        self.backends.get(name).map(|b| &**b)
    }

    /// List all registered backend names.
    ///
    /// Returns a vector of backend names in no particular order.
    pub fn list(&self) -> Vec<&str> {
        self.backends.keys().map(String::as_str).collect()
    }

    /// Check if a backend is registered.
    pub fn has(&self, name: &str) -> bool {
        self.backends.contains_key(name)
    }

    /// Get the number of registered backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

impl<S> Default for BackendRegistry<S>
where
    S: StructureDefinitionProvider + Sync + Send,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Core trait for language-specific code generation backends.
///
/// This trait extends `LanguageGenerator` with metadata methods that enable
/// backend discovery and registration. All language backends should implement
/// this trait to participate in the pluggable backend system.
///
/// # Design Philosophy
///
/// The trait is designed to be language-agnostic - it should not contain any
/// TypeScript-specific, Python-specific, or other language-specific details.
/// Language-specific configuration and behavior should be encapsulated within
/// the implementing types.
///
/// # Examples
///
/// ```ignore
/// use inkgen_core::backends::LanguageBackend;
/// use inkgen_core::LanguageGenerator;
///
/// pub struct RustBackend {
///     config: RustConfig,
/// }
///
/// impl LanguageBackend for RustBackend {
///     fn name(&self) -> &str {
///         "rust"
///     }
///
///     fn description(&self) -> &str {
///         "Rust code generator with serde support"
///     }
///
///     fn file_extension(&self) -> &str {
///         "rs"
///     }
///
///     fn supports_feature(&self, feature: &str) -> bool {
///         matches!(feature, "serde" | "builders" | "validation")
///     }
/// }
///
/// // LanguageGenerator trait must also be implemented
/// #[async_trait]
/// impl<S> LanguageGenerator<S> for RustBackend
/// where
///     S: StructureDefinitionProvider + Sync + Send,
/// {
///     async fn generate(
///         &self,
///         service: &S,
///         descriptor: &PackageDescriptor,
///         provider_config: &StructureProviderConfig,
///     ) -> Result<()> {
///         // Implementation here
///         Ok(())
///     }
/// }
/// ```
pub trait LanguageBackend<S>: LanguageGenerator<S> + Send + Sync
where
    S: StructureDefinitionProvider + Sync + Send,
{
    /// Canonical name of this backend (e.g., "typescript", "rust", "python").
    ///
    /// This name is used for registry lookups and CLI commands.
    /// Convention: lowercase, no spaces, dash-separated for multi-word names.
    fn name(&self) -> &str;

    /// Human-readable description of this backend.
    ///
    /// Should briefly describe the target language and key features.
    /// Example: "TypeScript/JavaScript with type-safe FHIR models"
    fn description(&self) -> &str;

    /// Primary file extension for generated files (without the dot).
    ///
    /// Example: "ts", "rs", "py"
    fn file_extension(&self) -> &str;

    /// Check if this backend supports a specific feature.
    ///
    /// This is an optional hook for feature detection. Common features might include:
    /// - "builders" - builder pattern support
    /// - "validation" - runtime validation
    /// - "serialization" - JSON serialization
    /// - "async" - async/await support
    ///
    /// Default implementation returns `false` for all features.
    fn supports_feature(&self, _feature: &str) -> bool {
        false
    }

    /// Get the version of this backend implementation.
    ///
    /// Default returns "unknown". Backends should override this with
    /// their actual version (typically from `env!("CARGO_PKG_VERSION")`).
    fn version(&self) -> &str {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::ResourceDefinition;
    use crate::services::StructureFilter;
    use crate::{CoreResult, PackageDescriptor, StructureProviderConfig, StructureSummary};
    use anyhow::Result;
    use async_trait::async_trait;

    // Mock provider for testing
    struct MockProvider;

    #[async_trait]
    impl StructureDefinitionProvider for MockProvider {
        async fn list_structures(
            &self,
            _filter: &StructureFilter<'_>,
        ) -> CoreResult<Vec<StructureSummary>> {
            Ok(Vec::new())
        }

        async fn load_structure(&self, _canonical: &str) -> CoreResult<ResourceDefinition> {
            unimplemented!("Mock provider doesn't load structures")
        }
    }

    // Mock backend for testing
    struct MockBackend {
        name: String,
        description: String,
        extension: String,
    }

    impl MockBackend {
        fn new(name: &str, description: &str, extension: &str) -> Self {
            Self {
                name: name.to_string(),
                description: description.to_string(),
                extension: extension.to_string(),
            }
        }
    }

    #[async_trait]
    impl LanguageGenerator<MockProvider> for MockBackend {
        async fn generate(
            &self,
            _service: &MockProvider,
            _descriptor: &PackageDescriptor,
            _provider_config: &StructureProviderConfig,
        ) -> Result<()> {
            Ok(())
        }
    }

    impl LanguageBackend<MockProvider> for MockBackend {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn file_extension(&self) -> &str {
            &self.extension
        }
    }

    #[test]
    fn test_backend_registry_basic_operations() {
        let mut registry = BackendRegistry::<MockProvider>::new();

        // Initially empty
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        // Register a backend
        let backend = Box::new(MockBackend::new(
            "typescript",
            "TypeScript generator",
            "ts",
        ));
        registry.register(backend);

        // Check registration
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert!(registry.has("typescript"));
        assert!(!registry.has("python"));

        // Retrieve backend
        let backend = registry.get("typescript").expect("backend should exist");
        assert_eq!(backend.name(), "typescript");
        assert_eq!(backend.description(), "TypeScript generator");
        assert_eq!(backend.file_extension(), "ts");

        // List backends
        let backends = registry.list();
        assert_eq!(backends.len(), 1);
        assert!(backends.contains(&"typescript"));
    }

    #[test]
    fn test_backend_registry_multiple_backends() {
        let mut registry = BackendRegistry::<MockProvider>::new();

        registry.register(Box::new(MockBackend::new("typescript", "TS", "ts")));
        registry.register(Box::new(MockBackend::new("rust", "Rust", "rs")));
        registry.register(Box::new(MockBackend::new("python", "Python", "py")));

        assert_eq!(registry.len(), 3);
        assert!(registry.has("typescript"));
        assert!(registry.has("rust"));
        assert!(registry.has("python"));

        let backends = registry.list();
        assert_eq!(backends.len(), 3);
    }

    #[test]
    fn test_backend_registry_replacement() {
        let mut registry = BackendRegistry::<MockProvider>::new();

        registry.register(Box::new(MockBackend::new(
            "typescript",
            "Old description",
            "ts",
        )));
        assert_eq!(
            registry.get("typescript").unwrap().description(),
            "Old description"
        );

        // Replace with new backend
        registry.register(Box::new(MockBackend::new(
            "typescript",
            "New description",
            "ts",
        )));

        assert_eq!(registry.len(), 1); // Still only one backend
        assert_eq!(
            registry.get("typescript").unwrap().description(),
            "New description"
        );
    }

    #[test]
    fn test_backend_not_found() {
        let registry = BackendRegistry::<MockProvider>::new();
        assert!(registry.get("nonexistent").is_none());
    }
}
