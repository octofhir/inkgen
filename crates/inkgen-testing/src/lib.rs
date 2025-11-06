//! Shared testing utilities for InkGen crates.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use inkgen_core::{
    BaseStructureService, CoreResult, InkgenConfig, InstallMode, LanguagesSection, PackageCache,
    PackageCacheConfig, PackageEntry, TreeShakingSection,
};
use tempfile::TempDir;
use tokio::sync::Mutex;

pub const CORE_PACKAGE: &str = "hl7.fhir.r4.core";
pub const CORE_VERSION: &str = "4.0.1";

/// Utility to create a temporary directory for generator tests.
pub fn temp_output_dir(prefix: &str) -> Result<TempDir> {
    let dir = tempfile::Builder::new().prefix(prefix).tempdir()?;
    Ok(dir)
}

static TEST_ROOT: OnceLock<PathBuf> = OnceLock::new();
static PACKAGE_CACHE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn shared_root() -> &'static PathBuf {
    TEST_ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate directory has parent")
            .parent()
            .expect("workspace has parent")
            .join("target")
            .join("inkgen-test");

        if let Err(err) = std::fs::create_dir_all(&root) {
            panic!("failed to create inkgen test root {:?}: {err}", root);
        }

        root
    })
}

fn package_cache_lock() -> &'static Mutex<()> {
    PACKAGE_CACHE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Filesystem layout shared by integration tests.
#[derive(Debug, Clone)]
pub struct CoreTestWorkspace {
    packages_dir: PathBuf,
    cache_dir: PathBuf,
}

impl CoreTestWorkspace {
    /// Returns a shared workspace rooted under `target/inkgen-test`.
    pub fn shared() -> CoreResult<Self> {
        let root = shared_root().clone();
        let packages_dir = root.join("packages");
        let cache_dir = root.join("cache");
        std::fs::create_dir_all(&packages_dir)?;
        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            packages_dir,
            cache_dir,
        })
    }

    pub fn packages_dir(&self) -> &Path {
        &self.packages_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn package_cache_config(&self) -> CoreResult<PackageCacheConfig> {
        let mut config = PackageCacheConfig::new(&self.packages_dir)?;
        if let Ok(url) = std::env::var("INKGEN_TEST_REGISTRY_URL") {
            config = config.with_registry_url(url);
        }
        Ok(config.with_connection_pool_size(2))
    }

    pub fn default_manifest(&self) -> InkgenConfig {
        InkgenConfig {
            packages: vec![PackageEntry {
                name: CORE_PACKAGE.to_string(),
                version: CORE_VERSION.to_string(),
            }],
            tree_shaking: TreeShakingSection::default(),
            languages: LanguagesSection::default(),
        }
    }

    pub fn manifest_with_resources<I, S>(&self, resources: I) -> InkgenConfig
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        InkgenConfig {
            packages: vec![PackageEntry {
                name: CORE_PACKAGE.to_string(),
                version: CORE_VERSION.to_string(),
            }],
            tree_shaking: TreeShakingSection {
                allowed_resources: resources.into_iter().map(Into::into).collect(),
                allowed_profiles: Vec::new(),
            },
            languages: LanguagesSection::default(),
        }
    }
}

/// Testing context bundling workspace layout, manifest, and package cache.
#[derive(Clone)]
pub struct CoreTestContext {
    workspace: CoreTestWorkspace,
    config: InkgenConfig,
    cache: Arc<PackageCache>,
}

impl CoreTestContext {
    /// Creates a context with the default `hl7.fhir.r4.core` manifest.
    pub async fn new() -> CoreResult<Self> {
        Self::with_allowed_resources(Vec::<String>::new()).await
    }

    /// Creates a context configured to allow only the specified resource types.
    pub async fn with_allowed_resources<I, S>(resources: I) -> CoreResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let workspace = CoreTestWorkspace::shared()?;
        let mut config = workspace.default_manifest();
        config.tree_shaking.allowed_resources = resources.into_iter().map(Into::into).collect();

        let cache_config = workspace.package_cache_config()?;
        let cache = PackageCache::builder().config(cache_config).build().await?;

        // Serialize downloads to reduce concurrent fetch churn in tests.
        let _guard = package_cache_lock().lock().await;
        cache
            .ensure_packages(&config.package_requests(), InstallMode::OnlinePreferred)
            .await?;

        Ok(Self {
            workspace,
            config,
            cache: Arc::new(cache),
        })
    }

    pub fn workspace(&self) -> &CoreTestWorkspace {
        &self.workspace
    }

    pub fn config(&self) -> &InkgenConfig {
        &self.config
    }

    pub fn cache(&self) -> Arc<PackageCache> {
        Arc::clone(&self.cache)
    }

    pub fn structure_service(&self) -> BaseStructureService {
        BaseStructureService::new(self.cache(), self.config.structure_config())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_output_dir_is_created() {
        let dir = temp_output_dir("inkgen-test").expect("dir");
        assert!(dir.path().exists());
    }

    #[tokio::test]
    async fn core_context_installs_core_package() {
        let ctx = CoreTestContext::new().await.expect("context");
        let installed = ctx.cache().list_installed().await.expect("list packages");
        assert!(
            installed.iter().any(|pkg| pkg.starts_with(CORE_PACKAGE)),
            "expected {} in installed packages: {:?}",
            CORE_PACKAGE,
            installed
        );
    }
}
