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
}
