//! FHIR slice detection — language-neutral.
//!
//! Groups sliced elements into [`SlicePattern`]s with their discriminators, so
//! every backend consumes the resolved slicing structure instead of re-deriving
//! it. Backends own only the rendering of slices into their target language.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DiscriminatorType, ElementDefinition, ResourceDefinition, SliceDiscriminator};

/// Information about a single slice within a sliced element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceInfo {
    /// Slice name (e.g. `codeExt`, `valueExt`).
    pub name: String,
    /// The discriminator value for this slice (for value discriminators).
    pub discriminator_value: Option<String>,
    /// The discriminator type value (for type discriminators).
    pub discriminator_type: Option<String>,
    /// Whether this slice has a fixed constraint.
    pub has_fixed: bool,
}

/// Pattern of slicing found on a parent element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicePattern {
    /// Path to the sliced element (e.g. `Extension.extension`).
    pub path: String,
    /// The discriminator used for slicing.
    pub discriminator: Option<SliceDiscriminator>,
    /// Information about each slice.
    pub slices: Vec<SliceInfo>,
    /// Whether this is open slicing (allows unspecified values).
    pub is_open: bool,
    /// Discriminator kind, when available.
    pub discriminator_kind: Option<DiscriminatorType>,
}

/// Extract all slice patterns from a resource definition.
pub fn detect_slices(resource: &ResourceDefinition) -> Vec<SlicePattern> {
    let mut patterns = Vec::new();

    for element in &resource.elements {
        if let Some(slicing) = &element.slicing {
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

/// Extract the discriminator value from a slice element (value discriminators).
fn extract_discriminator_value(element: &ElementDefinition) -> Option<String> {
    // First, try to extract from a fixed value.
    if let Some(fixed) = &element.fixed
        && let Some(value) = discriminator_field(fixed)
    {
        return Some(value);
    }

    // Then try a pattern value.
    if let Some(pattern) = &element.pattern
        && let Some(value) = discriminator_field(pattern)
    {
        return Some(value);
    }

    None
}

/// Pull a discriminator string from a fixed/pattern value — the common
/// discriminator fields (`url`, `system`, `code`) or a bare string.
fn discriminator_field(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["url", "system", "code"] {
                if let Some(Value::String(s)) = map.get(key) {
                    return Some(s.clone());
                }
            }
            None
        }
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Extract the discriminator type from a slice element (type discriminators).
fn extract_discriminator_type(element: &ElementDefinition) -> Option<String> {
    element.types.first().map(|t| t.code.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ElementCardinality, ElementMax};
    use serde_json::json;

    fn slice_element(
        slice: &str,
        fixed: Option<Value>,
        pattern: Option<Value>,
    ) -> ElementDefinition {
        ElementDefinition {
            id: format!("Extension.extension:{slice}"),
            path: "Extension.extension".to_string(),
            slice_name: Some(slice.to_string()),
            fixed,
            pattern,
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
        }
    }

    #[test]
    fn discriminator_value_from_fixed_url() {
        let url = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race";
        let el = slice_element("race", Some(json!({ "url": url })), None);
        assert_eq!(extract_discriminator_value(&el), Some(url.to_string()));
    }

    #[test]
    fn discriminator_value_from_pattern_url() {
        let url = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity";
        let el = slice_element("ethnicity", None, Some(json!({ "url": url })));
        assert_eq!(extract_discriminator_value(&el), Some(url.to_string()));
    }
}
