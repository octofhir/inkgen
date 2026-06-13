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
    /// For an `exists` discriminator: whether this slice requires the
    /// discriminating element to be present (`Some(true)`, `min >= 1`), forbids
    /// it (`Some(false)`, `max == 0`), or it does not apply / is undetermined
    /// (`None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminator_exists: Option<bool>,
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
///
/// Reads `flat_elements` — the flattened snapshot — rather than the nested
/// `elements` tree: for profiles the nested tree is sparse (constraints and
/// slice members live only in the flat snapshot), so the nested view misses
/// slicing entirely. The flat list carries the slicing parent, its slice
/// members, and their cardinality/fixed/pattern for every definition.
pub fn detect_slices(resource: &ResourceDefinition) -> Vec<SlicePattern> {
    let mut patterns = Vec::new();

    for element in &resource.flat_elements {
        if let Some(slicing) = &element.slicing {
            let mut slices = find_slices_for_parent(&resource.flat_elements, &element.path);

            if !slices.is_empty() {
                let is_open = slicing.rules.to_lowercase() == "open"
                    || slicing.rules.to_lowercase() == "openat";

                let discriminator = slicing.discriminators.first().cloned();
                let discriminator_kind = discriminator.as_ref().and_then(|d| d.kind);

                // For an `exists` discriminator, resolve per slice whether the
                // discriminating element must be present.
                if discriminator_kind == Some(DiscriminatorType::Exists)
                    && let Some(disc) = &discriminator
                {
                    for slice in &mut slices {
                        slice.discriminator_exists =
                            detect_exists(resource, &element.path, &slice.name, &disc.path);
                    }
                }

                // For a `value`/`pattern` discriminator the fixed value often
                // lives on a descendant of the slice (e.g. `bp`'s component
                // slices fix `code.coding.code`), not the slice header. When the
                // header carries no value, resolve it by walking the
                // discriminator path within the slice's flat descendants.
                if matches!(
                    discriminator_kind,
                    Some(DiscriminatorType::Value | DiscriminatorType::Pattern)
                ) && let Some(disc) = &discriminator
                {
                    for slice in &mut slices {
                        if slice.discriminator_value.is_none() {
                            slice.discriminator_value =
                                detect_value(resource, &element.path, &slice.name, &disc.path);
                        }
                    }
                }

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
            discriminator_exists: None,
            has_fixed: elem.fixed.is_some(),
        })
        .collect()
}

/// Resolve an `exists` discriminator for one slice: does the slice require the
/// element at `disc_path` (relative to the sliced element `parent_path`) to be
/// present? Reads `flat_elements`, whose ids retain the slice name
/// (`Observation.component:systolic.code`).
fn detect_exists(
    resource: &ResourceDefinition,
    parent_path: &str,
    slice_name: &str,
    disc_path: &str,
) -> Option<bool> {
    let elem = find_slice_discriminator_element(resource, parent_path, slice_name, disc_path)?;

    if matches!(elem.cardinality.max, super::ElementMax::Finite(0)) {
        Some(false)
    } else if elem.cardinality.min >= 1 {
        Some(true)
    } else {
        None
    }
}

/// Resolve the fixed/pattern value of a `value`/`pattern` discriminator for one
/// slice by reading the discriminating element resolved from the slice's flat
/// descendants. Returns the discriminator string (e.g. `bp`'s `8480-6`).
fn detect_value(
    resource: &ResourceDefinition,
    parent_path: &str,
    slice_name: &str,
    disc_path: &str,
) -> Option<String> {
    let elem = find_slice_discriminator_element(resource, parent_path, slice_name, disc_path)?;
    elem.fixed
        .as_ref()
        .and_then(discriminator_field)
        .or_else(|| elem.pattern.as_ref().and_then(discriminator_field))
}

/// Find the flat element that carries a slice's discriminating value/cardinality
/// at `disc_path` (relative to the sliced element `parent_path`). `$this`
/// targets the slice header itself; otherwise the discriminating element is the
/// slice descendant at `{parent_path}.{disc_path}` whose id retains the slice
/// name (`Observation.component:SystolicBP.code.coding.code`).
fn find_slice_discriminator_element<'a>(
    resource: &'a ResourceDefinition,
    parent_path: &str,
    slice_name: &str,
    disc_path: &str,
) -> Option<&'a ElementDefinition> {
    let (target_path, id_marker) = if disc_path == "$this" {
        (parent_path.to_string(), format!(":{slice_name}"))
    } else {
        (
            format!("{parent_path}.{disc_path}"),
            format!(":{slice_name}."),
        )
    };

    resource
        .flat_elements
        .iter()
        .find(|e| e.path == target_path && e.id.contains(&id_marker))
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

    fn flat_element(id: &str, path: &str, min: u32, max: ElementMax) -> ElementDefinition {
        let mut e = slice_element("x", None, None);
        e.id = id.to_string();
        e.path = path.to_string();
        e.slice_name = None;
        e.cardinality = ElementCardinality { min, max };
        e
    }

    fn resource_with_flat(flat: Vec<ElementDefinition>) -> ResourceDefinition {
        use crate::ir::{ProfileLineage, ResourceKind};
        ResourceDefinition {
            id: "Observation".to_string(),
            url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
            name: None,
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: Some("Observation".to_string()),
            date: None,
            lineage: ProfileLineage::default(),
            elements: Vec::new(),
            flat_elements: flat,
            extensions: Vec::new(),
            invariants: Vec::new(),
        }
    }

    #[test]
    fn value_resolved_from_a_slice_descendant() {
        // `bp`-style: the component slice's discriminating value lives on a
        // descendant (`code.coding.code`), not the slice header.
        let mut el = flat_element(
            "Observation.component:SystolicBP.code.coding.code",
            "Observation.component.code.coding.code",
            1,
            ElementMax::Finite(1),
        );
        el.fixed = Some(json!("8480-6"));
        let r = resource_with_flat(vec![el]);
        assert_eq!(
            detect_value(
                &r,
                "Observation.component",
                "SystolicBP",
                "code.coding.code"
            ),
            Some("8480-6".to_string())
        );
        // A different slice name does not match.
        assert_eq!(
            detect_value(
                &r,
                "Observation.component",
                "DiastolicBP",
                "code.coding.code"
            ),
            None
        );
    }

    #[test]
    fn value_resolved_from_a_descendant_pattern() {
        let mut el = flat_element(
            "Observation.component:DiastolicBP.code.coding.code",
            "Observation.component.code.coding.code",
            1,
            ElementMax::Finite(1),
        );
        el.pattern = Some(json!({ "code": "8462-4" }));
        let r = resource_with_flat(vec![el]);
        assert_eq!(
            detect_value(
                &r,
                "Observation.component",
                "DiastolicBP",
                "code.coding.code"
            ),
            Some("8462-4".to_string())
        );
    }

    #[test]
    fn exists_true_when_slice_requires_element() {
        let r = resource_with_flat(vec![flat_element(
            "Observation.component:systolic.code",
            "Observation.component.code",
            1,
            ElementMax::Finite(1),
        )]);
        assert_eq!(
            detect_exists(&r, "Observation.component", "systolic", "code"),
            Some(true)
        );
    }

    #[test]
    fn exists_false_when_slice_forbids_element() {
        let r = resource_with_flat(vec![flat_element(
            "Observation.component:systolic.code",
            "Observation.component.code",
            0,
            ElementMax::Finite(0),
        )]);
        assert_eq!(
            detect_exists(&r, "Observation.component", "systolic", "code"),
            Some(false)
        );
    }

    #[test]
    fn exists_none_when_unconstrained_or_missing() {
        let r = resource_with_flat(vec![flat_element(
            "Observation.component:systolic.code",
            "Observation.component.code",
            0,
            ElementMax::Finite(1),
        )]);
        assert_eq!(
            detect_exists(&r, "Observation.component", "systolic", "code"),
            None
        );
        // No matching flat element at all.
        let empty = resource_with_flat(Vec::new());
        assert_eq!(
            detect_exists(&empty, "Observation.component", "systolic", "code"),
            None
        );
    }

    #[test]
    fn exists_this_targets_the_slice_element() {
        let r = resource_with_flat(vec![flat_element(
            "Observation.component:systolic",
            "Observation.component",
            1,
            ElementMax::Finite(1),
        )]);
        assert_eq!(
            detect_exists(&r, "Observation.component", "systolic", "$this"),
            Some(true)
        );
    }
}
