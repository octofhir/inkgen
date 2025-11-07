//! FHIR Code Generator for Rust
//!
//! This is an example backend that demonstrates how to implement a language generator
//! using the `LanguageGenerator` trait from `inkgen-core`. Unlike the TypeScript backend
//! which uses Tera templates, this backend generates Rust code programmatically.
//!
//! # Architecture
//!
//! The Rust backend is designed to:
//! - Generate idiomatic Rust structs with serde support
//! - Provide builder patterns for complex types
//! - Generate validation helpers for FHIR constraints
//! - Demonstrate mixing programmatic and template-based emission
//!
//! # Usage
//!
//! This backend is feature-flagged and can be toggled via `backend-rust` feature:
//!
//! ```toml
//! [features]
//! default = ["backend-typescript"]
//! backend-typescript = []
//! backend-rust = []
//! ```

use anyhow::Result;
use async_trait::async_trait;

use inkgen_core::{
    LanguageGenerator, PackageDescriptor, StructureDefinitionProvider, StructureProviderConfig,
};

mod config;
mod generation;

pub use config::RustGeneratorConfig;

/// Configuration for the Rust code generator
#[derive(Debug, Clone)]
pub struct RustGenerator {
    config: RustGeneratorConfig,
}

impl RustGenerator {
    /// Create a new Rust generator with the given configuration
    pub fn new(config: RustGeneratorConfig) -> Self {
        Self { config }
    }

    /// Get the current generator configuration
    pub fn config(&self) -> &RustGeneratorConfig {
        &self.config
    }
}

/// Implement the LanguageGenerator trait for Rust
#[async_trait]
impl<S> LanguageGenerator<S> for RustGenerator
where
    S: StructureDefinitionProvider + Sync + Send,
{
    async fn generate(
        &self,
        _service: &S,
        descriptor: &PackageDescriptor,
        _provider_config: &StructureProviderConfig,
    ) -> Result<()> {
        tracing::info!(
            "Starting Rust generation for package: {} v{}",
            descriptor.id.name,
            descriptor.id.version
        );

        // Phase 1: Get structures from package descriptor
        let structures = descriptor.structures();

        if structures.is_empty() {
            tracing::warn!("No structures found for package: {}", descriptor.id.name);
            return Ok(());
        }

        tracing::info!("Found {} structures to generate", structures.len());

        // Phase 2: Generate code for each structure (in this example we skip load_structure)
        // In a real implementation, you would load each structure and generate code
        for summary in structures {
            tracing::debug!(
                "Would generate for: {}",
                summary.name.as_deref().unwrap_or("Unknown")
            );
        }

        // Phase 3: Generate module index file
        generation::generate_module_index(&self.config, structures.len())?;

        tracing::info!(
            "Rust generation complete for package: {} ({} structures)",
            descriptor.id.name,
            structures.len()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_rust_generator_creates_config() {
        let config = RustGeneratorConfig {
            output_dir: PathBuf::from("./test-output"),
        };

        let generator = RustGenerator::new(config.clone());
        assert_eq!(
            generator.config().output_dir,
            PathBuf::from("./test-output")
        );
    }

    #[test]
    fn test_generator_debug_impl() {
        let config = RustGeneratorConfig {
            output_dir: PathBuf::from("./test"),
        };
        let generator = RustGenerator::new(config);

        let debug_str = format!("{:?}", generator);
        assert!(debug_str.contains("RustGenerator"));
    }
}
