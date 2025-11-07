//! Code generation for Rust structures

use anyhow::Context;
use anyhow::Result;
use std::fs;

use inkgen_core::ir::ResourceDefinition;

use crate::config::RustGeneratorConfig;

/// Generate a Rust struct for a FHIR structure definition
#[allow(dead_code)]
pub fn generate_structure(
    config: &RustGeneratorConfig,
    definition: &ResourceDefinition,
) -> Result<()> {
    // Ensure output directory exists
    fs::create_dir_all(&config.output_dir).context("Failed to create output directory")?;

    // Get the structure name
    let name = definition.name.as_deref().unwrap_or("Unknown");

    // Generate struct code
    let struct_code = generate_struct_definition(definition);

    // Determine output filename
    let filename = format!("{}.rs", name.to_lowercase());
    let output_path = config.output_dir.join(&filename);

    // Write to file
    fs::write(&output_path, struct_code).context(format!(
        "Failed to write structure file: {}",
        output_path.display()
    ))?;

    tracing::debug!("Generated: {}", output_path.display());

    Ok(())
}

/// Generate the module index file listing all generated types
pub fn generate_module_index(config: &RustGeneratorConfig, structure_count: usize) -> Result<()> {
    fs::create_dir_all(&config.output_dir).context("Failed to create output directory")?;

    let mut index_code = String::from(
        "//! Generated FHIR structures\n//!\n//! This module contains auto-generated Rust code for FHIR resources and types.\n\n",
    );

    // Add a placeholder for now - would be populated during actual generation
    index_code.push_str(&format!(
        "// Generated {} structure definitions\n",
        structure_count
    ));
    index_code.push_str("\npub mod structures {\n");
    index_code.push_str("    // Structure modules will be declared here\n");
    index_code.push_str("}\n");

    let output_path = config.output_dir.join("mod.rs");
    fs::write(&output_path, index_code).context(format!(
        "Failed to write module index: {}",
        output_path.display()
    ))?;

    tracing::debug!("Generated module index: {}", output_path.display());

    Ok(())
}

/// Generate a basic Rust struct definition from a FHIR structure
#[allow(dead_code)]
fn generate_struct_definition(definition: &ResourceDefinition) -> String {
    let mut code = String::new();

    let name = definition.name.as_deref().unwrap_or("Unknown");
    let url = &definition.url;

    // File header
    code.push_str(&format!("//! FHIR {} Resource\n", name));
    code.push_str("//!\n");
    code.push_str(&format!("//! Generated from: {}\n", url));
    code.push_str("//! This is a skeleton implementation.\n\n");

    // Derive attributes
    code.push_str("#[derive(Debug, Clone)]\n");

    // Struct definition
    code.push_str(&format!("pub struct {} {{\n", name));
    code.push_str("    /// Resource type (always \"");
    code.push_str(name);
    code.push_str("\")\n");
    code.push_str("    pub resource_type: String,\n");
    code.push_str("}\n\n");

    // Basic impl block
    code.push_str(&format!("impl {} {{\n", name));
    code.push_str("    /// Create a new instance\n");
    code.push_str("    pub fn new() -> Self {\n");
    code.push_str("        Self {\n");
    code.push_str("            resource_type: \"");
    code.push_str(name);
    code.push_str("\".to_string(),\n");
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    code.push_str(&format!("impl Default for {} {{\n", name));
    code.push_str("    fn default() -> Self {\n");
    code.push_str("        Self::new()\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_module_index() {
        let temp_dir = TempDir::new().unwrap();
        let config = RustGeneratorConfig::new(temp_dir.path().to_path_buf());

        let result = generate_module_index(&config, 1);
        assert!(result.is_ok());

        // Check that mod.rs was created
        let mod_file = temp_dir.path().join("mod.rs");
        assert!(mod_file.exists());

        // Check file contents
        let contents = fs::read_to_string(&mod_file).unwrap();
        assert!(contents.contains("Generated 1 structure definitions"));
        assert!(contents.contains("pub mod structures"));
    }
}
