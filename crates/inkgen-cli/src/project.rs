use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use inkgen_core::{CoreError, InkgenConfig, PackageCacheConfig, PackageRequest, PackageSource};

pub struct ProjectContext {
    manifest: InkgenConfig,
    root: PathBuf,
    manifest_path: PathBuf,
}

impl ProjectContext {
    pub fn load(config: Option<PathBuf>) -> Result<Self> {
        let config_path = config.unwrap_or_else(|| PathBuf::from("inkgen.toml"));
        let config_path = if config_path.is_relative() {
            std::env::current_dir()
                .context("failed to resolve current directory")?
                .join(config_path)
        } else {
            config_path
        };

        let manifest = InkgenConfig::load_from_path(&config_path).map_err(anyhow::Error::from)?;
        let root = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        Ok(Self {
            manifest,
            root,
            manifest_path: config_path,
        })
    }

    pub fn manifest(&self) -> &InkgenConfig {
        &self.manifest
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn default_packages_dir(&self) -> PathBuf {
        self.root.join("target").join("inkgen").join("packages")
    }

    pub fn default_cache_dir(&self) -> PathBuf {
        self.root.join("target").join("inkgen").join("cache")
    }

    pub fn default_output_dir(&self) -> PathBuf {
        self.root
            .join("target")
            .join("inkgen")
            .join("out")
            .join("typescript")
    }

    pub fn package_requests(&self) -> Vec<PackageRequest> {
        self.manifest.package_requests()
    }

    pub fn validate(&self) -> Result<()> {
        if self.manifest.packages.is_empty() {
            anyhow::bail!(
                "manifest contains no packages; add at least one entry under [[packages]]"
            );
        }

        for package in &self.manifest.packages {
            if package.name.trim().is_empty() {
                anyhow::bail!("manifest package name cannot be empty");
            }

            if package.version.trim().is_empty() {
                anyhow::bail!("manifest package {} must specify a version", package.name);
            }
        }

        Ok(())
    }

    pub fn typescript_section(&self) -> Option<&inkgen_core::TypescriptLanguageConfig> {
        self.manifest.typescript_config()
    }

    pub fn build_cache_config(
        &self,
        packages_dir: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
        registry_url: Option<String>,
    ) -> Result<PackageCacheConfig> {
        let packages_dir = packages_dir.unwrap_or_else(|| self.default_packages_dir());
        let cache_dir = cache_dir.unwrap_or_else(|| self.default_cache_dir());

        std::fs::create_dir_all(&packages_dir).with_context(|| {
            format!(
                "failed to create packages directory {}",
                packages_dir.display()
            )
        })?;
        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("failed to create cache directory {}", cache_dir.display()))?;

        let mut config = PackageCacheConfig::new(&packages_dir)
            .map_err(|err: CoreError| anyhow::Error::from(err))?;
        config.cache_dir = cache_dir;
        if let Some(url) = registry_url {
            config = config.with_registry_url(url);
        }
        Ok(config)
    }
}

pub fn select_requests(requests: &[PackageRequest], filters: &[String]) -> Vec<PackageRequest> {
    if filters.is_empty() {
        return requests.to_vec();
    }

    requests
        .iter()
        .filter(|request| {
            filters
                .iter()
                .any(|pattern| matches_package(&request.id, pattern))
        })
        .cloned()
        .collect()
}

fn matches_package(id: &inkgen_core::PackageId, pattern: &str) -> bool {
    if let Some((name, version)) = pattern.split_once('@') {
        id.name == name && id.version == version
    } else {
        id.name == pattern
    }
}

pub fn describe_source(source: &PackageSource) -> String {
    match source {
        PackageSource::Registry => "registry".to_string(),
        PackageSource::Local { path, .. } => format!("local ({})", path.display()),
    }
}
