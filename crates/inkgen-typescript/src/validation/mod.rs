/// Validation backend system for generating runtime validation code
///
/// This module provides an abstraction layer for generating validation code
/// with different libraries (Zod, JSON Schema, superstruct, io-ts, ArkType, etc.)
use inkgen_core::ir::ElementDefinition;

pub mod zod;

pub use zod::ZodBackend;

/// Backend for generating runtime validation code
///
/// Implementors generate validation schemas and validator functions
/// for a specific validation library.
pub trait ValidationBackend: Send + Sync {
    /// Returns the name of this validation backend
    ///
    /// Used for configuration and logging
    fn name(&self) -> &str;

    /// Generates a validation schema for an element
    ///
    /// # Arguments
    /// * `element` - The FHIR element definition to generate validation for
    /// * `type_name` - The TypeScript type name for the element
    ///
    /// # Returns
    /// TypeScript code defining the validation schema
    fn generate_schema(&self, element: &ElementDefinition, type_name: &str) -> String;

    /// Generates import statements needed for this backend
    ///
    /// # Returns
    /// Vector of TypeScript import statements
    ///
    /// # Example
    /// ```rust,no_run
    /// # use inkgen_typescript::validation::ValidationBackend;
    /// # struct MyBackend;
    /// # impl ValidationBackend for MyBackend {
    /// #     fn name(&self) -> &str { "my-backend" }
    /// #     fn generate_schema(&self, _element: &inkgen_core::ir::ElementDefinition, _type_name: &str) -> String { String::new() }
    /// fn generate_imports(&self) -> Vec<String> {
    ///     vec!["import { z } from 'zod';".to_string()]
    /// }
    /// #     fn generate_validator_function(&self, _type_name: &str, _schema_name: &str) -> String { String::new() }
    /// #     fn supports_lazy_loading(&self) -> bool { false }
    /// # }
    /// ```
    fn generate_imports(&self) -> Vec<String>;

    /// Generates a validator function for a type
    ///
    /// # Arguments
    /// * `type_name` - The TypeScript type name
    /// * `schema_name` - The name of the schema variable
    ///
    /// # Returns
    /// TypeScript function that validates and returns the typed result
    ///
    /// # Example
    /// ```typescript
    /// export function validatePatient(data: unknown): Patient {
    ///   return PatientSchema.parse(data);
    /// }
    /// ```
    fn generate_validator_function(&self, type_name: &str, schema_name: &str) -> String;

    /// Returns whether this backend supports lazy schema loading
    ///
    /// If true, schemas can be imported dynamically to reduce initial bundle size
    fn supports_lazy_loading(&self) -> bool;

    /// Generates a lazy-loaded schema import
    ///
    /// Only called if `supports_lazy_loading()` returns true
    ///
    /// # Arguments
    /// * `type_name` - The TypeScript type name
    /// * `import_path` - The path to import the schema from
    ///
    /// # Returns
    /// TypeScript code for lazy schema loading
    ///
    /// # Example
    /// ```typescript
    /// export const PatientSchema = () =>
    ///   import('./patient.schemas').then(m => m.PatientSchema);
    /// ```
    fn generate_lazy_schema_import(&self, type_name: &str, import_path: &str) -> String {
        format!(
            "export const {}Schema = () => import('{}').then(m => m.{}Schema);",
            type_name, import_path, type_name
        )
    }

    /// Generates validation code for modular (per-element) validation
    ///
    /// This allows validating individual fields rather than entire resources
    ///
    /// # Arguments
    /// * `element` - The FHIR element definition
    /// * `field_name` - The field name in the parent type
    /// * `field_type` - The TypeScript type for the field
    ///
    /// # Returns
    /// TypeScript function that validates just this field
    fn generate_element_validator(
        &self,
        element: &ElementDefinition,
        field_name: &str,
        field_type: &str,
    ) -> Option<String> {
        // Default implementation returns None (not supported)
        let _ = (element, field_name, field_type);
        None
    }

    /// Returns whether this backend can generate element-level validators
    fn supports_modular_validation(&self) -> bool {
        false
    }
}

/// Configuration for validation generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationConfig {
    /// Whether to generate validation code
    pub enabled: bool,
    /// Whether to use lazy loading for schemas
    pub lazy_loading: bool,
    /// Whether to generate modular (per-element) validators
    pub modular: bool,
    /// Whether to colocate schemas with types
    pub colocated: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            lazy_loading: false,
            modular: false,
            colocated: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend;

    impl ValidationBackend for MockBackend {
        fn name(&self) -> &str {
            "mock"
        }

        fn generate_schema(&self, _element: &ElementDefinition, type_name: &str) -> String {
            format!("const {}Schema = mockSchema();", type_name)
        }

        fn generate_imports(&self) -> Vec<String> {
            vec!["import { mockSchema } from 'mock-lib';".to_string()]
        }

        fn generate_validator_function(&self, type_name: &str, schema_name: &str) -> String {
            format!(
                "export function validate{}(data: unknown): {} {{ return {}.parse(data); }}",
                type_name, type_name, schema_name
            )
        }

        fn supports_lazy_loading(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_validation_backend_trait() {
        let backend = MockBackend;

        assert_eq!(backend.name(), "mock");
        assert!(backend.supports_lazy_loading());
        assert!(!backend.supports_modular_validation());

        let imports = backend.generate_imports();
        assert_eq!(imports.len(), 1);
        assert!(imports[0].contains("mock-lib"));
    }

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();

        assert!(config.enabled);
        assert!(!config.lazy_loading);
        assert!(!config.modular);
        assert!(config.colocated);
    }
}
