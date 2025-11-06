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
