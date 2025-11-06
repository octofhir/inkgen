//! TypeScript backend placeholder module.

use anyhow::Result;
use inkgen_core::PackageId;

/// Configuration placeholder for the TypeScript generator.
#[derive(Debug, Default)]
pub struct TypescriptGeneratorConfig {
    /// Output directory hint.
    pub output_dir: String,
}

/// Trait describing minimal generator behaviour.
pub trait LanguageGenerator<S> {
    /// Generate SDK artifacts for the provided package.
    fn generate(&self, service: &S, package: &PackageId) -> Result<()>;
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

impl<S> LanguageGenerator<S> for TypescriptGenerator {
    fn generate(&self, _service: &S, package: &PackageId) -> Result<()> {
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
    use std::sync::Arc;

    #[test]
    fn generator_invocation_succeeds() {
        let generator = TypescriptGenerator::default();
        let service = Arc::<()>::new(());
        let package = PackageId::new("hl7.fhir.r4.core", "4.0.1");
        generator.generate(&service, &package).expect("generate");
    }
}
