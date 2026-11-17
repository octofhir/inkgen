use std::collections::HashSet;
use std::future::Future;
#[cfg(feature = "resolver")]
use std::sync::Arc;

#[cfg(feature = "resolver")]
use crate::cache::{InstallMode, PackageCache};
#[cfg(feature = "resolver")]
use crate::error::CoreError;
use crate::error::CoreResult;
use crate::ir::ResourceDefinition;
#[cfg(feature = "resolver")]
use crate::package::{PackageDescriptor, PackageRequest};
use crate::package::{StructureKind, StructureSummary};
#[cfg(feature = "resolver")]
use crate::profile::ProfilePipeline;
#[cfg(feature = "resolver")]
use serde_json::Value;

/// Service responsible for ensuring packages are installed and discoverable.
#[derive(Debug)]
#[cfg(feature = "resolver")]
pub struct PackageResolver {
    cache: Arc<PackageCache>,
}

#[cfg(feature = "resolver")]
impl PackageResolver {
    /// Create a resolver backed by the given package cache.
    pub fn new(cache: Arc<PackageCache>) -> Self {
        Self { cache }
    }

    /// Install or load the requested packages according to `mode`, returning their descriptors.
    pub async fn ensure_packages(
        &self,
        requests: &[PackageRequest],
        mode: InstallMode,
    ) -> CoreResult<Vec<PackageDescriptor>> {
        self.cache.ensure_packages(requests, mode).await
    }

    /// Describe every package currently available in the cache.
    pub async fn descriptors(&self) -> CoreResult<Vec<PackageDescriptor>> {
        self.cache.descriptors().await
    }
}

/// Tree-shaking configuration applied when listing structures.
#[derive(Debug, Clone, Default)]
pub struct StructureProviderConfig {
    /// Restrict base resources/profiles to these type codes; `None` allows all.
    pub allowed_resource_types: Option<HashSet<String>>,
    /// Whether profile StructureDefinitions are included alongside base resources.
    pub include_profiles: bool,
}

/// Filter derived from the provider configuration and optional overrides.
#[derive(Debug, Clone)]
pub struct StructureFilter<'a> {
    allowed_resource_types: Option<&'a HashSet<String>>,
    include_profiles: bool,
}

impl<'a> StructureFilter<'a> {
    /// Build a filter borrowing its settings from a provider config.
    pub fn from_config(config: &'a StructureProviderConfig) -> Self {
        Self {
            allowed_resource_types: config.allowed_resource_types.as_ref(),
            include_profiles: config.include_profiles,
        }
    }

    /// Override the allowed resource type set (`None` allows all).
    pub fn with_allowed_types(mut self, allowed: Option<&'a HashSet<String>>) -> Self {
        self.allowed_resource_types = allowed;
        self
    }

    /// Override whether profiles pass the filter.
    pub fn include_profiles(mut self, include: bool) -> Self {
        self.include_profiles = include;
        self
    }

    /// Check whether a structure passes the profile and resource-type restrictions.
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

/// Async source of StructureDefinitions: lists summaries and loads resolved definitions.
pub trait StructureDefinitionProvider: Send + Sync {
    /// List structure summaries matching the filter, in deterministic order.
    fn list_structures(
        &self,
        filter: &StructureFilter<'_>,
    ) -> impl Future<Output = CoreResult<Vec<StructureSummary>>> + Send;

    /// Resolve a StructureDefinition by canonical URL into a lowered IR definition.
    fn load_structure(
        &self,
        canonical: &str,
    ) -> impl Future<Output = CoreResult<ResourceDefinition>> + Send;
}

/// Provider focused on base StructureDefinitions shipped in canonical packages.
#[derive(Debug)]
#[cfg(feature = "resolver")]
pub struct BaseStructureService {
    cache: Arc<PackageCache>,
    config: StructureProviderConfig,
}

#[cfg(feature = "resolver")]
impl BaseStructureService {
    /// Create a service over the given cache and provider configuration.
    pub fn new(cache: Arc<PackageCache>, config: StructureProviderConfig) -> Self {
        Self { cache, config }
    }

    /// Create a service using the structure settings derived from a project config.
    pub fn from_project_config(
        cache: Arc<PackageCache>,
        project: &crate::config::InkgenConfig,
    ) -> Self {
        Self::new(cache, project.structure_config())
    }

    /// Borrow the current provider configuration.
    pub fn config(&self) -> &StructureProviderConfig {
        &self.config
    }

    /// Mutably borrow the provider configuration for in-place adjustments.
    pub fn config_mut(&mut self) -> &mut StructureProviderConfig {
        &mut self.config
    }
}

#[cfg(feature = "resolver")]
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
