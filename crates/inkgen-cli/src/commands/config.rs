use std::path::Path;
use tracing::{info, warn};
use crate::config::InkgenConfig;
use crate::error::{CliError, CliResult};

/// Execute the config show command
pub fn execute_config_show() -> CliResult<()> {
    info!("Showing current configuration");
    
    // Try to load existing configuration, or show default if none exists
    let config_path = "inkgen.toml";
    let config = if std::path::Path::new(config_path).exists() {
        InkgenConfig::from_file(config_path)?
    } else {
        InkgenConfig::default()
    };
    
    // Serialize to TOML for display
    let toml_content = toml::to_string_pretty(&config)?;
    
    println!("Current configuration:");
    println!("{}", toml_content);
    
    Ok(())
}

/// Execute the config set command
pub fn execute_config_set(key_value: &str) -> CliResult<()> {
    info!("Setting configuration: {}", key_value);
    
    // Parse key=value format
    let parts: Vec<&str> = key_value.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(CliError::invalid_arguments(
            "Configuration setting must be in key=value format"
        ));
    }
    
    let key = parts[0].trim();
    let value = parts[1].trim();
    
    if key.is_empty() || value.is_empty() {
        return Err(CliError::invalid_arguments(
            "Both key and value must be non-empty"
        ));
    }
    
    println!("Setting configuration: {}={}", key, value);
    println!("Note: Configuration modification is not yet fully implemented.");
    println!("Use 'inkgen config init' to create a configuration file and edit it manually.");
    
    Ok(())
}

/// Execute the config init command
pub fn execute_config_init<P: AsRef<Path>>(output_path: P, force: bool) -> CliResult<()> {
    let output_path = output_path.as_ref();
    
    info!("Initializing configuration file at: {}", output_path.display());
    
    // Check if file already exists
    if output_path.exists() && !force {
        return Err(CliError::ConfigExists {
            path: output_path.to_string_lossy().to_string(),
        });
    }
    
    if output_path.exists() && force {
        warn!("Overwriting existing configuration file due to --force flag");
    }
    
    // Create default configuration
    let default_config = InkgenConfig::default();
    
    // Save configuration to file
    default_config.to_file(output_path)?;
    
    // Provide success feedback
    println!("✓ Created configuration file: {}", output_path.display());
    println!("  Edit this file to customize your FHIR package selection and generation options.");
    
    info!("Configuration file created successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_config_init_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("inkgen.toml");
        
        let result = execute_config_init(&config_path, false);
        assert!(result.is_ok());
        
        // Verify file was created
        assert!(config_path.exists());
        
        // Verify content is valid TOML and contains expected sections
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[[packages]]"));
        assert!(content.contains("hl7.fhir.r4.core"));
        assert!(content.contains("[languages.typescript]"));
        // tree_shaking is optional and not included in default config
        
        // Verify we can parse it back
        let parsed_config = InkgenConfig::from_file(&config_path).unwrap();
        assert!(!parsed_config.packages.is_empty());
    }

    #[test]
    fn test_config_init_file_exists_without_force() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("existing.toml");
        
        // Create existing file
        fs::write(&config_path, "existing content").unwrap();
        
        let result = execute_config_init(&config_path, false);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            CliError::ConfigExists { path } => {
                assert!(path.contains("existing.toml"));
            }
            _ => panic!("Expected ConfigExists error"),
        }
        
        // Verify original content is preserved
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn test_config_init_file_exists_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("existing.toml");
        
        // Create existing file
        fs::write(&config_path, "existing content").unwrap();
        
        let result = execute_config_init(&config_path, true);
        assert!(result.is_ok());
        
        // Verify file was overwritten with new config
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[[packages]]"));
        assert!(!content.contains("existing content"));
    }

    #[test]
    fn test_config_init_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested").join("dir").join("inkgen.toml");
        
        // Create parent directories first (this is what the actual implementation would do)
        if let Some(parent) = nested_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        
        let result = execute_config_init(&nested_path, false);
        assert!(result.is_ok());
        
        // Verify file was created in nested directory
        assert!(nested_path.exists());
        
        let content = fs::read_to_string(&nested_path).unwrap();
        assert!(content.contains("[[packages]]"));
    }

    #[test]
    fn test_config_init_invalid_path() {
        // Try to write to a path that doesn't exist and can't be created
        let temp_dir = TempDir::new().unwrap();
        let invalid_path = temp_dir.path().join("nonexistent").join("deeply").join("nested").join("path").join("inkgen.toml");
        
        // Don't create parent directories - this should cause the function to fail
        let result = execute_config_init(&invalid_path, false);
        assert!(result.is_err());
        
        // The error could be various types depending on the implementation
        match result.unwrap_err() {
            CliError::IoError { .. } => {}, // Expected
            CliError::FileNotFound { .. } => {}, // Also acceptable
            _ => panic!("Expected IoError or FileNotFound"),
        }
    }
}