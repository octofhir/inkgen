//! TypeScript-specific template functions for Tera.
//!
//! This module provides custom Tera functions specifically for TypeScript code generation.
//! Unlike filters (which use pipe syntax), functions are called with named arguments.
//!
//! # Design
//!
//! This module contains ONLY TypeScript-specific functions:
//! - `is_primitive()` - Check if FHIR type is primitive
//! - `ts_type()` - Map FHIR types to TypeScript branded types
//!
//! Generic/reusable functions are in `inkgen_core::template_helpers`:
//! - `import_path()` - Calculate relative imports (language-agnostic)
//! - `package_folder()` - Sanitize package names (language-agnostic)

use std::collections::HashMap;
use tera::{Error as TeraError, Function, Result as TeraResult, Value};

/// Check if a type is a FHIR primitive type.
///
/// # Usage in templates
///
/// ```tera
/// {% if is_primitive(type="string") %}
///     // Handle primitive type
/// {% endif %}
/// ```
///
/// # Arguments
///
/// - `type` (required): The type name to check
///
/// # Returns
///
/// Boolean indicating whether the type is a FHIR primitive.
pub struct IsPrimitiveFunction;

impl Function for IsPrimitiveFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let type_name = args
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| TeraError::msg("is_primitive requires 'type' argument"))?;

        let is_primitive = is_fhir_primitive(type_name);

        Ok(Value::Bool(is_primitive))
    }
}

/// Map a FHIR type to its TypeScript equivalent.
///
/// # Usage in templates
///
/// ```tera
/// const value: {{ ts_type(fhir_type="string") }}; // const value: FhirString;
/// ```
///
/// # Arguments
///
/// - `fhir_type` (required): The FHIR type name
///
/// # Returns
///
/// String containing the TypeScript type name.
pub struct TypeScriptTypeFunction;

impl Function for TypeScriptTypeFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let fhir_type = args
            .get("fhir_type")
            .and_then(Value::as_str)
            .ok_or_else(|| TeraError::msg("ts_type requires 'fhir_type' argument"))?;

        let ts_type = map_fhir_to_typescript(fhir_type);

        Ok(Value::String(ts_type))
    }
}

/// Check if a type name corresponds to a FHIR primitive type.
fn is_fhir_primitive(type_name: &str) -> bool {
    matches!(
        type_name,
        "boolean"
            | "integer"
            | "decimal"
            | "positiveInt"
            | "unsignedInt"
            | "integer64"
            | "string"
            | "code"
            | "id"
            | "markdown"
            | "oid"
            | "uri"
            | "canonical"
            | "url"
            | "uuid"
            | "base64Binary"
            | "date"
            | "dateTime"
            | "time"
            | "instant"
            | "xhtml"
            // Also check branded type names
            | "FhirBoolean"
            | "FhirInteger"
            | "FhirDecimal"
            | "FhirPositiveInt"
            | "FhirUnsignedInt"
            | "FhirInteger64"
            | "FhirString"
            | "FhirCode"
            | "FhirId"
            | "FhirMarkdown"
            | "FhirOid"
            | "FhirUri"
            | "FhirCanonical"
            | "FhirUrl"
            | "FhirUuid"
            | "FhirBase64Binary"
            | "FhirDate"
            | "FhirDateTime"
            | "FhirTime"
            | "FhirInstant"
            | "FhirXhtml"
    )
}

/// Map a FHIR type to its TypeScript branded type equivalent.
fn map_fhir_to_typescript(fhir_type: &str) -> String {
    match fhir_type {
        "boolean" => "FhirBoolean".to_string(),
        "integer" => "FhirInteger".to_string(),
        "decimal" => "FhirDecimal".to_string(),
        "positiveInt" => "FhirPositiveInt".to_string(),
        "unsignedInt" => "FhirUnsignedInt".to_string(),
        "integer64" => "FhirInteger64".to_string(),
        "string" => "FhirString".to_string(),
        "code" => "FhirCode".to_string(),
        "id" => "FhirId".to_string(),
        "markdown" => "FhirMarkdown".to_string(),
        "oid" => "FhirOid".to_string(),
        "uri" => "FhirUri".to_string(),
        "canonical" => "FhirCanonical".to_string(),
        "url" => "FhirUrl".to_string(),
        "uuid" => "FhirUuid".to_string(),
        "base64Binary" => "FhirBase64Binary".to_string(),
        "date" => "FhirDate".to_string(),
        "dateTime" => "FhirDateTime".to_string(),
        "time" => "FhirTime".to_string(),
        "instant" => "FhirInstant".to_string(),
        "xhtml" => "FhirXhtml".to_string(),
        // Pass through if already branded
        _ => fhir_type.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_fhir_primitive() {
        // Standard FHIR primitives
        assert!(is_fhir_primitive("string"));
        assert!(is_fhir_primitive("boolean"));
        assert!(is_fhir_primitive("integer"));
        assert!(is_fhir_primitive("decimal"));
        assert!(is_fhir_primitive("uri"));
        assert!(is_fhir_primitive("dateTime"));

        // Branded TypeScript types
        assert!(is_fhir_primitive("FhirString"));
        assert!(is_fhir_primitive("FhirBoolean"));
        assert!(is_fhir_primitive("FhirInteger"));

        // Non-primitives
        assert!(!is_fhir_primitive("Patient"));
        assert!(!is_fhir_primitive("Observation"));
        assert!(!is_fhir_primitive("CodeableConcept"));
    }

    #[test]
    fn test_map_fhir_to_typescript() {
        assert_eq!(map_fhir_to_typescript("string"), "FhirString");
        assert_eq!(map_fhir_to_typescript("boolean"), "FhirBoolean");
        assert_eq!(map_fhir_to_typescript("integer"), "FhirInteger");
        assert_eq!(map_fhir_to_typescript("dateTime"), "FhirDateTime");

        // Pass through complex types
        assert_eq!(map_fhir_to_typescript("Patient"), "Patient");
        assert_eq!(map_fhir_to_typescript("CodeableConcept"), "CodeableConcept");
    }

    #[test]
    fn test_is_primitive_function() {
        let func = IsPrimitiveFunction;
        let mut args = HashMap::new();

        args.insert("type".to_string(), Value::String("string".to_string()));
        let result = func.call(&args).unwrap();
        assert_eq!(result, Value::Bool(true));

        args.insert("type".to_string(), Value::String("Patient".to_string()));
        let result = func.call(&args).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_ts_type_function() {
        let func = TypeScriptTypeFunction;
        let mut args = HashMap::new();

        args.insert("fhir_type".to_string(), Value::String("string".to_string()));
        let result = func.call(&args).unwrap();
        assert_eq!(result, Value::String("FhirString".to_string()));

        args.insert(
            "fhir_type".to_string(),
            Value::String("Patient".to_string()),
        );
        let result = func.call(&args).unwrap();
        assert_eq!(result, Value::String("Patient".to_string()));
    }

    #[test]
    fn test_function_missing_args() {
        let func = IsPrimitiveFunction;
        let args = HashMap::new();
        assert!(func.call(&args).is_err());

        let func = TypeScriptTypeFunction;
        let args = HashMap::new();
        assert!(func.call(&args).is_err());
    }
}
