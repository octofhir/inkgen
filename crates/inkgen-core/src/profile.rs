use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::error::{CoreError, CoreResult};
use crate::ir::{
    BindingDefinition, BindingStrength, Derivation, ElementCardinality, ElementDefinition,
    ElementMax, ElementType, ExtensionInstance, InvariantDefinition, ProfileLineage,
    ResourceDefinition, ResourceKind, SliceDiscriminator, SlicingInfo,
};

/// Pipeline for resolving StructureDefinitions into the InkGen IR.
pub struct ProfilePipeline;

impl ProfilePipeline {
    pub fn resolve(
        structure: &Value,
        _base: Option<&ResourceDefinition>,
    ) -> CoreResult<ResourceDefinition> {
        let snapshot = structure
            .get("snapshot")
            .and_then(|v| v.get("element"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CoreError::validation("StructureDefinition missing snapshot.element array")
            })?;

        let mut element_invariants: IndexMap<String, InvariantDefinition> = IndexMap::new();
        let mut elements = Vec::with_capacity(snapshot.len());
        for element_value in snapshot {
            elements.push(parse_element(element_value, &mut element_invariants)?);
        }

        // Build hierarchical element tree from flat snapshot
        elements = build_element_tree(elements);

        let mut invariants: Vec<InvariantDefinition> = element_invariants.into_values().collect();
        invariants.sort_by(|a, b| a.key.cmp(&b.key));

        let mut definition = ResourceDefinition {
            id: string_field(structure, "id")
                .ok_or_else(|| CoreError::validation("StructureDefinition missing id"))?,
            url: string_field(structure, "url")
                .ok_or_else(|| CoreError::validation("StructureDefinition missing url"))?,
            name: string_field(structure, "name"),
            title: string_field(structure, "title"),
            description: string_field(structure, "description"),
            version: string_field(structure, "version"),
            status: string_field(structure, "status"),
            kind: parse_kind(structure)?,
            fhir_type: string_field(structure, "type"),
            date: string_field(structure, "date"),
            lineage: parse_lineage(structure),
            elements,
            extensions: Vec::new(),
            invariants,
        };

        definition.sort();
        Ok(definition)
    }
}

fn parse_kind(structure: &Value) -> CoreResult<ResourceKind> {
    let kind_raw = string_field(structure, "kind")
        .ok_or_else(|| CoreError::validation("StructureDefinition missing kind"))?;
    ResourceKind::from_fhir(&kind_raw).ok_or_else(|| {
        CoreError::validation(format!(
            "Unsupported StructureDefinition kind: {}",
            kind_raw
        ))
    })
}

fn parse_lineage(structure: &Value) -> ProfileLineage {
    let base_definition = string_field(structure, "baseDefinition");
    let base_id = base_definition
        .as_ref()
        .and_then(|base| base.rsplit('/').next())
        .map(|s| s.to_string());
    let derivation = string_field(structure, "derivation").and_then(|d| Derivation::from_fhir(&d));
    let type_name = string_field(structure, "type");

    ProfileLineage {
        base_definition,
        base_id,
        derivation,
        type_name,
    }
}

fn parse_element(
    value: &Value,
    invariant_store: &mut IndexMap<String, InvariantDefinition>,
) -> CoreResult<ElementDefinition> {
    let obj = value
        .as_object()
        .ok_or_else(|| CoreError::validation("ElementDefinition entry must be a JSON object"))?;

    let id = required_string(obj, "id")?;
    let path = required_string(obj, "path")?;

    let mut element = ElementDefinition {
        id,
        path,
        slice_name: obj
            .get("sliceName")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        short: obj
            .get("short")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        definition: obj
            .get("definition")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        comment: obj
            .get("comment")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        requirements: obj
            .get("requirements")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        cardinality: parse_cardinality(obj),
        types: parse_types(obj),
        content_reference: obj
            .get("contentReference")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        binding: obj.get("binding").and_then(parse_binding),
        invariants: Vec::new(),
        fixed: extract_choice(obj, "fixed"),
        pattern: extract_choice(obj, "pattern"),
        default_value: extract_choice(obj, "defaultValue"),
        example_values: parse_examples(obj),
        must_support: obj
            .get("mustSupport")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        is_summary: obj
            .get("isSummary")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        slicing: obj.get("slicing").and_then(parse_slicing),
        extension: parse_extensions(obj),
        additional_fields: IndexMap::new(),

        // Hierarchical structure fields (initialized with defaults; will be built later)
        children: Vec::new(),
        parent_path: None,
        depth: 0,
        is_backbone: false,
    };

    if let Some(constraints) = obj.get("constraint").and_then(Value::as_array) {
        for constraint in constraints {
            if let Some(invariant) = parse_constraint(constraint) {
                element.invariants.push(invariant.key.clone());
                invariant_store
                    .entry(invariant.key.clone())
                    .or_insert(invariant);
            }
        }
    }

    Ok(element)
}

fn parse_cardinality(obj: &Map<String, Value>) -> ElementCardinality {
    let min = obj
        .get("min")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        .max(0) as u32;

    let max = obj
        .get("max")
        .and_then(Value::as_str)
        .map(|m| {
            if m == "*" {
                ElementMax::Unbounded
            } else {
                m.parse::<u32>()
                    .map(ElementMax::Finite)
                    .unwrap_or(ElementMax::Unbounded)
            }
        })
        .unwrap_or(ElementMax::Unbounded);

    ElementCardinality { min, max }
}

fn parse_types(obj: &Map<String, Value>) -> Vec<ElementType> {
    obj.get("type")
        .and_then(Value::as_array)
        .map(|types| {
            types
                .iter()
                .filter_map(|entry| entry.as_object())
                .filter_map(|entry| {
                    let code = entry.get("code")?.as_str()?.to_string();
                    let profiles = collect_strings(entry.get("profile"));
                    let target_profiles = collect_strings(entry.get("targetProfile"));
                    let aggregation = collect_strings(entry.get("aggregation"));
                    let versioning = entry
                        .get("versioning")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string());

                    Some(ElementType {
                        code,
                        profiles,
                        target_profiles,
                        aggregation,
                        versioning,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_binding(value: &Value) -> Option<BindingDefinition> {
    let obj = value.as_object()?;
    let strength = obj
        .get("strength")
        .and_then(Value::as_str)
        .and_then(BindingStrength::from_fhir)?;

    let value_set = obj
        .get("valueSet")
        .or_else(|| obj.get("valueSetCanonical"))
        .or_else(|| obj.get("valueSetUri"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let mut additional = IndexMap::new();
    for (key, val) in obj {
        if matches!(
            key.as_str(),
            "strength" | "valueSet" | "valueSetCanonical" | "valueSetUri" | "description"
        ) {
            continue;
        }
        additional.insert(key.clone(), val.clone());
    }

    Some(BindingDefinition {
        strength,
        value_set,
        description,
        additional,
    })
}

fn parse_constraint(value: &Value) -> Option<InvariantDefinition> {
    let obj = value.as_object()?;
    let key = obj.get("key")?.as_str()?.to_string();

    let mut additional = IndexMap::new();
    for (field, val) in obj {
        if matches!(
            field.as_str(),
            "key" | "severity" | "human" | "expression" | "xpath"
        ) {
            continue;
        }
        additional.insert(field.clone(), val.clone());
    }

    Some(InvariantDefinition {
        key,
        severity: obj
            .get("severity")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        human: obj
            .get("human")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        expression: obj
            .get("expression")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        xpath: obj
            .get("xpath")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        additional,
    })
}

fn parse_examples(obj: &Map<String, Value>) -> Vec<Value> {
    obj.get("example")
        .and_then(Value::as_array)
        .map(|examples| {
            examples
                .iter()
                .filter_map(|example| {
                    example
                        .as_object()
                        .and_then(|map| extract_first_value(map, "value"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_extensions(obj: &Map<String, Value>) -> Vec<ExtensionInstance> {
    obj.get("extension")
        .and_then(Value::as_array)
        .map(|exts| {
            exts.iter()
                .filter_map(|raw| raw.as_object())
                .filter_map(|entry| {
                    let url = entry.get("url")?.as_str()?.to_string();
                    let value = extract_first_value(entry, "value");
                    Some(ExtensionInstance { url, value })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_slicing(value: &Value) -> Option<SlicingInfo> {
    let obj = value.as_object()?;
    let discriminators = obj
        .get("discriminator")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_object())
                .filter_map(|entry| {
                    Some(SliceDiscriminator {
                        discriminator_type: entry.get("type")?.as_str()?.to_string(),
                        path: entry.get("path")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(SlicingInfo {
        discriminators,
        ordered: obj.get("ordered").and_then(Value::as_bool).unwrap_or(false),
        rules: obj
            .get("rules")
            .and_then(Value::as_str)
            .unwrap_or("open")
            .to_string(),
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

fn required_string(obj: &Map<String, Value>, key: &str) -> CoreResult<String> {
    obj.get(key)
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .ok_or_else(|| CoreError::validation(format!("ElementDefinition missing {key}")))
}

fn collect_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.to_string())
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn extract_choice(obj: &Map<String, Value>, prefix: &str) -> Option<Value> {
    for (key, value) in obj {
        if key.starts_with(prefix) {
            return Some(value.clone());
        }
    }
    None
}

fn extract_first_value(obj: &Map<String, Value>, prefix: &str) -> Option<Value> {
    for (key, value) in obj {
        if key.starts_with(prefix) {
            return Some(value.clone());
        }
    }
    None
}

/// Builds a hierarchical element tree from a flat list of elements.
///
/// This function:
/// 1. Calculates depth for each element based on path segments
/// 2. Groups elements by parent-child relationships
/// 3. Detects BackboneElements (elements with children but no concrete type)
/// 4. Returns only root-level elements with children nested
fn build_element_tree(mut flat_elements: Vec<ElementDefinition>) -> Vec<ElementDefinition> {
    if flat_elements.is_empty() {
        return Vec::new();
    }

    // Calculate depth and parent_path for each element
    for element in &mut flat_elements {
        element.depth = count_path_segments(&element.path);
        element.parent_path = get_parent_path(&element.path);
    }

    // Build a map of all elements by path
    let mut element_map: IndexMap<String, ElementDefinition> = flat_elements
        .into_iter()
        .map(|elem| (elem.path.clone(), elem))
        .collect();

    // Sort paths by depth (deepest first), then by path for determinism
    let mut paths_by_depth: Vec<(String, usize)> = element_map
        .iter()
        .map(|(path, elem)| (path.clone(), elem.depth))
        .collect();
    paths_by_depth.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Move children to their parents (from deepest to shallowest)
    for (path, _depth) in paths_by_depth {
        if let Some(element) = element_map.get(&path) {
            if let Some(parent_path) = element.parent_path.clone() {
                // Remove child from map (shift_remove maintains order)
                if let Some(child) = element_map.shift_remove(&path) {
                    // Add to parent's children
                    if let Some(parent) = element_map.get_mut(&parent_path) {
                        parent.children.push(child);
                    } else {
                        // Parent not in map (shouldn't happen), put child back
                        element_map.insert(path, child);
                    }
                }
            }
        }
    }

    // Detect BackboneElements: elements with children but no explicit type
    let mut elements: Vec<ElementDefinition> = element_map.into_values().collect();
    mark_backbone_elements(&mut elements);

    // Sort for deterministic output
    elements.sort_by(|a, b| a.path.cmp(&b.path));

    elements
}

/// Recursively marks elements as BackboneElements if they have children
/// but no concrete type (or type is BackboneElement/Element).
fn mark_backbone_elements(elements: &mut [ElementDefinition]) {
    for element in elements {
        if !element.children.is_empty() {
            // Has children - check if it's a BackboneElement
            let has_concrete_type = element.types.iter().any(|t| {
                !matches!(
                    t.code.as_str(),
                    "BackboneElement" | "Element" | "Base" | ""
                )
            });

            if !has_concrete_type {
                element.is_backbone = true;
            }
        }

        // Recursively process children
        mark_backbone_elements(&mut element.children);
    }
}

/// Counts the number of segments in an element path.
/// Examples:
/// - "Patient" -> 0 (root element)
/// - "Patient.name" -> 1
/// - "Patient.name.family" -> 2
fn count_path_segments(path: &str) -> usize {
    if path.is_empty() {
        return 0;
    }
    path.matches('.').count()
}

/// Extracts the parent path from an element path.
/// Examples:
/// - "Patient.name" -> Some("Patient")
/// - "Patient.name.family" -> Some("Patient.name")
/// - "Patient" -> None
fn get_parent_path(path: &str) -> Option<String> {
    path.rfind('.').map(|idx| path[..idx].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_path_segments() {
        assert_eq!(count_path_segments("Patient"), 0);
        assert_eq!(count_path_segments("Patient.name"), 1);
        assert_eq!(count_path_segments("Patient.name.family"), 2);
        assert_eq!(count_path_segments("Patient.contact.name.given"), 3);
    }

    #[test]
    fn test_get_parent_path() {
        assert_eq!(get_parent_path("Patient"), None);
        assert_eq!(
            get_parent_path("Patient.name"),
            Some("Patient".to_string())
        );
        assert_eq!(
            get_parent_path("Patient.name.family"),
            Some("Patient.name".to_string())
        );
        assert_eq!(
            get_parent_path("Patient.contact.name.given"),
            Some("Patient.contact.name".to_string())
        );
    }

    #[test]
    fn test_build_element_tree() {
        // Create flat elements
        let elements = vec![
            create_test_element("Patient", vec!["DomainResource"]),
            create_test_element("Patient.id", vec!["id"]),
            create_test_element("Patient.name", vec!["HumanName"]),
            create_test_element("Patient.contact", vec![]), // BackboneElement
            create_test_element("Patient.contact.name", vec!["HumanName"]),
            create_test_element("Patient.contact.telecom", vec!["ContactPoint"]),
        ];

        let tree = build_element_tree(elements);

        // Should have only 1 root element (Patient)
        assert_eq!(tree.len(), 1);

        let patient = &tree[0];
        assert_eq!(patient.path, "Patient");
        assert_eq!(patient.depth, 0);
        assert_eq!(patient.children.len(), 3); // id, name, contact

        // Find contact element
        let contact = patient
            .children
            .iter()
            .find(|e| e.path == "Patient.contact")
            .unwrap();
        assert_eq!(contact.depth, 1);
        assert!(contact.is_backbone); // Should be marked as BackboneElement
        assert_eq!(contact.children.len(), 2); // name, telecom
        assert_eq!(
            contact.parent_path,
            Some("Patient".to_string())
        );

        // Check nested child
        let contact_name = &contact.children[0];
        assert_eq!(contact_name.path, "Patient.contact.name");
        assert_eq!(contact_name.depth, 2);
        assert_eq!(
            contact_name.parent_path,
            Some("Patient.contact".to_string())
        );
    }

    fn create_test_element(path: &str, type_codes: Vec<&str>) -> ElementDefinition {
        ElementDefinition {
            id: path.to_string(),
            path: path.to_string(),
            slice_name: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Unbounded,
            },
            types: type_codes
                .into_iter()
                .map(|code| ElementType {
                    code: code.to_string(),
                    profiles: Vec::new(),
                    target_profiles: Vec::new(),
                    aggregation: Vec::new(),
                    versioning: None,
                })
                .collect(),
            content_reference: None,
            binding: None,
            invariants: Vec::new(),
            fixed: None,
            pattern: None,
            default_value: None,
            example_values: Vec::new(),
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: Vec::new(),
            additional_fields: IndexMap::new(),
            children: Vec::new(),
            parent_path: None,
            depth: 0,
            is_backbone: false,
        }
    }
}
