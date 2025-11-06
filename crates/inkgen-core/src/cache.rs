use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use octofhir_canonical_manager::CanonicalManager;
use octofhir_canonical_manager::config::{
    FcmConfig, LocalPackageSpec, OptimizationConfig, RegistryConfig, StorageConfig,
};
use octofhir_canonical_manager::search::ResourceMatch;
use serde_json::Value;
use tokio::sync::{Mutex, RwLock};
use tracing::Instrument;

use crate::error::{CoreError, CoreResult};
use crate::package::{
    ArtifactDescriptor, PackageDescriptor, PackageId, PackageInventory, PackageRequest,
    PackageSource, StructureKind, StructureSummary,
};

/// Installation mode for requested packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// Attempt network download; fall back to cached copy when offline.
    OnlinePreferred,
    /// Do not hit the network; rely entirely on cache/local overrides.
    OfflineOnly,
}

/// Configuration options for the package cache.
#[derive(Debug, Clone)]
pub struct PackageCacheConfig {
    pub packages_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub registry_url: String,
    pub connection_pool_size: usize,
}

impl PackageCacheConfig {
    pub fn new(packages_dir: impl AsRef<Path>) -> CoreResult<Self> {
        let packages_dir = packages_dir.as_ref().to_path_buf();
        let cache_dir = packages_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache");

        Ok(Self {
            packages_dir,
            cache_dir,
            registry_url: default_registry_url(),
            connection_pool_size: 4,
        })
    }

    pub fn with_registry_url(mut self, url: impl Into<String>) -> Self {
        self.registry_url = url.into();
        self
    }

    pub fn with_connection_pool_size(mut self, size: usize) -> Self {
        self.connection_pool_size = size;
        self
    }

    pub fn storage_config(&self) -> StorageConfig {
        StorageConfig {
            cache_dir: self.cache_dir.clone(),
            packages_dir: self.packages_dir.clone(),
            max_cache_size: "2GB".to_string(),
            connection_pool_size: self.connection_pool_size,
        }
    }

    pub fn registry_config(&self) -> RegistryConfig {
        RegistryConfig {
            url: self.registry_url.clone(),
            timeout: 30,
            retry_attempts: 3,
        }
    }
}

impl Default for PackageCacheConfig {
    fn default() -> Self {
        let packages_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("target")
            .join("inkgen")
            .join("packages");
        Self::new(packages_dir).expect("default package cache config")
    }
}

fn default_registry_url() -> String {
    "https://packages.fhir.org/".to_string()
}

/// Wrapper around the canonical manager providing InkGen-specific semantics.
pub struct PackageCache {
    config: PackageCacheConfig,
    manager: Arc<Mutex<Option<Arc<CanonicalManager>>>>,
    descriptors: Arc<RwLock<HashMap<PackageId, PackageDescriptor>>>,
    local_packages: Vec<PackageRequest>,
}

impl PackageCache {
    pub fn builder() -> PackageCacheBuilder {
        PackageCacheBuilder::default()
    }

    pub fn config(&self) -> &PackageCacheConfig {
        &self.config
    }

    pub async fn manager(&self) -> CoreResult<Arc<CanonicalManager>> {
        if let Some(existing) = self.current_manager().await {
            return Ok(existing);
        }

        let config = self.build_fcm_config()?;
        let span = tracing::info_span!("canonical_manager.init");
        let manager = CanonicalManager::new(config).instrument(span).await?;

        let manager = Arc::new(manager);
        let mut guard = self.manager.lock().await;
        if guard.is_none() {
            *guard = Some(manager.clone());
        } else if let Some(existing) = guard.clone() {
            return Ok(existing);
        }
        Ok(manager)
    }

    async fn current_manager(&self) -> Option<Arc<CanonicalManager>> {
        let guard = self.manager.lock().await;
        guard.clone()
    }

    fn build_fcm_config(&self) -> CoreResult<FcmConfig> {
        let mut config = FcmConfig {
            registry: self.config.registry_config(),
            packages: Vec::new(),
            storage: self.config.storage_config(),
            optimization: OptimizationConfig::default(),
            local_packages: Vec::new(),
            resource_directories: Vec::new(),
        };

        for request in &self.local_packages {
            if let PackageSource::Local { path, priority } = &request.source {
                config.local_packages.push(LocalPackageSpec {
                    name: request.id.name.clone(),
                    version: request.id.version.clone(),
                    path: path.clone(),
                    priority: *priority,
                });
            }
        }

        Ok(config)
    }

    pub fn with_local_packages(mut self, packages: Vec<PackageRequest>) -> Self {
        self.local_packages = packages;
        self
    }

    pub async fn ensure_packages(
        &self,
        requests: &[PackageRequest],
        mode: InstallMode,
    ) -> CoreResult<Vec<PackageDescriptor>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let manager = self.manager().await?;
        let mut installed_specs: HashSet<String> =
            manager.list_packages().await?.into_iter().collect();

        let mut descriptors = Vec::new();

        for request in requests {
            let package_spec = request.id.as_str();
            let is_installed = installed_specs.contains(&package_spec);

            if !is_installed {
                match (&request.source, mode) {
                    (PackageSource::Registry, InstallMode::OfflineOnly) => {
                        return Err(CoreError::OfflineUnavailable {
                            package: request.id.clone(),
                            cache_dir: self.config.packages_dir.clone(),
                        });
                    }
                    (PackageSource::Registry, InstallMode::OnlinePreferred) => {
                        if let Err(err) = manager
                            .install_package(&request.id.name, &request.id.version)
                            .await
                        {
                            // If installation fails, check whether the package became available
                            // concurrently (e.g., already cached) before propagating the error.
                            let still_missing = {
                                installed_specs =
                                    manager.list_packages().await?.into_iter().collect();
                                !installed_specs.contains(&package_spec)
                            };
                            if still_missing {
                                return Err(CoreError::from(err));
                            }
                        } else {
                            installed_specs.insert(package_spec.clone());
                        }
                    }
                    (PackageSource::Local { path, .. }, _) => {
                        manager
                            .load_from_directory(path, Some(&package_spec))
                            .await?;
                        installed_specs.insert(package_spec.clone());
                    }
                }
            }

            let descriptor = self.describe_package(&manager, request).await?;
            self.descriptors
                .write()
                .await
                .insert(request.id.clone(), descriptor.clone());
            descriptors.push(descriptor);
        }

        Ok(descriptors)
    }

    pub async fn list_installed(&self) -> CoreResult<Vec<String>> {
        let manager = self.manager().await?;
        let packages = manager.list_packages().await?;
        Ok(packages)
    }

    pub async fn descriptors(&self) -> CoreResult<Vec<PackageDescriptor>> {
        let manager = self.manager().await?;
        let installed_specs = manager.list_packages().await?;
        {
            let mut guard = self.descriptors.write().await;
            for spec in installed_specs {
                let id = parse_package_spec(&spec)?;
                if !guard.contains_key(&id) {
                    let inferred = self.infer_request(&id);
                    let descriptor = self.describe_package(&manager, &inferred).await?;
                    guard.insert(id.clone(), descriptor);
                }
            }
            Ok(guard.values().cloned().collect())
        }
    }

    fn infer_request(&self, id: &PackageId) -> PackageRequest {
        if let Some(local) = self
            .local_packages
            .iter()
            .find(|candidate| candidate.id == *id)
        {
            return local.clone();
        }

        PackageRequest {
            id: id.clone(),
            priority: 1,
            source: PackageSource::Registry,
        }
    }

    async fn describe_package(
        &self,
        manager: &CanonicalManager,
        request: &PackageRequest,
    ) -> CoreResult<PackageDescriptor> {
        let package_spec = request.id.as_str();
        let resources = self
            .collect_package_resources(manager, &package_spec)
            .await?;

        let mut inventory = PackageInventory::default();
        let mut structure_seen: HashSet<String> = HashSet::new();

        for resource_match in &resources {
            if resource_match.resource.resource_type == "StructureDefinition" {
                if let Some(summary) =
                    Self::summarize_structure(&request.id, &resource_match.resource)
                    && structure_seen.insert(summary.canonical_url.clone())
                {
                    inventory.push_structure(summary);
                }
                continue;
            }

            inventory.push_artifact(Self::summarize_artifact(
                &request.id,
                &resource_match.resource,
                &resource_match.index.canonical_url,
            ));
        }

        inventory.sort();

        Ok(request.descriptor(resources.len(), inventory))
    }

    async fn collect_package_resources(
        &self,
        manager: &CanonicalManager,
        package_spec: &str,
    ) -> CoreResult<Vec<ResourceMatch>> {
        let mut collected = Vec::new();
        let mut offset = 0usize;

        loop {
            let builder = manager.search().await;
            let result = builder
                .package(package_spec)
                .offset(offset)
                .limit(1000)
                .execute()
                .await?;

            if result.resources.is_empty() {
                break;
            }

            offset += result.resources.len();
            collected.extend(result.resources);

            if offset >= result.total_count {
                break;
            }
        }

        Ok(collected)
    }

    fn summarize_structure(
        package: &PackageId,
        resource: &octofhir_canonical_manager::package::FhirResource,
    ) -> Option<StructureSummary> {
        let canonical = resource
            .url
            .clone()
            .or_else(|| {
                resource
                    .content
                    .get("url")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
            })
            .or_else(|| {
                if resource.file_path.exists() {
                    Some(resource.file_path.to_string_lossy().into_owned())
                } else {
                    None
                }
            })?;

        let kind_raw = resource
            .content
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let derivation = resource.content.get("derivation").and_then(Value::as_str);
        let structure_kind = match (kind_raw, derivation) {
            (_, Some("constraint")) => StructureKind::Profile,
            ("resource", _) => StructureKind::BaseResource,
            ("complex-type", _) => StructureKind::ComplexType,
            ("primitive-type", _) => StructureKind::PrimitiveType,
            ("logical", _) => StructureKind::Logical,
            _ => StructureKind::Profile,
        };

        let type_code = resource
            .content
            .get("type")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        let name = resource
            .content
            .get("name")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let title = resource
            .content
            .get("title")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let version = resource.version.clone().or_else(|| {
            resource
                .content
                .get("version")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        });
        let status = resource
            .content
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        Some(StructureSummary {
            canonical_url: canonical,
            name,
            type_code,
            title,
            version,
            status,
            package: package.clone(),
            kind: structure_kind,
        })
    }

    fn summarize_artifact(
        package: &PackageId,
        resource: &octofhir_canonical_manager::package::FhirResource,
        canonical: &str,
    ) -> ArtifactDescriptor {
        let name = resource
            .content
            .get("name")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let version = resource.version.clone().or_else(|| {
            resource
                .content
                .get("version")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        });
        let status = resource
            .content
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        ArtifactDescriptor {
            canonical_url: Some(canonical.to_string()),
            name,
            resource_type: resource.resource_type.clone(),
            version,
            status,
            package: package.clone(),
        }
    }
}

fn parse_package_spec(spec: &str) -> CoreResult<PackageId> {
    PackageId::from_str(spec).map_err(|detail| CoreError::InvalidPackage { detail })
}

#[derive(Default)]
pub struct PackageCacheBuilder {
    config: PackageCacheConfig,
    local_packages: Vec<PackageRequest>,
}

impl PackageCacheBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config(mut self, config: PackageCacheConfig) -> Self {
        self.config = config;
        self
    }

    pub fn add_local_package(mut self, request: PackageRequest) -> Self {
        self.local_packages.push(request);
        self
    }

    pub async fn build(self) -> CoreResult<PackageCache> {
        let cache = PackageCache {
            config: self.config,
            manager: Arc::new(Mutex::new(None)),
            descriptors: Arc::new(RwLock::new(HashMap::new())),
            local_packages: self.local_packages,
        };

        Ok(cache)
    }
}
