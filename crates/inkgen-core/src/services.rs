use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;

use crate::cache::{InstallMode, PackageCache};
use crate::error::{CoreError, CoreResult};
use crate::ir::ResourceDefinition;
use crate::package::{PackageDescriptor, PackageRequest, StructureKind, StructureSummary};
use crate::profile::ProfilePipeline;
use serde_json::Value;

/// Service responsible for ensuring packages are installed and discoverable.
pub struct PackageResolver {
    cache: Arc<PackageCache>,
}

impl PackageResolver {
    pub fn new(cache: Arc<PackageCache>) -> Self {
        Self { cache }
    }

    pub async fn ensure_packages(
        &self,
        requests: &[PackageRequest],
        mode: InstallMode,
    ) -> CoreResult<Vec<PackageDescriptor>> {
        self.cache.ensure_packages(requests, mode).await
    }

    pub async fn descriptors(&self) -> CoreResult<Vec<PackageDescriptor>> {
        self.cache.descriptors().await
    }
}

/// Tree-shaking configuration applied when listing structures.
#[derive(Debug, Clone, Default)]
pub struct StructureProviderConfig {
    pub allowed_resource_types: Option<HashSet<String>>,
    pub include_profiles: bool,
}

/// Filter derived from the provider configuration and optional overrides.
#[derive(Debug, Clone)]
pub struct StructureFilter<'a> {
    allowed_resource_types: Option<&'a HashSet<String>>,
    include_profiles: bool,
}

impl<'a> StructureFilter<'a> {
    pub fn from_config(config: &'a StructureProviderConfig) -> Self {
        Self {
            allowed_resource_types: config.allowed_resource_types.as_ref(),
            include_profiles: config.include_profiles,
        }
    }

    pub fn with_allowed_types(mut self, allowed: Option<&'a HashSet<String>>) -> Self {
        self.allowed_resource_types = allowed;
        self
    }

    pub fn include_profiles(mut self, include: bool) -> Self {
        self.include_profiles = include;
        self
    }

    pub fn matches(&self, summary: &StructureSummary) -> bool {
        if !self.include_profiles && summary.kind == StructureKind::Profile {
            return false;
        }

        if let Some(allowed) = self.allowed_resource_types
            && matches!(
                summary.kind,
                StructureKind::BaseResource | StructureKind::Profile
            )
        {
            let type_code = summary.type_code.as_deref().unwrap_or_default();
            return allowed.contains(type_code);
        }

        true
    }
}

#[async_trait]
pub trait StructureDefinitionProvider: Send + Sync {
    async fn list_structures(
        &self,
        filter: &StructureFilter<'_>,
    ) -> CoreResult<Vec<StructureSummary>>;

    async fn load_structure(&self, canonical: &str) -> CoreResult<ResourceDefinition>;
}

/// Provider focused on base StructureDefinitions shipped in canonical packages.
pub struct BaseStructureService {
    cache: Arc<PackageCache>,
    config: StructureProviderConfig,
}

impl BaseStructureService {
    pub fn new(cache: Arc<PackageCache>, config: StructureProviderConfig) -> Self {
        Self { cache, config }
    }

    pub fn from_project_config(
        cache: Arc<PackageCache>,
        project: &crate::config::InkgenConfig,
    ) -> Self {
        Self::new(cache, project.structure_config())
    }

    pub fn config(&self) -> &StructureProviderConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut StructureProviderConfig {
        &mut self.config
    }
}

#[async_trait]
impl StructureDefinitionProvider for BaseStructureService {
    async fn list_structures(
        &self,
        filter: &StructureFilter<'_>,
    ) -> CoreResult<Vec<StructureSummary>> {
        let mut summaries = Vec::new();
        let mut seen = HashSet::new();

        for descriptor in self.cache.descriptors().await? {
            for structure in descriptor.structures() {
                if filter.matches(structure) && seen.insert(structure.canonical_url.clone()) {
                    summaries.push(structure.clone());
                }
            }
        }

        summaries.sort_by(|a, b| {
            let key_a = (
                a.type_code.clone().unwrap_or_default(),
                a.canonical_url.clone(),
            );
            let key_b = (
                b.type_code.clone().unwrap_or_default(),
                b.canonical_url.clone(),
            );
            key_a.cmp(&key_b)
        });

        Ok(summaries)
    }

    async fn load_structure(&self, canonical: &str) -> CoreResult<ResourceDefinition> {
        let manager = self.cache.manager().await?;
        let resolved = manager.resolve(canonical).await.map_err(CoreError::from)?;

        // Profiles are not yet supported in the base service; guard callers early.
        if resolved.resource.resource_type != "StructureDefinition" {
            return Err(CoreError::Unsupported {
                detail: format!("resource {} is not a StructureDefinition", canonical),
            });
        }

        if !self.config.include_profiles
            && resolved
                .resource
                .content
                .get("derivation")
                .and_then(Value::as_str)
                .is_some_and(|derivation| derivation == "constraint")
        {
            return Err(CoreError::Unsupported {
                detail: format!("profile resolution is not yet supported for {}", canonical),
            });
        }

        ProfilePipeline::resolve(&resolved.resource.content, None)
    }
}
