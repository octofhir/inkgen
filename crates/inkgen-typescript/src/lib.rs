//! TypeScript backend placeholder module.

use anyhow::Result;
use inkgen_core::{PackageId, PackageService};

/// Configuration placeholder for the TypeScript generator.
#[derive(Debug, Default)]
pub struct TypescriptGeneratorConfig {
    /// Output directory hint.
    pub output_dir: String,
}

/// Trait describing minimal generator behaviour.
pub trait LanguageGenerator {
    /// Generate SDK artifacts for the provided package.
    fn generate<S: PackageService>(&self, service: &S, package: &PackageId) -> Result<()>;
}

/// Stub generator that currently logs intent.
#[derive(Debug, Default)]
pub struct TypescriptGenerator {
    config: TypescriptGeneratorConfig,
}

impl TypescriptGenerator {
    /// Create a new generator using the provided configuration.
    pub fn new(config: TypescriptGeneratorConfig) -> Self {
        Self { config }
    }
}

impl LanguageGenerator for TypescriptGenerator {
    fn generate<S: PackageService>(&self, _service: &S, package: &PackageId) -> Result<()> {
        tracing::info!(
            "TypeScript generator placeholder invoked for package {}",
            package.name
        );
        let _ = &self.config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use inkgen_core::{PackageMetadata, PackageService};

    struct StubService;

    impl PackageService for StubService {
        fn describe(&self, id: &PackageId) -> anyhow::Result<Option<PackageMetadata>> {
            let mut properties = IndexMap::new();
            properties.insert("name".into(), id.name.clone());
            let metadata = PackageMetadata { properties };
            Ok(Some(metadata))
        }
    }

    #[test]
    fn generator_invocation_succeeds() {
        let generator = TypescriptGenerator::default();
        let service = StubService;
        let package = PackageId::new("hl7.fhir.r4.core", None);
        generator.generate(&service, &package).expect("generate");
    }
}
