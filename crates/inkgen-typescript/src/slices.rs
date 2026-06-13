//! Slice detection and discriminated union type generation.
//!
//! This module handles:
//! - Identifying sliced elements from IR structures
//! - Analyzing discriminator patterns (value, type, pattern, profile)
//! - Generating TypeScript discriminated union types
//! - Creating type guard functions for slice discrimination

use inkgen_core::ir::ElementDefinition;

// Slice DETECTION (SlicePattern, SliceInfo, detect_slices) is FHIR-neutral and
// now lives in inkgen-core; this module keeps only the TypeScript rendering of
// the detected slices. Re-exported so existing `slices::…` paths keep working.
pub use inkgen_core::ir::{SliceInfo, SlicePattern, detect_slices};

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
            discriminator_exists: None,
            discriminator_values: Vec::new(),
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
}
