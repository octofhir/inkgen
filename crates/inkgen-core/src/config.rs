use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

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

/// Style of extension accessors to generate in profile classes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionAccessorStyle {
    /// Generate both typed value and raw Extension accessors
    #[default]
    Both,
    /// Generate only typed value getters/setters
    Typed,
    /// Generate only raw Extension getters/setters
    Raw,
}

/// Configuration for profile method generation
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProfileMethodConfig {
    /// Generate extension accessor methods (default: true)
    pub extension_accessors: bool,

    /// Style of extension accessors to generate (default: Both)
    pub extension_style: ExtensionAccessorStyle,

    /// Generate serialization methods (toJson, toObject) (default: true)
    pub serialization: bool,

    /// Generate validation methods (fromJson, fromObject) (default: true)
    pub validation: bool,
}

impl Default for ProfileMethodConfig {
    fn default() -> Self {
        Self {
            extension_accessors: true,
            extension_style: ExtensionAccessorStyle::Both,
            serialization: true,
            validation: true,
        }
    }
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

    #[serde(default = "default_true")]
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

    /// Generate profile classes instead of interfaces (default: true)
    #[serde(default = "default_true")]
    pub profile_classes: bool,

    /// Profile method generation configuration
    #[serde(default)]
    pub profile_methods: ProfileMethodConfig,

    /// Generate Zod schemas for runtime validation (default: true)
    #[serde(default = "default_true")]
    pub zod_schemas: bool,

    /// Co-locate Zod schemas in same file as types (default: true)
    /// If false, generates separate .schemas.ts files
    #[serde(default = "default_true")]
    pub zod_colocated: bool,

    /// Generate branded primitive types for type-level safety (default: false)
    #[serde(default)]
    pub branded_primitives: bool,

    // === Feature 1: Import Optimization ===
    /// Tree-shaking level: "none" | "basic" | "aggressive" (default: "none")
    #[serde(default = "default_tree_shaking")]
    pub tree_shaking: String,

    /// Import style: "named" | "namespace" (default: "named")
    #[serde(default = "default_import_style")]
    pub import_style: String,

    /// Lazy load validation schemas (default: false)
    #[serde(default)]
    pub lazy_schemas: bool,

    // === Feature 2: ValueSet Enhancements ===
    /// Generate rich metadata for ValueSets (definitions, comments, system URLs) (default: false)
    #[serde(default)]
    pub valueset_metadata: bool,

    /// Generate Coding/CodeableConcept helper factories (default: false)
    #[serde(default)]
    pub valueset_helpers: bool,

    /// Link ValueSets to CodeSystem resources for enhanced metadata (default: false)
    #[serde(default)]
    pub valueset_codesystem_link: bool,

    // === Feature 3: Profile Architecture ===
    /// Profile generation mode: "class" | "mixin" | "builder" | "functional" (default: "class")
    #[serde(default = "default_profile_mode")]
    pub profile_mode: String,

    /// Export profile constraints as external metadata objects (default: false)
    #[serde(default)]
    pub profile_constraints_external: bool,

    /// Extension helper style: "bound" | "standalone" | "both" (default: "bound")
    #[serde(default = "default_profile_extension_helpers")]
    pub profile_extension_helpers: String,

    // === Feature 4: Validation Backend ===
    /// Validation backend: "zod" | "json-schema" | "superstruct" | "io-ts" | "arktype" | "none" (default: "zod")
    #[serde(default = "default_validation_backend")]
    pub validation_backend: String,

    /// Generate modular per-element validators (default: false)
    #[serde(default)]
    pub validation_modular: bool,

    // === Feature 5: Interop Utilities ===
    /// Generate interop utilities (default: false)
    #[serde(default)]
    pub generate_interop: bool,

    /// Generate typed Reference<T> helpers (default: false)
    #[serde(default)]
    pub interop_typed_references: bool,

    /// Generate FHIR date parsing/formatting utilities (default: false)
    #[serde(default)]
    pub interop_date_helpers: bool,

    /// Generate Bundle traversal utilities (default: false)
    #[serde(default)]
    pub interop_bundle_traversal: bool,

    /// Generate search parameter helpers (default: false)
    #[serde(default)]
    pub interop_search_helpers: bool,

    /// Enable advanced search parameters (dynamic _include, _has, _filter, enhanced chaining) (default: false)
    #[serde(default)]
    pub interop_search_advanced: bool,

    // === Feature 6: Developer Experience ===
    /// Add tree-shaking hints for bundlers (default: false)
    #[serde(default)]
    pub bundler_hints: bool,
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

fn default_tree_shaking() -> String {
    "none".to_string()
}

fn default_import_style() -> String {
    "named".to_string()
}

fn default_profile_mode() -> String {
    "class".to_string()
}

fn default_profile_extension_helpers() -> String {
    "bound".to_string()
}

fn default_validation_backend() -> String {
    "zod".to_string()
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
///
/// # Examples
///
/// ```
/// use inkgen_core::config::sanitize_package_name;
///
/// assert_eq!(sanitize_package_name("hl7.fhir.r4.core"), "r4-core");
/// assert_eq!(sanitize_package_name("hl7.fhir.us.core"), "us-core");
/// ```
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
            allowed_resource_types: None, // Include all at provider level
            include_profiles: true,       // Filter at generation time
        }
    }

    pub fn typescript_config(&self) -> Option<&TypescriptLanguageConfig> {
        self.languages.typescript.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_method_config_defaults() {
        let config = ProfileMethodConfig::default();
        assert!(config.extension_accessors);
        assert_eq!(config.extension_style, ExtensionAccessorStyle::Both);
        assert!(config.serialization);
        assert!(config.validation);
    }

    #[test]
    fn test_extension_accessor_style_deserialization() {
        // Test "both" variant
        let toml_both = r#"
            extension_accessors = true
            extension_style = "both"
            serialization = true
            validation = true
        "#;
        let config: ProfileMethodConfig = toml::from_str(toml_both).unwrap();
        assert_eq!(config.extension_style, ExtensionAccessorStyle::Both);

        // Test "typed" variant
        let toml_typed = r#"
            extension_accessors = true
            extension_style = "typed"
            serialization = true
            validation = true
        "#;
        let config: ProfileMethodConfig = toml::from_str(toml_typed).unwrap();
        assert_eq!(config.extension_style, ExtensionAccessorStyle::Typed);

        // Test "raw" variant
        let toml_raw = r#"
            extension_accessors = true
            extension_style = "raw"
            serialization = true
            validation = true
        "#;
        let config: ProfileMethodConfig = toml::from_str(toml_raw).unwrap();
        assert_eq!(config.extension_style, ExtensionAccessorStyle::Raw);
    }

    #[test]
    fn test_profile_method_config_custom_values() {
        let toml_config = r#"
            extension_accessors = false
            extension_style = "typed"
            serialization = false
            validation = false
        "#;
        let config: ProfileMethodConfig = toml::from_str(toml_config).unwrap();
        assert!(!config.extension_accessors);
        assert_eq!(config.extension_style, ExtensionAccessorStyle::Typed);
        assert!(!config.serialization);
        assert!(!config.validation);
    }

    #[test]
    fn test_typescript_config_with_profile_methods() {
        let toml_config = r#"
            [languages.typescript]
            mode = "class"
            profile_classes = true

            [languages.typescript.profile_methods]
            extension_accessors = true
            extension_style = "both"
            serialization = true
            validation = true
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        assert!(ts_config.profile_classes);
        assert!(ts_config.profile_methods.extension_accessors);
        assert_eq!(
            ts_config.profile_methods.extension_style,
            ExtensionAccessorStyle::Both
        );
        assert!(ts_config.profile_methods.serialization);
        assert!(ts_config.profile_methods.validation);
    }

    #[test]
    fn test_typescript_config_with_typed_extension_style() {
        let toml_config = r#"
            [languages.typescript]
            profile_classes = true

            [languages.typescript.profile_methods]
            extension_style = "typed"
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        assert_eq!(
            ts_config.profile_methods.extension_style,
            ExtensionAccessorStyle::Typed
        );
        // Other values should be default
        assert!(ts_config.profile_methods.extension_accessors);
        assert!(ts_config.profile_methods.serialization);
        assert!(ts_config.profile_methods.validation);
    }

    #[test]
    fn test_typescript_config_with_raw_extension_style() {
        let toml_config = r#"
            [languages.typescript]
            profile_classes = true

            [languages.typescript.profile_methods]
            extension_style = "raw"
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        assert_eq!(
            ts_config.profile_methods.extension_style,
            ExtensionAccessorStyle::Raw
        );
    }

    #[test]
    fn test_typescript_config_defaults_when_missing() {
        let toml_config = r#"
            [languages.typescript]
            profile_classes = true
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        // profile_methods should use defaults
        assert!(ts_config.profile_methods.extension_accessors);
        assert_eq!(
            ts_config.profile_methods.extension_style,
            ExtensionAccessorStyle::Both
        );
        assert!(ts_config.profile_methods.serialization);
        assert!(ts_config.profile_methods.validation);
    }

    #[test]
    fn test_typescript_config_opt_out_defaults() {
        let toml_config = r#"
            [languages.typescript]
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        assert!(ts_config.structural_guards);
        assert!(ts_config.profile_classes);
        assert!(ts_config.zod_schemas);
        assert!(ts_config.generate_profiles);
        assert!(ts_config.generate_valuesets);
    }

    #[test]
    fn test_typescript_config_can_disable_features() {
        let toml_config = r#"
            [languages.typescript]
            structural_guards = false
            profile_classes = false
            zod_schemas = false
            generate_profiles = false
            generate_valuesets = false
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        assert!(!ts_config.structural_guards);
        assert!(!ts_config.profile_classes);
        assert!(!ts_config.zod_schemas);
        assert!(!ts_config.generate_profiles);
        assert!(!ts_config.generate_valuesets);
    }

    #[test]
    fn test_profile_methods_disabled() {
        let toml_config = r#"
            [languages.typescript]
            profile_classes = true

            [languages.typescript.profile_methods]
            extension_accessors = false
            serialization = false
            validation = false
        "#;
        let config: InkgenConfig = toml::from_str(toml_config).unwrap();
        let ts_config = config.typescript_config().unwrap();
        assert!(!ts_config.profile_methods.extension_accessors);
        assert!(!ts_config.profile_methods.serialization);
        assert!(!ts_config.profile_methods.validation);
    }

    #[test]
    fn test_sanitize_package_name() {
        assert_eq!(sanitize_package_name("hl7.fhir.r4.core"), "r4-core");
        assert_eq!(sanitize_package_name("hl7.fhir.us.core"), "us-core");
        assert_eq!(sanitize_package_name("hl7.terminology"), "terminology");
        assert_eq!(sanitize_package_name("ihe.iti.pix"), "iti-pix");
        assert_eq!(
            sanitize_package_name("org.example.custom"),
            "example-custom"
        );
        assert_eq!(sanitize_package_name("custom.package"), "custom-package");
    }
}
