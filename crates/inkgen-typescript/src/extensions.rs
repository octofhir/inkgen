//! Extension type generation and helper function rendering.
//!
//! This module handles:
//! - Extracting extension metadata from IR ResourceDefinition
//! - Generating TypeScript interface definitions for extensions
//! - Creating typed accessor functions for safe extension access
//! - Handling both simple and complex extensions

use crate::nested::NestedTypeInfo;
use indexmap::IndexMap;
use inkgen_core::ir::{ElementDefinition, ExtensionDefinition, ResourceDefinition};
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

    extensions
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
fn collect_extension_nested_types(_ext_def: &ExtensionDefinition) -> Vec<NestedTypeInfo> {
    // Reuse the nested type collection logic from nested.rs
    // For now, we'll create a simple implementation
    // This mirrors the pattern used in nested.rs::collect_nested_types
    Vec::new()
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

    // Convert kebab-case or snake_case to PascalCase
    last_component
        .split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>()
        + "Extension"
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
