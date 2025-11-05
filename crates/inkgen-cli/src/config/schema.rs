use serde::{Deserialize, Serialize};


/// Main configuration structure for Inkgen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InkgenConfig {
    /// List of FHIR packages to include
    pub packages: Vec<PackageSpec>,
    
    /// Tree-shaking configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_shaking: Option<TreeShakingConfig>,
    
    /// Language-specific configurations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<LanguageConfigs>,
}

/// Specification for a FHIR package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSpec {
    /// Package name (supports shortened names without hl7.fhir prefix)
    pub name: String,
    
    /// Package version (optional, defaults to latest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    
    /// Package-specific inclusion rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    
    /// Package-specific exclusion rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

/// Tree-shaking configuration to control what gets included in generation
/// By default, all resources from packages are generated.
/// Use tree-shaking to limit generation to specific resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeShakingConfig {
    /// Explicit resource allowlist to limit generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_resources: Option<Vec<String>>,
    
    /// Explicit profile allowlist to limit generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_profiles: Option<Vec<String>>,
}

/// Language-specific configuration sections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfigs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typescript: Option<TypeScriptConfig>,
}

/// TypeScript-specific generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeScriptConfig {
    /// Generation mode: "interface" | "class" | "class_with_builder"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    
    /// Enable structural guards
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structural_guards: Option<bool>,
    
    /// Naming convention override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naming_convention: Option<String>,
    
    /// Output directory structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_structure: Option<String>,
}

impl Default for InkgenConfig {
    fn default() -> Self {
        Self {
            packages: vec![
                PackageSpec {
                    name: "hl7.fhir.r4.core".to_string(),
                    version: Some("4.0.1".to_string()),
                    include: None,
                    exclude: None,
                },
            ],
            tree_shaking: None,
            languages: Some(LanguageConfigs {
                typescript: Some(TypeScriptConfig {
                    mode: Some("class_with_builder".to_string()),
                    structural_guards: Some(true),
                    naming_convention: Some("PascalCase".to_string()),
                    output_structure: Some("flat".to_string()),
                }),
            }),
        }
    }
}

impl Default for TypeScriptConfig {
    fn default() -> Self {
        Self {
            mode: Some("class_with_builder".to_string()),
            structural_guards: Some(true),
            naming_convention: Some("PascalCase".to_string()),
            output_structure: Some("flat".to_string()),
        }
    }
}

impl PackageSpec {
    /// Normalize package name by expanding shortened forms
    pub fn normalize_name(&self) -> String {
        if self.name.starts_with("hl7.fhir.") {
            self.name.clone()
        } else {
            // Handle common shortened forms
            match self.name.as_str() {
                "r4.core" => "hl7.fhir.r4.core".to_string(),
                "r5.core" => "hl7.fhir.r5.core".to_string(),
                "r6.core" => "hl7.fhir.r6.core".to_string(),
                name if name.starts_with("us.") => format!("hl7.fhir.{}", name),
                name if name.starts_with("r4.") => format!("hl7.fhir.{}", name),
                name if name.starts_with("r5.") => format!("hl7.fhir.{}", name),
                name if name.starts_with("r6.") => format!("hl7.fhir.{}", name),
                _ => self.name.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_name_normalization() {
        let test_cases = vec![
            ("hl7.fhir.r4.core", "hl7.fhir.r4.core"),
            ("r4.core", "hl7.fhir.r4.core"),
            ("r5.core", "hl7.fhir.r5.core"),
            ("us.core", "hl7.fhir.us.core"),
            ("r4.examples", "hl7.fhir.r4.examples"),
            ("custom.package", "custom.package"),
        ];

        for (input, expected) in test_cases {
            let package = PackageSpec {
                name: input.to_string(),
                version: None,
                include: None,
                exclude: None,
            };
            assert_eq!(package.normalize_name(), expected);
        }
    }

    #[test]
    fn test_default_config_serialization() {
        let config = InkgenConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        
        // Verify it contains expected sections
        assert!(toml_str.contains("[[packages]]"));
        assert!(toml_str.contains("hl7.fhir.r4.core"));
        assert!(toml_str.contains("[languages.typescript]"));
        // tree_shaking is optional and not included in default config
    }

    #[test]
    fn test_config_deserialization() {
        let toml_content = r#"
[[packages]]
name = "r4.core"
version = "4.0.1"

[tree_shaking]
allowed_resources = ["Patient", "Observation"]

[languages.typescript]
mode = "interface"
structural_guards = false
"#;

        let config: InkgenConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].name, "r4.core");
        assert_eq!(config.packages[0].version, Some("4.0.1".to_string()));
        
        let ts_config = config.languages.unwrap().typescript.unwrap();
        assert_eq!(ts_config.mode, Some("interface".to_string()));
        assert_eq!(ts_config.structural_guards, Some(false));
    }
}