//! Zod schema generation for FHIR types.
//!
//! This module provides utilities for generating Zod schemas for runtime validation
//! of FHIR resources and data types.

use inkgen_core::ir::{ElementCardinality, ElementDefinition, ElementMax};

/// Map FHIR primitive types to Zod validators.
///
/// Returns the Zod expression for validating a FHIR primitive type.
/// Complex types return a reference to their schema (e.g., "IdentifierSchema").
pub fn fhir_to_zod_type(fhir_type: &str) -> String {
    match fhir_type {
        // String-like primitives
        "string" | "markdown" => "z.string()".to_string(),
        "code" => "z.string()".to_string(), // Could add enum validation
        "id" => "z.string().regex(/^[A-Za-z0-9\\-\\.]{1,64}$/)".to_string(),
        "uri" => "z.string().url()".to_string(),
        "url" => "z.string().url()".to_string(),
        "canonical" => "z.string().url()".to_string(),
        "oid" => "z.string().regex(/^urn:oid:[0-2](\\.(0|[1-9][0-9]*))+$/)".to_string(),
        "uuid" => "z.string().uuid()".to_string(),

        // Date/time primitives
        "date" => "z.string().regex(/^\\d{4}(-\\d{2}(-\\d{2})?)?$/)".to_string(),
        "dateTime" => "z.string().regex(/^\\d{4}(-\\d{2}(-\\d{2}(T\\d{2}:\\d{2}(:\\d{2}(\\.\\d+)?)?(Z|[+-]\\d{2}:\\d{2})?)?)?)?$/)".to_string(),
        "instant" => "z.string().datetime()".to_string(),
        "time" => "z.string().regex(/^\\d{2}:\\d{2}(:\\d{2}(\\.\\d+)?)?$/)".to_string(),

        // Numeric primitives
        "boolean" => "z.boolean()".to_string(),
        "integer" => "z.number().int()".to_string(),
        "unsignedInt" => "z.number().int().nonnegative()".to_string(),
        "positiveInt" => "z.number().int().positive()".to_string(),
        "decimal" => "z.number()".to_string(),

        // Base64
        "base64Binary" => "z.string().regex(/^[A-Za-z0-9+/]*={0,2}$/)".to_string(),

        // XHTML
        "xhtml" => "z.string()".to_string(),

        // Complex types - reference their schemas
        _ => format!("{}Schema", fhir_type),
    }
}

/// Apply cardinality constraints to a Zod type.
///
/// Handles array wrapping, min/max items, and optional/required.
pub fn apply_cardinality(base_type: &str, cardinality: &ElementCardinality) -> String {
    let mut schema = base_type.to_string();

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
        if let ElementMax::Finite(max) = &cardinality.max {
            if *max > 1 {
                schema = format!("{}.max({})", schema, max);
            }
        }
    }

    // Optional if min cardinality is 0
    if cardinality.min == 0 {
        schema = format!("{}.optional()", schema);
    }

    schema
}

/// Generate Zod schema for an element.
pub fn element_to_zod_schema(element: &ElementDefinition) -> Option<String> {
    // Skip elements without types
    if element.types.is_empty() {
        return None;
    }

    // Get the base Zod type
    let base_type = if element.types.len() == 1 {
        // Single type
        fhir_to_zod_type(&element.types[0].code)
    } else {
        // Choice type - union of possibilities
        let union_types: Vec<String> = element.types
            .iter()
            .map(|t| fhir_to_zod_type(&t.code))
            .collect();
        format!("z.union([{}])", union_types.join(", "))
    };

    // Apply cardinality
    let schema = apply_cardinality(&base_type, &element.cardinality);

    Some(schema)
}

/// Generate field name for an element in a Zod schema.
pub fn element_to_field_name(element: &ElementDefinition) -> String {
    element
        .path
        .split('.')
        .last()
        .unwrap_or(&element.path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkgen_core::ir::{ElementMax, ElementCardinality};

    #[test]
    fn test_fhir_to_zod_primitive_types() {
        assert_eq!(fhir_to_zod_type("string"), "z.string()");
        assert_eq!(fhir_to_zod_type("boolean"), "z.boolean()");
        assert_eq!(fhir_to_zod_type("integer"), "z.number().int()");
        assert_eq!(fhir_to_zod_type("decimal"), "z.number()");
        assert!(fhir_to_zod_type("id").contains("regex"));
        assert!(fhir_to_zod_type("date").contains("regex"));
    }

    #[test]
    fn test_fhir_to_zod_complex_types() {
        assert_eq!(fhir_to_zod_type("Identifier"), "IdentifierSchema");
        assert_eq!(fhir_to_zod_type("HumanName"), "HumanNameSchema");
        assert_eq!(fhir_to_zod_type("CodeableConcept"), "CodeableConceptSchema");
    }

    #[test]
    fn test_apply_cardinality_optional() {
        let cardinality = ElementCardinality {
            min: 0,
            max: ElementMax::Finite(1),
        };
        assert_eq!(
            apply_cardinality("z.string()", &cardinality),
            "z.string().optional()"
        );
    }

    #[test]
    fn test_apply_cardinality_required() {
        let cardinality = ElementCardinality {
            min: 1,
            max: ElementMax::Finite(1),
        };
        assert_eq!(
            apply_cardinality("z.string()", &cardinality),
            "z.string()"
        );
    }

    #[test]
    fn test_apply_cardinality_array_optional() {
        let cardinality = ElementCardinality {
            min: 0,
            max: ElementMax::Unbounded,
        };
        assert_eq!(
            apply_cardinality("z.string()", &cardinality),
            "z.array(z.string()).optional()"
        );
    }

    #[test]
    fn test_apply_cardinality_array_required() {
        let cardinality = ElementCardinality {
            min: 1,
            max: ElementMax::Unbounded,
        };
        assert_eq!(
            apply_cardinality("z.string()", &cardinality),
            "z.array(z.string()).min(1)"
        );
    }

    #[test]
    fn test_apply_cardinality_array_with_max() {
        let cardinality = ElementCardinality {
            min: 1,
            max: ElementMax::Finite(5),
        };
        assert_eq!(
            apply_cardinality("z.string()", &cardinality),
            "z.array(z.string()).min(1).max(5)"
        );
    }
}
