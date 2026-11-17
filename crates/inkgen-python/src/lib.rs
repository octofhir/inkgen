//! FHIR code generator backend for Python (Pydantic v2).
//!
//! Implements [`Backend`] over a resolved [`PackageIr`], purely — no provider,
//! no resolver, no async. Emits an importable `fhir` package: one module per
//! resource/complex type with snake_case fields aliased to FHIR wire names,
//! composite classes for backbone elements, `Literal` aliases for enumerated
//! ValueSet bindings, and `model_validator` runtime checks for choice groups
//! and XOR-exists invariants. Cross-module cycles resolve via `model_rebuild()`
//! in `__init__.py`. The generated code requires Python >= 3.11 and
//! `pydantic >= 2.11` — dependencies of the output, not of this generator.

use inkgen_core::{Backend, BackendError, GenerationOutput, PackageIr};

mod config;
mod generation;
mod invariants;
mod naming;
mod primitives;

pub use config::{PythonGeneratorConfig, PythonLanguageConfig};

/// Python (Pydantic v2) code generator backend.
#[derive(Debug, Clone, Default)]
pub struct PythonGenerator {
    config: PythonGeneratorConfig,
}

impl PythonGenerator {
    /// Create a new Python generator with the given configuration.
    pub fn new(config: PythonGeneratorConfig) -> Self {
        Self { config }
    }

    /// Get the current generator configuration.
    pub fn config(&self) -> &PythonGeneratorConfig {
        &self.config
    }
}

impl Backend for PythonGenerator {
    fn id(&self) -> &str {
        "python"
    }

    fn generate(&self, ir: &PackageIr) -> Result<GenerationOutput, BackendError> {
        tracing::info!(
            "Python backend: generating from PackageIr {} ({} types)",
            ir.id,
            ir.types().count()
        );
        generation::generate(ir, &self.config.package_name)
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
    fn backend_id_is_python() {
        assert_eq!(PythonGenerator::default().id(), "python");
    }

    #[test]
    fn generate_empty_ir_yields_package() {
        let out = PythonGenerator::default()
            .generate(&empty_ir())
            .expect("generate");
        assert!(out.files.contains_key("fhir/__init__.py"));
        assert!(out.files.contains_key("fhir/_runtime.py"));
        assert!(!out.files.contains_key("fhir/enums.py"));
    }
}
