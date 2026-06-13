//! Reference FHIR code generator backend for Rust.
//!
//! Demonstrates the plugin contract: implement [`Backend`] over a resolved
//! [`PackageIr`]. The backend is a pure function of the IR — no provider, no
//! resolver, no async — so it doubles as the proof that the IR carries enough,
//! language-neutrally, for any backend to generate from.
//!
//! Unlike the TypeScript backend (Tera templates), this emits Rust
//! programmatically. It is intentionally compact: correct, compiling structs,
//! not a full SDK.

use inkgen_core::{Backend, BackendError, GenerationOutput, PackageIr};

mod config;
mod generation;

pub use config::RustGeneratorConfig;

/// Rust code generator backend.
#[derive(Debug, Clone, Default)]
pub struct RustGenerator {
    config: RustGeneratorConfig,
}

impl RustGenerator {
    /// Create a new Rust generator with the given configuration.
    pub fn new(config: RustGeneratorConfig) -> Self {
        Self { config }
    }

    /// Get the current generator configuration.
    pub fn config(&self) -> &RustGeneratorConfig {
        &self.config
    }
}

impl Backend for RustGenerator {
    fn id(&self) -> &str {
        "rust"
    }

    fn generate(&self, ir: &PackageIr) -> Result<GenerationOutput, BackendError> {
        tracing::info!(
            "Rust backend: generating from PackageIr {} ({} types)",
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
            diagnostics: vec![],
        }
    }

    #[test]
    fn backend_id_is_rust() {
        let backend = RustGenerator::default();
        assert_eq!(backend.id(), "rust");
    }

    #[test]
    fn generate_empty_ir_yields_module() {
        let backend = RustGenerator::default();
        let out = backend.generate(&empty_ir()).expect("generate");
        assert!(out.files.contains_key("mod.rs"));
        let module = &out.files["mod.rs"];
        assert!(module.contains("use serde::{Deserialize, Serialize};"));
    }
}
