//! FHIR code generator backend for Gleam.
//!
//! Mirrors the reference Rust backend: implement [`Backend`] over a resolved
//! [`PackageIr`], purely — no provider, no resolver, no async. Emits Gleam
//! record types (one `pub type` per resolved FHIR type) into a single module so
//! cross-type references resolve without imports. Anything not itself generated
//! (backbone/unknown types) falls back to `Dynamic`, so the output type-checks.

use inkgen_core::{Backend, BackendError, GenerationOutput, PackageIr};

mod config;
mod generation;

pub use config::GleamGeneratorConfig;

/// Gleam code generator backend.
#[derive(Debug, Clone, Default)]
pub struct GleamGenerator {
    config: GleamGeneratorConfig,
}

impl GleamGenerator {
    /// Create a new Gleam generator with the given configuration.
    pub fn new(config: GleamGeneratorConfig) -> Self {
        Self { config }
    }

    /// Get the current generator configuration.
    pub fn config(&self) -> &GleamGeneratorConfig {
        &self.config
    }
}

impl Backend for GleamGenerator {
    fn id(&self) -> &str {
        "gleam"
    }

    fn generate(&self, ir: &PackageIr) -> Result<GenerationOutput, BackendError> {
        tracing::info!(
            "Gleam backend: generating from PackageIr {} ({} types)",
            ir.id,
            ir.types().count()
        );
        Ok(generation::generate(ir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use inkgen_core::CONTRACT_VERSION;

    fn empty_ir() -> PackageIr {
        PackageIr {
            contract_version: CONTRACT_VERSION,
            id: "test@0".to_string(),
            fhir_version: None,
            dependencies: vec![],
            types: IndexMap::new(),
            canonical_index: IndexMap::new(),
            type_map: Default::default(),
            structures: vec![],
            value_sets: IndexMap::new(),
            search_parameters: vec![],
            package_descriptor: None,
            diagnostics: vec![],
        }
    }

    #[test]
    fn backend_id_is_gleam() {
        let backend = GleamGenerator::default();
        assert_eq!(backend.id(), "gleam");
    }

    #[test]
    fn generate_empty_ir_yields_module() {
        let backend = GleamGenerator::default();
        let out = backend.generate(&empty_ir()).expect("generate");
        assert!(out.files.contains_key("fhir.gleam"));
    }
}
