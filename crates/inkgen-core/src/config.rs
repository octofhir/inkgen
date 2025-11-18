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
    pub languages: LanguagesSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageEntry {
    pub name: String,
    pub version: String,

    /// Optional custom folder name (defaults to sanitized package name)
    #[serde(default)]
    pub folder: Option<String>,

    /// Filter mode for this package
    #[serde(default = "default_filter_mode")]
    pub filter: FilterMode,

    /// Resources to include (only used with filter = "include")
    #[serde(default)]
    pub include_resources: Vec<String>,

    /// Resource URLs to include (only used with filter = "include")
    #[serde(default)]
    pub include_urls: Vec<String>,

    /// Resources to exclude (only used with filter = "exclude")
    #[serde(default)]
    pub exclude_resources: Vec<String>,

    /// Resource URLs to exclude (only used with filter = "exclude")
    #[serde(default)]
    pub exclude_urls: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterMode {
    /// Generate all resources from this package
    All,
    /// Only generate resources referenced by other packages (smart default for base FHIR)
    Dependencies,
    /// Skip this package (reference only)
    None,
    /// Explicit include list
    Include,
    /// Explicit exclude list
    Exclude,
}

fn default_filter_mode() -> FilterMode {
    FilterMode::All
}

// REMOVED: Global tree_shaking section
// Per-package filtering is now the standard approach

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

    /// Template overlay directories for customization (default: empty)
    /// Paths are relative to the manifest file location.
    /// Overlays allow customization of built-in templates by providing files
    /// with the same name in the overlay directories. Overlay templates override
    /// the built-in templates with matching names.
    #[serde(default)]
    pub overlays: Vec<String>,
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

impl PackageEntry {
    /// Get the folder name for this package (custom or sanitized)
    pub fn folder_name(&self) -> String {
        self.folder
            .clone()
            .unwrap_or_else(|| sanitize_package_name(&self.name))
    }

    /// Check if a resource should be included based on this package's filter
    pub fn should_include_resource(&self, name: &str, url: &str) -> bool {
        match self.filter {
            FilterMode::All => true,
            FilterMode::None => false,
            FilterMode::Dependencies => {
                // Dependencies mode handled by dependency analyzer
                // Default to false here, analyzer will override
                false
            }
            FilterMode::Include => {
                // Include if in whitelist (by name or URL)
                self.include_resources.contains(&name.to_string())
                    || self.include_urls.contains(&url.to_string())
            }
            FilterMode::Exclude => {
                // Exclude if in blacklist
                !self.exclude_resources.contains(&name.to_string())
                    && !self.exclude_urls.contains(&url.to_string())
            }
        }
    }

    /// Check if resource should be included considering dependencies
    pub fn should_include_by_filter(&self, url: &str, is_dependency: bool) -> bool {
        match self.filter {
            FilterMode::All => true,
            FilterMode::None => false,
            FilterMode::Dependencies => is_dependency,
            FilterMode::Include => self.include_urls.contains(&url.to_string()),
            FilterMode::Exclude => !self.exclude_urls.contains(&url.to_string()),
        }
    }
}

/// Sanitize package name to a clean folder name
///
/// Rules:
/// - Remove common prefixes: `hl7.fhir.`, `hl7.`, `ihe.`, `org.`
/// - Replace `.` with `-`
///
/// Examples:
/// - `hl7.fhir.r4.core` → `r4-core`
/// - `hl7.fhir.r5.core` → `r5-core`
/// - `hl7.fhir.us.core` → `us-core`
/// - `ihe.iti.pix` → `iti-pix`
/// - `org.example.custom` → `example-custom`
pub fn sanitize_package_name(name: &str) -> String {
    let mut result = name.to_string();

    // Remove common prefixes
    for prefix in ["hl7.fhir.", "hl7.", "ihe.", "org."] {
        if let Some(stripped) = result.strip_prefix(prefix) {
            result = stripped.to_string();
            break;
        }
    }

    // Replace dots with hyphens
    result = result.replace('.', "-");

    result
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

    /// Get structure provider config (per-package filtering now preferred)
    pub fn structure_config(&self) -> StructureProviderConfig {
        // No global filtering - use per-package filtering instead
        StructureProviderConfig {
            allowed_resource_types: None,  // Include all at provider level
            include_profiles: true,         // Filter at generation time
        }
    }

    pub fn typescript_config(&self) -> Option<&TypescriptLanguageConfig> {
        self.languages.typescript.as_ref()
    }
}
