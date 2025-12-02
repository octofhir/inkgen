//! Zod schema generation backend for FHIR types.
//!
//! This module provides a ValidationBackend implementation using Zod
//! for runtime validation of FHIR resources and data types.

use inkgen_core::ir::{ElementCardinality, ElementDefinition, ElementMax};

use super::ValidationBackend;

/// Zod validation backend
///
/// Generates Zod schemas and validators for TypeScript types.
#[derive(Debug, Clone, Default)]
pub struct ZodBackend;

impl ZodBackend {
    /// Creates a new Zod backend
    pub fn new() -> Self {
        Self
    }
}

impl ValidationBackend for ZodBackend {
    fn name(&self) -> &str {
        "zod"
    }

    fn generate_schema(&self, element: &ElementDefinition, type_name: &str) -> String {
        if let Some(info) = element_to_zod_schema_info(element) {
            format!("export const {}Schema = {};", type_name, info.schema)
        } else {
            format!("export const {}Schema = z.unknown();", type_name)
        }
    }

    fn generate_imports(&self) -> Vec<String> {
        vec!["import { z } from 'zod';".to_string()]
    }

    fn generate_validator_function(&self, type_name: &str, schema_name: &str) -> String {
        format!(
            r#"export function parse{}(data: unknown): {} {{
  return {}.parse(data);
}}

export type {}Validated = z.infer<typeof {}Schema>;"#,
            type_name, type_name, schema_name, type_name, type_name
        )
    }

    fn supports_lazy_loading(&self) -> bool {
        true
    }

    fn generate_element_validator(
        &self,
        element: &ElementDefinition,
        field_name: &str,
        field_type: &str,
    ) -> Option<String> {
        element_to_zod_schema_info(element).map(|info| {
            format!(
                r#"export function validate{}(value: unknown): {} {{
  return {}.parse(value);
}}"#,
                capitalize_first(field_name),
                field_type,
                info.schema
            )
        })
    }

    fn supports_modular_validation(&self) -> bool {
        true
    }
}

/// Information about a Zod schema including the schema expression and type references.
#[derive(Debug, Clone)]
pub struct ZodSchemaInfo {
    /// The Zod schema expression (e.g., "z.string()" or "AddressSchema")
    pub schema: String,
    /// Complex types referenced that need schema imports (e.g., ["Address", "Identifier"])
    /// Does NOT include primitives since they use inline Zod validators
    pub type_refs: Vec<String>,
}

/// Normalize a FHIR type reference to a simple type name.
///
/// Handles both simple type names and full URLs.
/// Examples:
/// - "string" -> "string"
/// - "http://hl7.org/fhirpath/System.String" -> "string"
/// - "http://hl7.org/fhir/StructureDefinition/Address" -> "Address"
fn normalize_type_name(fhir_type: &str) -> String {
    // If it's a URL, extract the last part
    if fhir_type.starts_with("http://") || fhir_type.starts_with("https://") {
        let last_part = fhir_type.split('/').next_back().unwrap_or(fhir_type);

        // For System types like "System.String", extract after the dot
        if let Some(type_name) = last_part.split('.').next_back() {
            // Convert to lowercase for primitives (String -> string)
            let normalized = type_name.to_string();
            // Check if it's a known primitive that should be lowercase
            match normalized.as_str() {
                "String" => return "string".to_string(),
                "Boolean" => return "boolean".to_string(),
                "Integer" => return "integer".to_string(),
                "Decimal" => return "decimal".to_string(),
                "Date" => return "date".to_string(),
                "DateTime" => return "dateTime".to_string(),
                "Time" => return "time".to_string(),
                "Instant" => return "instant".to_string(),
                _ => return normalized,
            }
        }
        last_part.to_string()
    } else {
        fhir_type.to_string()
    }
}

/// Map FHIR primitive types to Zod validators.
///
/// Returns ZodSchemaInfo containing the Zod expression and type references.
/// Primitive types use inline Zod validators (no type refs).
/// Complex types return a reference to their schema with the type added to type_refs.
pub fn fhir_to_zod_type(fhir_type: &str) -> ZodSchemaInfo {
    let normalized = normalize_type_name(fhir_type);
    let (schema, is_complex) = match normalized.as_str() {
        // String-like primitives
        "string" | "markdown" => ("z.string()".to_string(), false),
        "code" => ("z.string()".to_string(), false), // Could add enum validation
        "id" => ("z.string().regex(/^[A-Za-z0-9\\-\\.]{1,64}$/)".to_string(), false),
        "uri" => ("z.string().url()".to_string(), false),
        "url" => ("z.string().url()".to_string(), false),
        "canonical" => ("z.string().url()".to_string(), false),
        "oid" => ("z.string().regex(/^urn:oid:[0-2](\\.(0|[1-9][0-9]*))+$/)".to_string(), false),
        "uuid" => ("z.string().uuid()".to_string(), false),

        // Date/time primitives
        "date" => ("z.string().regex(/^\\d{4}(-\\d{2}(-\\d{2})?)?$/)".to_string(), false),
        "dateTime" => ("z.string().regex(/^\\d{4}(-\\d{2}(-\\d{2}(T\\d{2}:\\d{2}(:\\d{2}(\\.\\d+)?)?(Z|[+-]\\d{2}:\\d{2})?)?)?)?$/)".to_string(), false),
        "instant" => ("z.string().datetime()".to_string(), false),
        "time" => ("z.string().regex(/^\\d{2}:\\d{2}(:\\d{2}(\\.\\d+)?)?$/)".to_string(), false),

        // Numeric primitives
        "boolean" => ("z.boolean()".to_string(), false),
        "integer" => ("z.number().int()".to_string(), false),
        "unsignedInt" => ("z.number().int().nonnegative()".to_string(), false),
        "positiveInt" => ("z.number().int().positive()".to_string(), false),
        "decimal" => ("z.number()".to_string(), false),

        // Base64
        "base64Binary" => ("z.string().regex(/^[A-Za-z0-9+/]*={0,2}$/)".to_string(), false),

        // XHTML
        "xhtml" => ("z.string()".to_string(), false),

        // Complex types - reference their schemas
        _ => (format!("{}Schema", normalized), true),
    };

    ZodSchemaInfo {
        schema,
        type_refs: if is_complex {
            vec![normalized]
        } else {
            Vec::new()
        },
    }
}

/// Apply cardinality constraints to a Zod type.
///
/// Handles array wrapping, min/max items, and optional/required.
/// Preserves type references from the input ZodSchemaInfo.
pub fn apply_cardinality(
    base_info: ZodSchemaInfo,
    cardinality: &ElementCardinality,
) -> ZodSchemaInfo {
    let mut schema = base_info.schema;

    let is_array = match &cardinality.max {
        ElementMax::Unbounded => true,
        ElementMax::Finite(n) => *n > 1,
    };

    if is_array {
        // Wrap in array
        schema = format!("z.array({})", schema);

        // Add min items constraint
        if cardinality.min > 0 {
            schema = format!("{}.min({})", schema, cardinality.min);
        }

        // Add max items constraint if finite
        if let ElementMax::Finite(max) = &cardinality.max
            && *max > 1
        {
            schema = format!("{}.max({})", schema, max);
        }
    }

    // Optional if min cardinality is 0
    if cardinality.min == 0 {
        schema = format!("{}.optional()", schema);
    }

    ZodSchemaInfo {
        schema,
        type_refs: base_info.type_refs,
    }
}

/// Generate Zod schema for an element with type tracking.
pub fn element_to_zod_schema_info(element: &ElementDefinition) -> Option<ZodSchemaInfo> {
    // Skip elements without types
    if element.types.is_empty() {
        return None;
    }

    // Get the base Zod type
    let base_info = if element.types.len() == 1 {
        // Single type
        fhir_to_zod_type(&element.types[0].code)
    } else {
        // Choice type - union of possibilities
        let mut all_type_refs = Vec::new();
        let union_schemas: Vec<String> = element
            .types
            .iter()
            .map(|t| {
                let info = fhir_to_zod_type(&t.code);
                all_type_refs.extend(info.type_refs);
                info.schema
            })
            .collect();

        ZodSchemaInfo {
            schema: format!("z.union([{}])", union_schemas.join(", ")),
            type_refs: all_type_refs,
        }
    };

    // Apply cardinality
    let schema_info = apply_cardinality(base_info, &element.cardinality);

    Some(schema_info)
}

/// Generate Zod schema for an element (schema string only, for backward compatibility).
pub fn element_to_zod_schema(element: &ElementDefinition) -> Option<String> {
    element_to_zod_schema_info(element).map(|info| info.schema)
}

/// Generate field name for an element in a Zod schema.
pub fn element_to_field_name(element: &ElementDefinition) -> String {
    element
        .path
        .split('.')
        .next_back()
        .unwrap_or(&element.path)
        .to_string()
}

/// Capitalize the first character of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkgen_core::ir::{ElementCardinality, ElementMax};

    #[test]
    fn test_zod_backend_trait() {
        let backend = ZodBackend::new();

        assert_eq!(backend.name(), "zod");
        assert!(backend.supports_lazy_loading());
        assert!(backend.supports_modular_validation());

        let imports = backend.generate_imports();
        assert_eq!(imports.len(), 1);
        assert!(imports[0].contains("zod"));
    }

    #[test]
    fn test_normalize_type_name() {
        // Simple type names pass through
        assert_eq!(normalize_type_name("string"), "string");
        assert_eq!(normalize_type_name("Address"), "Address");

        // URLs are normalized
        assert_eq!(
            normalize_type_name("http://hl7.org/fhirpath/System.String"),
            "string"
        );
        assert_eq!(
            normalize_type_name("http://hl7.org/fhirpath/System.Boolean"),
            "boolean"
        );
        assert_eq!(
            normalize_type_name("http://hl7.org/fhir/StructureDefinition/Address"),
            "Address"
        );
    }

    #[test]
    fn test_fhir_to_zod_primitive_types() {
        assert_eq!(fhir_to_zod_type("string").schema, "z.string()");
        assert_eq!(fhir_to_zod_type("boolean").schema, "z.boolean()");
        assert_eq!(fhir_to_zod_type("integer").schema, "z.number().int()");

        // Test with URLs
        assert_eq!(
            fhir_to_zod_type("http://hl7.org/fhirpath/System.String").schema,
            "z.string()"
        );
        assert_eq!(
            fhir_to_zod_type("http://hl7.org/fhirpath/System.Boolean").schema,
            "z.boolean()"
        );
        assert_eq!(fhir_to_zod_type("decimal").schema, "z.number()");
        assert!(fhir_to_zod_type("id").schema.contains("regex"));
        assert!(fhir_to_zod_type("date").schema.contains("regex"));
    }

    #[test]
    fn test_fhir_to_zod_complex_types() {
        let identifier_info = fhir_to_zod_type("Identifier");
        assert_eq!(identifier_info.schema, "IdentifierSchema");
        assert_eq!(identifier_info.type_refs, vec!["Identifier"]);

        let address_info = fhir_to_zod_type("Address");
        assert_eq!(address_info.schema, "AddressSchema");
        assert_eq!(address_info.type_refs, vec!["Address"]);
    }

    #[test]
    fn test_apply_cardinality_required() {
        let base = ZodSchemaInfo {
            schema: "z.string()".to_string(),
            type_refs: vec![],
        };

        let card = ElementCardinality {
            min: 1,
            max: ElementMax::Finite(1),
        };

        let result = apply_cardinality(base, &card);
        assert_eq!(result.schema, "z.string()");
    }

    #[test]
    fn test_apply_cardinality_optional() {
        let base = ZodSchemaInfo {
            schema: "z.string()".to_string(),
            type_refs: vec![],
        };

        let card = ElementCardinality {
            min: 0,
            max: ElementMax::Finite(1),
        };

        let result = apply_cardinality(base, &card);
        assert_eq!(result.schema, "z.string().optional()");
    }

    #[test]
    fn test_apply_cardinality_array_optional() {
        let base = ZodSchemaInfo {
            schema: "z.string()".to_string(),
            type_refs: vec![],
        };

        let card = ElementCardinality {
            min: 0,
            max: ElementMax::Unbounded,
        };

        let result = apply_cardinality(base, &card);
        assert_eq!(result.schema, "z.array(z.string()).optional()");
    }

    #[test]
    fn test_apply_cardinality_array_required() {
        let base = ZodSchemaInfo {
            schema: "z.string()".to_string(),
            type_refs: vec![],
        };

        let card = ElementCardinality {
            min: 1,
            max: ElementMax::Unbounded,
        };

        let result = apply_cardinality(base, &card);
        assert_eq!(result.schema, "z.array(z.string()).min(1)");
    }

    #[test]
    fn test_apply_cardinality_array_with_max() {
        let base = ZodSchemaInfo {
            schema: "z.string()".to_string(),
            type_refs: vec![],
        };

        let card = ElementCardinality {
            min: 1,
            max: ElementMax::Finite(5),
        };

        let result = apply_cardinality(base, &card);
        assert_eq!(result.schema, "z.array(z.string()).min(1).max(5)");
    }
}
