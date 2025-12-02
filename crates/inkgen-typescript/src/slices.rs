//! Slice detection and discriminated union type generation.
//!
//! This module handles:
//! - Identifying sliced elements from IR structures
//! - Analyzing discriminator patterns (value, type, pattern, profile)
//! - Generating TypeScript discriminated union types
//! - Creating type guard functions for slice discrimination

use inkgen_core::ir::{
    DiscriminatorType, ElementDefinition, ResourceDefinition, SliceDiscriminator,
};
use serde::Serialize;
use serde_json::Value;

/// Information about a single slice within a sliced element.
#[derive(Debug, Clone, Serialize)]
pub struct SliceInfo {
    /// Slice name (e.g., "codeExt", "valueExt")
    pub name: String,
    /// The discriminator value for this slice (if value discriminator)
    pub discriminator_value: Option<String>,
    /// The discriminator type value (if type discriminator)
    pub discriminator_type: Option<String>,
    /// Whether this slice has a fixed constraint
    pub has_fixed: bool,
}

/// Pattern of slicing found in a parent element.
#[derive(Debug, Clone, Serialize)]
pub struct SlicePattern {
    /// Path to the sliced element (e.g., "Extension.extension")
    pub path: String,
    /// The discriminator used for slicing
    pub discriminator: Option<SliceDiscriminator>,
    /// Information about each slice
    pub slices: Vec<SliceInfo>,
    /// Whether this is open slicing (allows unspecified values)
    pub is_open: bool,
    /// Discriminator kind enum (if available)
    pub discriminator_kind: Option<DiscriminatorType>,
}

/// Type information for a discriminated union variant.
#[derive(Debug, Clone)]
pub struct UnionVariant {
    /// Variant name (often matches slice name)
    pub name: String,
    /// Discriminator field name (e.g., "url")
    pub discriminator_field: String,
    /// Discriminator value for this variant (if value-based)
    pub discriminator_value: Option<String>,
    /// Type constraint (e.g., literal string type)
    pub type_constraint: String,
    /// Documentation for this variant
    pub description: Option<String>,
}

/// Represents a generated discriminated union type.
#[derive(Debug, Clone)]
pub struct DiscriminatedUnion {
    /// Union type name (e.g., "ExtensionSlice")
    pub type_name: String,
    /// Variants in the union
    pub variants: Vec<UnionVariant>,
    /// Fallback/catch-all variant for open slicing
    pub catch_all_variant: Option<UnionVariant>,
    /// Documentation
    pub description: Option<String>,
}

/// Type guard function for a single slice.
#[derive(Debug, Clone)]
pub struct SliceTypeGuard {
    /// Function name (e.g., "isCodeExtension")
    pub function_name: String,
    /// Input parameter type
    pub input_type: String,
    /// Return type (narrowed)
    pub return_type: String,
    /// Guard condition
    pub condition: String,
}

/// Extract all slice patterns from a resource definition.
pub fn detect_slices(resource: &ResourceDefinition) -> Vec<SlicePattern> {
    let mut patterns = Vec::new();

    // Find all parent elements that have slicing metadata
    for element in &resource.elements {
        if let Some(slicing) = &element.slicing {
            // Find all child slices for this parent
            let slices = find_slices_for_parent(&resource.elements, &element.path);

            if !slices.is_empty() {
                let is_open = slicing.rules.to_lowercase() == "open"
                    || slicing.rules.to_lowercase() == "openat";

                let discriminator = slicing.discriminators.first().cloned();
                let discriminator_kind = discriminator.as_ref().and_then(|d| d.kind);

                patterns.push(SlicePattern {
                    path: element.path.clone(),
                    discriminator,
                    slices,
                    is_open,
                    discriminator_kind,
                });
            }
        }
    }

    patterns
}

/// Find all slices for a given parent element path.
fn find_slices_for_parent(elements: &[ElementDefinition], parent_path: &str) -> Vec<SliceInfo> {
    elements
        .iter()
        .filter(|elem| elem.path == parent_path && elem.slice_name.is_some())
        .map(|elem| SliceInfo {
            name: elem.slice_name.as_ref().unwrap().clone(),
            discriminator_value: extract_discriminator_value(elem),
            discriminator_type: extract_discriminator_type(elem),
            has_fixed: elem.fixed.is_some(),
        })
        .collect()
}

/// Extract the discriminator value from a slice element (for value discriminators).
fn extract_discriminator_value(element: &ElementDefinition) -> Option<String> {
    // First, try to extract from fixed value
    if let Some(fixed) = &element.fixed {
        match fixed {
            Value::Object(map) => {
                // For value discriminators, try to extract the discriminator field value
                // Common discriminator fields: url, system, code, value
                if let Some(Value::String(url)) = map.get("url") {
                    return Some(url.clone());
                }
                if let Some(Value::String(system)) = map.get("system") {
                    return Some(system.clone());
                }
                if let Some(Value::String(code)) = map.get("code") {
                    return Some(code.clone());
                }
            }
            Value::String(s) => {
                return Some(s.clone());
            }
            _ => {}
        }
    }

    // Try pattern if fixed is not available
    if let Some(pattern) = &element.pattern {
        match pattern {
            Value::Object(map) => {
                if let Some(Value::String(url)) = map.get("url") {
                    return Some(url.clone());
                }
                if let Some(Value::String(system)) = map.get("system") {
                    return Some(system.clone());
                }
                if let Some(Value::String(code)) = map.get("code") {
                    return Some(code.clone());
                }
            }
            Value::String(s) => {
                return Some(s.clone());
            }
            _ => {}
        }
    }

    None
}

/// Extract the discriminator type from a slice element (for type discriminators).
fn extract_discriminator_type(element: &ElementDefinition) -> Option<String> {
    // If there are specific types defined, use the first one
    if !element.types.is_empty() {
        return Some(element.types[0].code.clone());
    }

    None
}

/// Generate a discriminated union type from a slice pattern.
pub fn generate_union_type(
    parent_element: &ElementDefinition,
    pattern: &SlicePattern,
) -> Option<DiscriminatedUnion> {
    // Determine union type name from parent element path
    let type_name = slice_union_type_name(&pattern.path);

    // Get discriminator field name from the discriminator path
    let discriminator_field = pattern
        .discriminator
        .as_ref()
        .and_then(|d| d.path.split('.').next_back())
        .unwrap_or("value")
        .to_string();

    // Generate variants for each slice
    let mut variants = Vec::new();
    for slice in &pattern.slices {
        let variant_type_constraint = if let Some(ref disc_type) = slice.discriminator_type {
            disc_type.clone()
        } else if let Some(ref disc_value) = slice.discriminator_value {
            format!("'{}'", disc_value)
        } else {
            "unknown".to_string()
        };

        variants.push(UnionVariant {
            name: slice.name.clone(),
            discriminator_field: discriminator_field.clone(),
            discriminator_value: slice.discriminator_value.clone(),
            type_constraint: variant_type_constraint,
            description: None,
        });
    }

    // Add catch-all for open slicing
    let catch_all_variant = if pattern.is_open {
        Some(UnionVariant {
            name: "default".to_string(),
            discriminator_field: discriminator_field.clone(),
            discriminator_value: None,
            type_constraint: "string".to_string(),
            description: Some("Catch-all for open slicing".to_string()),
        })
    } else {
        None
    };

    Some(DiscriminatedUnion {
        type_name,
        variants,
        catch_all_variant,
        description: parent_element.definition.clone(),
    })
}

/// Generate type guard function for a slice.
pub fn generate_slice_type_guard(
    union_type_name: &str,
    slice: &SliceInfo,
    discriminator_field: &str,
    discriminator_value: &Option<String>,
) -> SliceTypeGuard {
    let function_name = format!("is{}", to_pascal_case(&slice.name));
    let input_type = union_type_name.to_string();
    let narrowed_type = format!(
        "{{ {}: {} }}",
        discriminator_field,
        discriminator_value
            .as_ref()
            .map(|v| format!("'{}'", v))
            .unwrap_or_else(|| "unknown".to_string())
    );

    let condition = if let Some(value) = discriminator_value {
        format!("(input as any).{} === '{}'", discriminator_field, value)
    } else {
        format!("typeof (input as any).{} === 'string'", discriminator_field)
    };

    SliceTypeGuard {
        function_name,
        input_type,
        return_type: narrowed_type,
        condition,
    }
}

/// Convert a slice name to PascalCase for type names.
fn to_pascal_case(input: &str) -> String {
    input
        .split(&['_', '-'][..])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Generate union type name from element path.
pub fn slice_union_type_name(path: &str) -> String {
    // Convert "Extension.extension" to "ExtensionSlice"
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() > 1 {
        let camel = to_pascal_case(parts[parts.len() - 1]);
        format!("{}Slice", camel)
    } else {
        format!("{}Slice", to_pascal_case(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_union_type_name() {
        assert_eq!(
            slice_union_type_name("Extension.extension"),
            "ExtensionSlice"
        );
    }

    #[test]
    fn test_slice_union_type_name_kebab_case() {
        assert_eq!(
            slice_union_type_name("Observation.component"),
            "ComponentSlice"
        );
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("code_ext"), "CodeExt");
        assert_eq!(to_pascal_case("value-ext"), "ValueExt");
        assert_eq!(to_pascal_case("simple"), "Simple");
    }

    #[test]
    fn test_slice_type_guard_generation() {
        let slice = SliceInfo {
            name: "codeExt".to_string(),
            discriminator_value: Some("http://example.org/code".to_string()),
            discriminator_type: None,
            has_fixed: true,
        };

        let guard = generate_slice_type_guard(
            "ExtensionSlice",
            &slice,
            "url",
            &Some("http://example.org/code".to_string()),
        );

        assert_eq!(guard.function_name, "isCodeExt");
        assert_eq!(guard.input_type, "ExtensionSlice");
        assert!(guard.condition.contains("url"));
        assert!(guard.condition.contains("http://example.org/code"));
    }

    #[test]
    fn test_extract_discriminator_value_from_fixed_url() {
        use inkgen_core::ir::{ElementCardinality, ElementMax};
        use serde_json::json;

        let element = ElementDefinition {
            id: "Extension.extension:race".to_string(),
            path: "Extension.extension".to_string(),
            slice_name: Some("race".to_string()),
            fixed: Some(json!({
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race"
            })),
            pattern: None,
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Finite(1),
            },
            types: Vec::new(),
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            content_reference: None,
            binding: None,
            invariants: Vec::new(),
            default_value: None,
            example_values: Vec::new(),
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: Vec::new(),
            additional_fields: indexmap::IndexMap::new(),
            children: Vec::new(),
            parent_path: None,
            depth: 0,
            is_backbone: false,
        };

        let discriminator_value = extract_discriminator_value(&element);
        assert_eq!(
            discriminator_value,
            Some("http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string())
        );
    }

    #[test]
    fn test_extract_discriminator_value_from_pattern_url() {
        use inkgen_core::ir::{ElementCardinality, ElementMax};
        use serde_json::json;

        let element = ElementDefinition {
            id: "Extension.extension:ethnicity".to_string(),
            path: "Extension.extension".to_string(),
            slice_name: Some("ethnicity".to_string()),
            fixed: None,
            pattern: Some(json!({
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity"
            })),
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Finite(1),
            },
            types: Vec::new(),
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            content_reference: None,
            binding: None,
            invariants: Vec::new(),
            default_value: None,
            example_values: Vec::new(),
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: Vec::new(),
            additional_fields: indexmap::IndexMap::new(),
            children: Vec::new(),
            parent_path: None,
            depth: 0,
            is_backbone: false,
        };

        let discriminator_value = extract_discriminator_value(&element);
        assert_eq!(
            discriminator_value,
            Some("http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity".to_string())
        );
    }
}
