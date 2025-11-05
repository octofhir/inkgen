use crate::config::schema::InkgenConfig;
use crate::error::{CliError, CliResult};
use std::path::Path;
use tracing::{debug, info};

impl InkgenConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> CliResult<Self> {
        let path = path.as_ref();
        debug!("Loading configuration from: {}", path.display());
        
        let content = std::fs::read_to_string(path)
            .map_err(|_e| CliError::file_not_found_with_context(
                path.to_string_lossy().to_string(),
                "configuration file"
            ))?;
        
        Self::from_str(&content)
    }
    
    /// Parse configuration from a TOML string
    pub fn from_str(content: &str) -> CliResult<Self> {
        debug!("Parsing configuration from string");
        
        let config: InkgenConfig = toml::from_str(content)
            .map_err(|e| CliError::invalid_config_with_source(
                format!("Failed to parse TOML: {}", e),
                e
            ))?;
        
        config.validate()?;
        info!("Configuration loaded successfully with {} packages", config.packages.len());
        
        Ok(config)
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> CliResult<()> {
        debug!("Validating configuration");
        
        if self.packages.is_empty() {
            return Err(CliError::invalid_config(
                "Configuration must specify at least one package"
            ));
        }
        
        // Validate package specifications
        for (i, package) in self.packages.iter().enumerate() {
            if package.name.trim().is_empty() {
                return Err(CliError::invalid_config(
                    format!("Package {} has empty name", i + 1)
                ));
            }
            
            // Validate version format if specified
            if let Some(version) = &package.version {
                if version.trim().is_empty() {
                    return Err(CliError::invalid_config(
                        format!("Package '{}' has empty version", package.name)
                    ));
                }
            }
        }
        
        // Tree-shaking configuration is optional and doesn't require validation
        // By default, all resources are generated unless explicitly limited
        
        debug!("Configuration validation passed");
        Ok(())
    }
    
    /// Write configuration to a TOML file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> CliResult<()> {
        let path = path.as_ref();
        debug!("Writing configuration to: {}", path.display());
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| CliError::invalid_config_with_source(
                format!("Failed to serialize configuration: {}", e),
                e
            ))?;
        
        std::fs::write(path, content)
            .map_err(|_e| CliError::file_not_found_with_context(
                path.to_string_lossy().to_string(),
                "writing configuration file"
            ))?;
        
        info!("Configuration written to: {}", path.display());
        Ok(())
    }
}

/// Normalize package name by expanding shortened forms
pub fn normalize_package_name(name: &str) -> String {
    let name = name.trim();
    
    // If already fully qualified, return as-is
    if name.starts_with("hl7.fhir.") {
        return name.to_string();
    }
    
    // Handle common shortened forms
    match name {
        "r4.core" => "hl7.fhir.r4.core".to_string(),
        "r5.core" => "hl7.fhir.r5.core".to_string(),
        "r6.core" => "hl7.fhir.r6.core".to_string(),
        "us.core" => "hl7.fhir.us.core".to_string(),
        name if name.starts_with("us.") => format!("hl7.fhir.{}", name),
        name if name.starts_with("r4.") => format!("hl7.fhir.{}", name),
        name if name.starts_with("r5.") => format!("hl7.fhir.{}", name),
        name if name.starts_with("r6.") => format!("hl7.fhir.{}", name),
        _ => {
            // For other cases, assume it needs the hl7.fhir prefix
            format!("hl7.fhir.{}", name)
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{PackageSpec, TreeShakingConfig, LanguageConfigs, TypeScriptConfig};
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_normalize_package_name_fully_qualified() {
        assert_eq!(
            normalize_package_name("hl7.fhir.r4.core"),
            "hl7.fhir.r4.core"
        );
        assert_eq!(
            normalize_package_name("hl7.fhir.us.core"),
            "hl7.fhir.us.core"
        );
    }
    
    #[test]
    fn test_normalize_package_name_shortened_forms() {
        assert_eq!(normalize_package_name("r4.core"), "hl7.fhir.r4.core");
        assert_eq!(normalize_package_name("r5.core"), "hl7.fhir.r5.core");
        assert_eq!(normalize_package_name("r6.core"), "hl7.fhir.r6.core");
        assert_eq!(normalize_package_name("us.core"), "hl7.fhir.us.core");
    }
    
    #[test]
    fn test_normalize_package_name_prefixed_forms() {
        assert_eq!(normalize_package_name("us.example"), "hl7.fhir.us.example");
        assert_eq!(normalize_package_name("r4.example"), "hl7.fhir.r4.example");
        assert_eq!(normalize_package_name("r5.example"), "hl7.fhir.r5.example");
    }
    
    #[test]
    fn test_normalize_package_name_generic() {
        assert_eq!(normalize_package_name("example"), "hl7.fhir.example");
        assert_eq!(normalize_package_name("custom.package"), "hl7.fhir.custom.package");
    }
    
    #[test]
    fn test_normalize_package_name_whitespace() {
        assert_eq!(normalize_package_name("  r4.core  "), "hl7.fhir.r4.core");
        assert_eq!(normalize_package_name("\tus.core\n"), "hl7.fhir.us.core");
    }
    
    #[test]
    fn test_config_validation_empty_packages() {
        let config = InkgenConfig {
            packages: vec![],
            tree_shaking: None,
            languages: None,
        };
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one package"));
    }
    
    #[test]
    fn test_config_validation_empty_package_name() {
        let config = InkgenConfig {
            packages: vec![PackageSpec {
                name: "".to_string(),
                version: None,
                include: None,
                exclude: None,
            }],
            tree_shaking: None,
            languages: None,
        };
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty name"));
    }
    
    #[test]
    fn test_config_validation_empty_version() {
        let config = InkgenConfig {
            packages: vec![PackageSpec {
                name: "hl7.fhir.r4.core".to_string(),
                version: Some("".to_string()),
                include: None,
                exclude: None,
            }],
            tree_shaking: None,
            languages: None,
        };
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty version"));
    }
    
    #[test]
    fn test_config_validation_with_tree_shaking() {
        let config = InkgenConfig {
            packages: vec![PackageSpec {
                name: "hl7.fhir.r4.core".to_string(),
                version: None,
                include: None,
                exclude: None,
            }],
            tree_shaking: Some(TreeShakingConfig {
                allowed_resources: Some(vec!["Patient".to_string(), "Observation".to_string()]),
                allowed_profiles: Some(vec!["us-core-patient".to_string()]),
            }),
            languages: None,
        };
        
        let result = config.validate();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_config_from_str_valid() {
        let toml_content = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[tree_shaking]
allowed_resources = ["Patient", "Observation"]

[languages.typescript]
mode = "class_with_builder"
"#;
        
        let result = InkgenConfig::from_str(toml_content);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].name, "hl7.fhir.r4.core");
        assert_eq!(config.packages[0].version, Some("4.0.1".to_string()));
    }
    
    #[test]
    fn test_config_from_str_invalid_toml() {
        let invalid_toml = r#"
[[packages]
name = "invalid toml
"#;
        
        let result = InkgenConfig::from_str(invalid_toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse TOML"));
    }
    
    #[test]
    fn test_config_file_roundtrip() {
        let config = InkgenConfig {
            packages: vec![PackageSpec {
                name: "hl7.fhir.r4.core".to_string(),
                version: Some("4.0.1".to_string()),
                include: None,
                exclude: None,
            }],
            tree_shaking: Some(TreeShakingConfig {
                allowed_resources: Some(vec!["Patient".to_string(), "Observation".to_string()]),
                allowed_profiles: None,
            }),
            languages: Some(LanguageConfigs {
                typescript: Some(TypeScriptConfig {
                    mode: Some("class_with_builder".to_string()),
                    structural_guards: Some(true),
                    naming_convention: Some("PascalCase".to_string()),
                    output_structure: Some("flat".to_string()),
                }),
            }),
        };
        
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();
        
        // Write config to file
        config.to_file(temp_path).unwrap();
        
        // Read config back from file
        let loaded_config = InkgenConfig::from_file(temp_path).unwrap();
        
        // Verify the loaded config matches the original
        assert_eq!(loaded_config.packages.len(), 1);
        assert_eq!(loaded_config.packages[0].name, "hl7.fhir.r4.core");
        assert_eq!(loaded_config.packages[0].version, Some("4.0.1".to_string()));
        
        let tree_shaking = loaded_config.tree_shaking.unwrap();
        assert_eq!(tree_shaking.allowed_resources, Some(vec!["Patient".to_string(), "Observation".to_string()]));
        
        let typescript_config = loaded_config.languages.unwrap().typescript.unwrap();
        assert_eq!(typescript_config.mode, Some("class_with_builder".to_string()));
        assert_eq!(typescript_config.structural_guards, Some(true));
    }
    

}