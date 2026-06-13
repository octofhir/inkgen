//! Extension type generation and helper function rendering.
//!
//! This module handles:
//! - Extracting extension metadata from IR ResourceDefinition
//! - Generating TypeScript interface definitions for extensions
//! - Creating typed accessor functions for safe extension access
//! - Handling both simple and complex extensions

use crate::naming;
use crate::nested::NestedTypeInfo;
use indexmap::IndexMap;
use inkgen_core::ir::{
    ElementDefinition, ExtensionDefinition, ResourceDefinition, choice_variant_name,
};
use serde::Serialize;

/// Metadata extracted from an extension definition for TypeScript rendering.
#[derive(Debug, Clone, Serialize)]
pub struct RenderExtension {
    /// Extension URL
    pub url: String,
    /// TypeScript type name for this extension (e.g., "USCoreRaceExtension")
    pub type_name: String,
    /// Contexts where this extension can appear
    pub contexts: Vec<ExtensionContextInfo>,
    /// Whether this extension has complex structure (multiple fields)
    pub is_complex: bool,
    /// The value type of the extension (for simple extensions)
    pub value_type: Option<String>,
    /// Raw FHIR type code of a single-typed simple extension's `value[x]`
    /// (e.g. `dateTime`, `Reference`). Used to build the wire-correct value
    /// member name (`valueDateTime`, `valueReference`). `None` when the value is
    /// untyped, complex, or a multi-type choice. Distinct from `value_type`,
    /// which is the (lossy) TypeScript type — `dateTime` and `string` both map to
    /// the TS `string`, so the code is needed to pick the right member.
    pub value_type_code: Option<String>,
    /// Wire-correct value member name for a single-typed simple extension
    /// (`valueDateTime`, `valueReference`), derived from `value_type_code` via the
    /// core `choice_variant_name`. `None` for complex / untyped / multi-type
    /// extensions, where the accessor returns the raw extension instead.
    pub value_member: Option<String>,
    /// Nested types (for complex extensions)
    pub nested_types: Vec<NestedTypeInfo>,
    /// Extension cardinality (min/max)
    pub cardinality_min: u32,
    pub cardinality_max: Option<u32>,
    /// Documentation/description
    pub description: Option<String>,
}

/// Context information for where an extension can appear.
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionContextInfo {
    /// Resource or element path where extension can appear
    pub context: String,
    /// Context type: "element", "resource", "extension"
    pub context_type: String,
}

/// Helper function for accessing an extension value from a resource.
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionAccessorFunction {
    /// Function name (e.g., "getUSCoreRaceExtension")
    pub function_name: String,
    /// Extension URL being accessed
    pub extension_url: String,
    /// TypeScript return type
    pub return_type: String,
    /// Parameter type (e.g., "Patient")
    pub parameter_type: String,
    /// Whether to return an array or single value
    pub is_array: bool,
}

/// Extract all extensions from a resource definition.
pub fn extract_extensions(resource: &ResourceDefinition) -> IndexMap<String, RenderExtension> {
    let mut extensions = IndexMap::new();

    // Process each extension definition in the resource
    for ext_def in &resource.extensions {
        if let Some(render_ext) = create_render_extension(ext_def) {
            extensions.insert(ext_def.url.clone(), render_ext);
        }
    }

    // For Extension StructureDefinitions (type="Extension", derivation="constraint"):
    // Convert the ResourceDefinition itself to an extension
    if resource.is_extension_definition()
        && let Some(render_ext) = create_render_extension_from_resource(resource)
    {
        extensions.insert(resource.url.clone(), render_ext);
    }

    // For profiles: extract extension slices from elements
    extract_extension_slices_from_elements(&resource.elements, &mut extensions);

    extensions
}

/// Create a RenderExtension from an Extension StructureDefinition ResourceDefinition.
fn create_render_extension_from_resource(resource: &ResourceDefinition) -> Option<RenderExtension> {
    let type_name = extension_url_to_type_name(&resource.url);

    // Use flat_elements which preserves slice information
    let flat_elements = &resource.flat_elements;

    tracing::info!(
        "create_render_extension_from_resource: url={}, flat_elements={}",
        resource.url,
        flat_elements.len()
    );

    // Extract contexts
    let contexts = vec![]; // TODO: extract from resource.context if available

    // Extension shape analysis (FHIR-neutral) lives in inkgen-core.
    let is_complex = resource.extension_is_complex();

    // For simple extensions, map the value[x] type to a TypeScript type.
    let value_elem = if !is_complex {
        flat_elements
            .iter()
            .find(|elem| elem.path == "Extension.value[x]")
    } else {
        None
    };
    let value_type = value_elem
        .and_then(|elem| elem.types.first())
        .map(|t| element_type_to_typescript_from_type(&t.code));
    // The wire-faithful type code (single-typed value only) comes from core.
    let value_type_code = resource.simple_extension_value_type_code();

    // Collect nested types for complex extensions
    let nested_types = if is_complex {
        collect_nested_from_elements_flat(flat_elements)
    } else {
        Vec::new()
    };

    tracing::info!(
        "Created RenderExtension from StructureDefinition: url={}, is_complex={}, nested_types={}",
        resource.url,
        is_complex,
        nested_types.len()
    );

    Some(RenderExtension {
        url: resource.url.clone(),
        type_name,
        contexts,
        is_complex,
        value_type,
        value_member: value_type_code
            .as_deref()
            .map(|code| choice_variant_name("value", code)),
        value_type_code,
        nested_types,
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: resource.description.clone(),
    })
}

/// Collect nested types from Extension.extension slices in elements.
///
/// This works with a FLAT list of elements to preserve slice information.
/// The tree structure loses slices because it merges elements with the same path.
fn collect_nested_from_elements_flat(flat_elements: &[ElementDefinition]) -> Vec<NestedTypeInfo> {
    let mut nested_types = Vec::new();

    for (idx, element) in flat_elements.iter().enumerate() {
        // Look for Extension.extension:sliceName
        if element.path == "Extension.extension"
            && let Some(slice_name) = element.slice_name.as_ref()
        {
            // The next few elements should be the child elements of this slice
            // Look for the value[x] element immediately following
            let value_type = flat_elements
                .iter()
                .skip(idx + 1)
                .take(3)
                .find(|e| e.path == "Extension.extension.value[x]")
                .and_then(|value_elem| value_elem.types.first())
                .map(|t| element_type_to_typescript_from_type(&t.code))
                .unwrap_or_else(|| "unknown".to_string());

            nested_types.push(NestedTypeInfo {
                type_name: slice_name.clone(),
                element_path: element.path.clone(),
                base_type: value_type.clone(),
                children: vec![],
                doc_comment: element.definition.clone().or_else(|| element.short.clone()),
                depth: 0,
            });

            tracing::debug!(
                "Found sub-extension in StructureDefinition: slice_name={}, value_type={}",
                slice_name,
                value_type
            );
        }
    }

    tracing::info!(
        "collect_nested_from_elements_flat: found {} nested types",
        nested_types.len()
    );

    nested_types
}

/// Extract extension slices from profile differential elements.
/// Looks for elements like "Resource.extension:sliceName" with type=Extension.
fn extract_extension_slices_from_elements(
    elements: &[inkgen_core::ir::ElementDefinition],
    extensions: &mut IndexMap<String, RenderExtension>,
) {
    for element in elements {
        // Check if this is an extension slice
        if element.path.contains(".extension") && element.slice_name.is_some() {
            // Extract the extension URL from the type profile
            if let Some(ext_url) = element
                .types
                .iter()
                .find(|t| t.code == "Extension")
                .and_then(|t| t.profiles.first())
                .map(|p| p.as_str())
            {
                tracing::info!(
                    "Found extension slice: path={}, slice_name={:?}, url={}",
                    element.path,
                    element.slice_name,
                    ext_url
                );

                // Create RenderExtension for this slice
                let type_name = extension_url_to_type_name(ext_url);

                extensions.insert(
                    ext_url.to_string(),
                    RenderExtension {
                        url: ext_url.to_string(),
                        type_name,
                        description: element.definition.clone().or_else(|| element.short.clone()),
                        is_complex: false,     // Will be determined later if needed
                        value_type: None,      // Will be populated if simple extension
                        value_type_code: None, // Will be populated if simple extension
                        value_member: None,    // Will be populated if simple extension
                        nested_types: Vec::new(), // Will be populated if complex
                        contexts: Vec::new(),  // Could extract from element path
                        cardinality_min: element.cardinality.min,
                        cardinality_max: match element.cardinality.max {
                            inkgen_core::ir::ElementMax::Finite(n) => Some(n),
                            inkgen_core::ir::ElementMax::Unbounded => None,
                        },
                    },
                );
            }
        }

        // Recursively process children
        extract_extension_slices_from_elements(&element.children, extensions);
    }
}

/// Create a RenderExtension from an ExtensionDefinition.
fn create_render_extension(ext_def: &ExtensionDefinition) -> Option<RenderExtension> {
    let type_name = extension_url_to_type_name(&ext_def.url);

    // Extract contexts
    let contexts = ext_def
        .context
        .iter()
        .map(|ctx| ExtensionContextInfo {
            context: ctx.context.clone(),
            context_type: ctx.context_type.clone(),
        })
        .collect();

    // Determine if complex and extract value type
    let (is_complex, value_type) = analyze_extension_structure(ext_def);

    // Raw FHIR code for a single-typed simple extension's value (wire member name).
    let value_type_code = if is_complex {
        None
    } else {
        find_value_element(ext_def)
            .filter(|elem| elem.types.len() == 1)
            .and_then(|elem| elem.types.first())
            .map(|t| t.code.clone())
    };

    // Collect nested types for complex extensions
    let nested_types = if is_complex {
        collect_extension_nested_types(ext_def)
    } else {
        Vec::new()
    };

    // Extract cardinality
    let (cardinality_min, cardinality_max) = ext_def
        .cardinality
        .as_ref()
        .map(|card| {
            let max = match &card.max {
                inkgen_core::ir::ElementMax::Finite(n) => Some(*n),
                inkgen_core::ir::ElementMax::Unbounded => None,
            };
            (card.min, max)
        })
        .unwrap_or((1, Some(1)));

    Some(RenderExtension {
        url: ext_def.url.clone(),
        type_name,
        contexts,
        is_complex,
        value_type,
        value_member: value_type_code
            .as_deref()
            .map(|code| choice_variant_name("value", code)),
        value_type_code,
        nested_types,
        cardinality_min,
        cardinality_max,
        description: ext_def.description.clone(),
    })
}

/// Analyze the extension structure to determine complexity and value type.
fn analyze_extension_structure(ext_def: &ExtensionDefinition) -> (bool, Option<String>) {
    // If marked as complex or has children elements beyond the root
    if ext_def.is_complex || ext_def.elements.len() > 1 {
        return (true, None);
    }

    // For simple extensions, find the value element type
    if let Some(value_elem) = find_value_element(ext_def) {
        let type_name = element_type_to_typescript(value_elem);
        return (false, Some(type_name));
    }

    (false, None)
}

/// Find the value element in an extension definition.
fn find_value_element(ext_def: &ExtensionDefinition) -> Option<&ElementDefinition> {
    // First check if value_element field is specified
    if let Some(value_elem_path) = &ext_def.value_element {
        return ext_def
            .elements
            .iter()
            .find(|elem| elem.path == *value_elem_path);
    }

    // Otherwise, look for an element named "value[x]" or the actual value element
    if let Some(elem) = ext_def
        .elements
        .iter()
        .find(|elem| elem.path.ends_with(".value"))
    {
        return Some(elem);
    }

    ext_def
        .elements
        .iter()
        .find(|elem| elem.path.contains("value["))
}

/// Collect nested types from extension structure.
///
/// For complex extensions, this extracts sub-extensions (Extension.extension slices)
/// and creates NestedTypeInfo for each one.
fn collect_extension_nested_types(ext_def: &ExtensionDefinition) -> Vec<NestedTypeInfo> {
    let mut nested_types = Vec::new();

    // Find all Extension.extension:sliceName elements
    for element in &ext_def.elements {
        // Look for extension slices: "Extension.extension:sliceName" or just "Extension.extension"
        if element.path == "Extension.extension"
            && let Some(slice_name) = element.slice_name.as_ref()
        {
            // Find the corresponding value[x] element for this slice
            let value_elem_path = format!("{}.value[x]", element.path);
            let value_type = ext_def
                .elements
                .iter()
                .find(|e| {
                    e.path == value_elem_path
                        || e.path.starts_with(&format!("{}.value", element.path))
                })
                .and_then(|value_elem| {
                    // Get the first type from the element's types
                    value_elem
                        .types
                        .first()
                        .map(|t| element_type_to_typescript_from_type(&t.code))
                })
                .unwrap_or_else(|| "unknown".to_string());

            // Get cardinality
            let cardinality_min = element.cardinality.min;
            let cardinality_max = match element.cardinality.max {
                inkgen_core::ir::ElementMax::Finite(n) => Some(n),
                inkgen_core::ir::ElementMax::Unbounded => None,
            };

            // Create NestedTypeInfo for this sub-extension
            nested_types.push(NestedTypeInfo {
                type_name: slice_name.clone(),
                element_path: element.path.clone(),
                base_type: value_type.clone(),
                children: vec![], // Sub-extensions don't have children
                doc_comment: element.definition.clone().or_else(|| element.short.clone()),
                depth: 0,
            });

            tracing::debug!(
                "Found sub-extension: slice_name={}, value_type={}, min={}, max={:?}",
                slice_name,
                value_type,
                cardinality_min,
                cardinality_max
            );
        }
    }

    tracing::info!(
        "Collected {} nested types for extension {}",
        nested_types.len(),
        ext_def.url
    );

    nested_types
}

/// Convert a FHIR type code string to TypeScript type.
fn element_type_to_typescript_from_type(type_code: &str) -> String {
    match type_code {
        "string" | "uri" | "url" | "canonical" | "code" | "oid" | "id" | "uuid" | "markdown" => {
            "string".to_string()
        }
        "integer" | "unsignedInt" | "positiveInt" => "number".to_string(),
        "boolean" => "boolean".to_string(),
        "decimal" => "number".to_string(),
        "date" | "dateTime" | "time" | "instant" => "string".to_string(),
        "base64Binary" => "string".to_string(),
        // Complex types - keep as-is
        "Duration" => "Duration".to_string(),
        "Period" => "Period".to_string(),
        "CodeableConcept" => "CodeableConcept".to_string(),
        "Coding" => "Coding".to_string(),
        "Reference" => "Reference".to_string(),
        "Identifier" => "Identifier".to_string(),
        "Quantity" => "Quantity".to_string(),
        "Range" => "Range".to_string(),
        "Ratio" => "Ratio".to_string(),
        "Attachment" => "Attachment".to_string(),
        "Address" => "Address".to_string(),
        "ContactPoint" => "ContactPoint".to_string(),
        "HumanName" => "HumanName".to_string(),
        _ => {
            tracing::warn!("Unknown FHIR type code: {}", type_code);
            "unknown".to_string()
        }
    }
}

/// Convert a FHIR element type to TypeScript type.
fn element_type_to_typescript(element: &ElementDefinition) -> String {
    if let Some(binding) = &element.binding {
        // Coded element - use CodeableConcept or Coding
        if binding.value_set.is_some() {
            "CodeableConcept".to_string()
        } else {
            "string".to_string()
        }
    } else if !element.types.is_empty() {
        match element.types[0].code.as_str() {
            "string" => "string".to_string(),
            "integer" => "number".to_string(),
            "boolean" => "boolean".to_string(),
            "decimal" => "number".to_string(),
            "date" => "string".to_string(),
            "dateTime" => "string".to_string(),
            "time" => "string".to_string(),
            "CodeableConcept" => "CodeableConcept".to_string(),
            "Coding" => "Coding".to_string(),
            "Reference" => "Reference".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        "unknown".to_string()
    }
}

/// Convert an extension URL to a TypeScript type name.
pub fn extension_url_to_type_name(url: &str) -> String {
    // Extract the last component of the URL path
    let last_component = url.split('/').next_back().unwrap_or("Extension");

    // Use naming::pascal_case which handles sanitization (prefixes digits, etc.)
    let base_name = naming::pascal_case(last_component);
    format!("{}Extension", base_name)
}

/// Generate accessor function information for extensions on a resource.
pub fn generate_accessor_functions(
    resource: &ResourceDefinition,
    extensions: &IndexMap<String, RenderExtension>,
) -> Vec<ExtensionAccessorFunction> {
    let resource_name = &resource.id;
    let mut accessors = Vec::new();

    // For each extension that applies to this resource
    for (url, ext) in extensions {
        if should_generate_accessor(resource_name, ext) {
            let function_name = format!("get{}", ext.type_name);
            let return_type = ext
                .value_type
                .clone()
                .unwrap_or_else(|| ext.type_name.clone());

            accessors.push(ExtensionAccessorFunction {
                function_name,
                extension_url: url.clone(),
                return_type,
                parameter_type: resource_name.clone(),
                is_array: ext.cardinality_max.is_none() || ext.cardinality_max.unwrap() > 1,
            });
        }
    }

    accessors
}

/// Check if an extension applies to a resource and should have an accessor generated.
fn should_generate_accessor(resource_name: &str, extension: &RenderExtension) -> bool {
    extension.contexts.iter().any(|ctx| {
        // Match resource-level contexts or element paths within this resource
        ctx.context == resource_name || ctx.context.starts_with(&format!("{}.", resource_name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_url_to_type_name_simple() {
        assert_eq!(
            extension_url_to_type_name("http://example.org/fhir/extension/custom"),
            "CustomExtension"
        );
    }

    #[test]
    fn test_extension_url_to_type_name_kebab_case() {
        assert_eq!(
            extension_url_to_type_name(
                "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race"
            ),
            "UsCoreRaceExtension"
        );
    }

    #[test]
    fn test_extension_url_to_type_name_snake_case() {
        assert_eq!(
            extension_url_to_type_name("http://example.org/fhir/extension/my_custom_ext"),
            "MyCustomExtExtension"
        );
    }
}
