use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::{CoreError, CoreResult};
use crate::package::PackageRequest;
use crate::services::StructureProviderConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct InkgenConfig {
    #[serde(default)]
    pub packages: Vec<PackageEntry>,

    #[serde(default)]
    pub tree_shaking: TreeShakingSection,

    #[serde(default)]
    pub languages: LanguagesSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageEntry {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TreeShakingSection {
    #[serde(default)]
    pub allowed_resources: Vec<String>,

    #[serde(default)]
    pub allowed_profiles: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LanguagesSection {
    #[serde(default)]
    pub typescript: Option<TypescriptLanguageConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypescriptLanguageConfig {
    #[serde(default = "default_mode")]
    pub mode: String,

    #[serde(default)]
    pub structural_guards: bool,

    #[serde(default = "default_naming")]
    pub naming_convention: String,

    #[serde(default = "default_output_structure")]
    pub output_structure: String,

    #[serde(default)]
    pub output_dir: Option<String>,

    /// Generate profile types (default: true)
    #[serde(default = "default_true")]
    pub generate_profiles: bool,

    /// Generate value sets as const arrays (default: true)
    #[serde(default = "default_true")]
    pub generate_valuesets: bool,

    /// Maximum value set size for inline type unions (default: 50)
    /// If a value set has more codes than this limit, fallback to plain string type
    #[serde(default = "default_max_valueset_size")]
    pub max_valueset_size: usize,

    /// Generate value sets in separate files (default: false)
    #[serde(default)]
    pub valueset_separate_files: bool,
}

fn default_mode() -> String {
    "interface".to_string()
}

fn default_naming() -> String {
    "pascal".to_string()
}

fn default_output_structure() -> String {
    "flat".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_valueset_size() -> usize {
    50
}

impl InkgenConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> CoreResult<Self> {
        let contents = fs::read_to_string(path)?;
        Self::load_from_str(&contents)
    }

    pub fn load_from_str(contents: &str) -> CoreResult<Self> {
        toml::from_str(contents).map_err(|err| CoreError::Validation {
            detail: format!("invalid inkgen config: {err}"),
        })
    }

    pub fn package_requests(&self) -> Vec<PackageRequest> {
        self.packages
            .iter()
            .map(|entry| PackageRequest::registry(entry.name.clone(), entry.version.clone()))
            .collect()
    }

    pub fn structure_config(&self) -> StructureProviderConfig {
        let allowed = if self.tree_shaking.allowed_resources.is_empty() {
            None
        } else {
            Some(
                self.tree_shaking
                    .allowed_resources
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<HashSet<_>>(),
            )
        };

        StructureProviderConfig {
            allowed_resource_types: allowed,
            include_profiles: !self.tree_shaking.allowed_profiles.is_empty(),
        }
    }

    pub fn typescript_config(&self) -> Option<&TypescriptLanguageConfig> {
        self.languages.typescript.as_ref()
    }
}
