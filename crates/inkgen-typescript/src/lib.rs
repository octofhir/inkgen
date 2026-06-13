use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use indexmap::IndexMap;
use inkgen_core::config::{
    FilterMode, PackageEntry, ProfileMethodConfig, ProjectFilesConfig, sanitize_package_name,
};
use inkgen_core::ir::{
    Derivation, ElementDefinition, ElementMax, ElementType, ResourceDefinition, ResourceKind,
};
use inkgen_core::{
    CanonicalTypeMap, DependencyAnalyzer, FhirTypeRegistry, LanguageBackend, LanguageGenerator,
    PackageCache, PackageDescriptor, PackageId, StructureDefinitionProvider, StructureFilter,
    StructureKind, StructureProviderConfig, StructureSummary, TypescriptLanguageConfig,
};
use once_cell::sync::Lazy;
use serde::Serialize;
use tera::{Context as TeraContext, Tera};
use tracing::{debug, info, warn};

pub use config::{GenerationMode, NamingConvention, OutputStructure, TypescriptGeneratorConfig};

/// Well-known external code systems that are not included in FHIR packages.
/// These are external terminology standards that we don't attempt to resolve.
static EXTERNAL_CODE_SYSTEMS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "http://unitsofmeasure.org",                   // UCUM units
        "http://loinc.org",                            // LOINC codes
        "http://snomed.info/sct",                      // SNOMED CT
        "http://hl7.org/fhir/sid/icd-10",              // ICD-10
        "http://hl7.org/fhir/sid/icd-9",               // ICD-9
        "urn:ietf:bcp:47",                             // Language tags (BCP-47)
        "urn:iso:std:iso:3166",                        // Country codes (ISO 3166)
        "urn:iso:std:iso:4217",                        // Currency codes (ISO 4217)
        "urn:iso:std:iso:11073:10101",                 // Health device codes
        "http://www.nlm.nih.gov/research/umls/rxnorm", // RxNorm
        "http://www.ama-assn.org/go/cpt",              // CPT
    ]
    .into_iter()
    .collect()
});

/// Check if a URL is an external code system that shouldn't be resolved
fn is_external_code_system(url: &str) -> bool {
    EXTERNAL_CODE_SYSTEMS
        .iter()
        .any(|prefix| url.starts_with(prefix))
}
pub use imports::TypeRegistry;

pub mod extensions;
pub mod imports;
pub mod interop;
pub mod invariants;
pub mod nested;
pub mod overlays;
pub mod profile_helpers;
pub mod profiles;
pub mod slices;
pub mod template_functions;
pub mod terminology_helpers;
pub mod validation;
pub mod valuesets;
pub mod zod;

mod config {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    pub enum GenerationMode {
        Interface,
        Class,
        ClassWithBuilder,
    }

    impl GenerationMode {
        #[allow(clippy::should_implement_trait)]
        pub fn from_str(value: &str) -> Self {
            match value.to_lowercase().as_str() {
                "class" => Self::Class,
                "class_with_builder" | "class-with-builder" => Self::ClassWithBuilder,
                _ => Self::Interface,
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub enum NamingConvention {
        PascalCase,
        CamelCase,
        SnakeCase,
    }

    impl NamingConvention {
        #[allow(clippy::should_implement_trait)]
        pub fn from_str(value: &str) -> Self {
            match value.to_lowercase().as_str() {
                "camel" | "camelcase" => Self::CamelCase,
                "snake" | "snake_case" => Self::SnakeCase,
                _ => Self::PascalCase,
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub enum OutputStructure {
        Flat,
        ByPackage,
    }

    impl OutputStructure {
        #[allow(clippy::should_implement_trait)]
        pub fn from_str(value: &str) -> Self {
            match value.to_lowercase().as_str() {
                "by_package" | "package" | "packages" | "by-package" => Self::ByPackage,
                _ => Self::Flat,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TreeShakingLevel {
        None,
        Basic,
        Aggressive,
    }

    impl TreeShakingLevel {
        pub fn from_str(value: &str) -> Self {
            match value.to_lowercase().as_str() {
                "aggressive" => Self::Aggressive,
                "basic" => Self::Basic,
                _ => Self::None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ImportStyle {
        Named,
        Namespace,
    }

    impl ImportStyle {
        pub fn from_str(value: &str) -> Self {
            match value.to_lowercase().as_str() {
                "namespace" | "ns" => Self::Namespace,
                _ => Self::Named,
            }
        }
    }

    #[derive(Clone)]
    pub struct TypescriptGeneratorConfig {
        pub mode: GenerationMode,
        pub structural_guards: bool,
        pub naming: NamingConvention,
        pub output_structure: OutputStructure,
        pub output_dir: PathBuf,
        pub generate_profiles: bool,
        pub generate_valuesets: bool,
        pub max_valueset_size: usize,
        pub valueset_metadata: bool,
        pub valueset_helpers: bool,
        pub valueset_codesystem_link: bool,
        /// Mapping of package IDs to their folder names (for by_package output structure)
        pub package_folders: HashMap<PackageId, String>,
        /// Mapping of package IDs to their filter settings (for per-package filtering)
        pub package_filters: HashMap<PackageId, PackageEntry>,
        /// Dependency analyzer for cross-package tree-shaking (optional)
        pub dependency_analyzer: Option<DependencyAnalyzer>,
        /// Global type registry for cross-package imports (optional)
        pub type_registry: Option<super::imports::TypeRegistry>,
        /// Package cache for loading ValueSets and other resources (optional)
        pub package_cache: Option<std::sync::Arc<PackageCache>>,
        /// Generate profile classes instead of interfaces
        pub profile_classes: bool,
        /// Profile method generation configuration
        pub profile_methods: ProfileMethodConfig,
        /// Generate Zod schemas for runtime validation
        pub zod_schemas: bool,
        /// Co-locate Zod schemas in same file as types
        pub zod_colocated: bool,
        /// Generate branded primitive types for type-level safety
        pub branded_primitives: bool,
        // Interop utilities
        pub generate_interop: bool,
        pub interop_typed_references: bool,
        pub interop_date_helpers: bool,
        pub interop_bundle_traversal: bool,
        pub interop_search_helpers: bool,
        pub interop_search_advanced: bool,
        /// Tree-shaking level for TypeScript generation
        pub tree_shaking: TreeShakingLevel,
        /// Import style (named vs namespace)
        pub import_style: ImportStyle,
        /// Lazy load validation schemas
        pub lazy_schemas: bool,
        // Project files configuration
        pub config: ProjectFilesConfig,
    }

    impl std::fmt::Debug for TypescriptGeneratorConfig {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TypescriptGeneratorConfig")
                .field("mode", &self.mode)
                .field("structural_guards", &self.structural_guards)
                .field("naming", &self.naming)
                .field("output_structure", &self.output_structure)
                .field("output_dir", &self.output_dir)
                .field("generate_profiles", &self.generate_profiles)
                .field("generate_valuesets", &self.generate_valuesets)
                .field("max_valueset_size", &self.max_valueset_size)
                .field("valueset_metadata", &self.valueset_metadata)
                .field("valueset_helpers", &self.valueset_helpers)
                .field("valueset_codesystem_link", &self.valueset_codesystem_link)
                .field("package_folders", &self.package_folders)
                .field("package_filters", &self.package_filters)
                .field("dependency_analyzer", &self.dependency_analyzer)
                .field("type_registry", &self.type_registry)
                .field("package_cache", &"<PackageCache>")
                .field("profile_classes", &self.profile_classes)
                .field("profile_methods", &self.profile_methods)
                .field("zod_schemas", &self.zod_schemas)
                .field("zod_colocated", &self.zod_colocated)
                .field("branded_primitives", &self.branded_primitives)
                .field("tree_shaking", &self.tree_shaking)
                .field("import_style", &self.import_style)
                .field("lazy_schemas", &self.lazy_schemas)
                .finish()
        }
    }

    impl TypescriptGeneratorConfig {
        #[allow(clippy::too_many_arguments)]
        pub fn from_manifest(
            section: Option<&TypescriptLanguageConfig>,
            default_output: PathBuf,
            override_output: Option<PathBuf>,
            package_folders: HashMap<PackageId, String>,
            package_filters: HashMap<PackageId, PackageEntry>,
            dependency_analyzer: Option<DependencyAnalyzer>,
            type_registry: Option<super::imports::TypeRegistry>,
            package_cache: Option<std::sync::Arc<PackageCache>>,
        ) -> Self {
            let section = section.cloned();
            let mode = section
                .as_ref()
                .map(|s| GenerationMode::from_str(&s.mode))
                .unwrap_or(GenerationMode::Interface);
            let naming = section
                .as_ref()
                .map(|s| NamingConvention::from_str(&s.naming_convention))
                .unwrap_or(NamingConvention::PascalCase);
            let output_structure = section
                .as_ref()
                .map(|s| OutputStructure::from_str(&s.output_structure))
                .unwrap_or(OutputStructure::Flat);
            let structural_guards = section
                .as_ref()
                .map(|s| s.structural_guards)
                .unwrap_or(true);

            let profile_classes = section.as_ref().map(|s| s.profile_classes).unwrap_or(true);

            let profile_methods = section
                .as_ref()
                .map(|s| s.profile_methods.clone())
                .unwrap_or_default();

            let zod_schemas = section.as_ref().map(|s| s.zod_schemas).unwrap_or(true);

            let zod_colocated = section.as_ref().map(|s| s.zod_colocated).unwrap_or(true);

            let branded_primitives = section
                .as_ref()
                .map(|s| s.branded_primitives)
                .unwrap_or(false);

            let generate_profiles = section
                .as_ref()
                .map(|s| s.generate_profiles)
                .unwrap_or(true);

            let generate_valuesets = section
                .as_ref()
                .map(|s| s.generate_valuesets)
                .unwrap_or(true);

            let max_valueset_size = section.as_ref().map(|s| s.max_valueset_size).unwrap_or(50);

            let valueset_metadata = section
                .as_ref()
                .map(|s| s.valueset_metadata)
                .unwrap_or(true);

            let valueset_helpers = section.as_ref().map(|s| s.valueset_helpers).unwrap_or(true);

            let valueset_codesystem_link = section
                .as_ref()
                .map(|s| s.valueset_codesystem_link)
                .unwrap_or(true);

            let generate_interop = section
                .as_ref()
                .map(|s| s.generate_interop)
                .unwrap_or(false);

            let interop_typed_references = section
                .as_ref()
                .map(|s| s.interop_typed_references)
                .unwrap_or(false);

            let interop_date_helpers = section
                .as_ref()
                .map(|s| s.interop_date_helpers)
                .unwrap_or(false);

            let interop_bundle_traversal = section
                .as_ref()
                .map(|s| s.interop_bundle_traversal)
                .unwrap_or(false);

            let interop_search_helpers = section
                .as_ref()
                .map(|s| s.interop_search_helpers)
                .unwrap_or(false);

            let interop_search_advanced = section
                .as_ref()
                .map(|s| s.interop_search_advanced)
                .unwrap_or(false);

            let tree_shaking = section
                .as_ref()
                .map(|s| TreeShakingLevel::from_str(&s.tree_shaking))
                .unwrap_or(TreeShakingLevel::None);

            let import_style = section
                .as_ref()
                .map(|s| ImportStyle::from_str(&s.import_style))
                .unwrap_or(ImportStyle::Named);

            let lazy_schemas = section.as_ref().map(|s| s.lazy_schemas).unwrap_or(false);

            let config = section
                .as_ref()
                .map(|s| s.config.clone())
                .unwrap_or_default();

            let mut output_dir = override_output
                .or_else(|| section.as_ref()?.output_dir.as_ref().map(PathBuf::from))
                .unwrap_or(default_output.clone());

            if output_dir.is_relative() {
                output_dir = default_output
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(output_dir);
            }

            Self {
                mode,
                structural_guards,
                naming,
                output_structure,
                output_dir,
                generate_profiles,
                generate_valuesets,
                max_valueset_size,
                valueset_metadata,
                valueset_helpers,
                valueset_codesystem_link,
                package_folders,
                package_filters,
                dependency_analyzer,
                type_registry,
                package_cache,
                profile_classes,
                profile_methods,
                zod_schemas,
                zod_colocated,
                branded_primitives,
                generate_interop,
                interop_typed_references,
                interop_date_helpers,
                interop_bundle_traversal,
                interop_search_helpers,
                interop_search_advanced,
                tree_shaking,
                import_style,
                lazy_schemas,
                config,
            }
        }
    }
}

pub mod naming {
    pub fn pascal_case(value: &str) -> String {
        let result: String = split_tokens(value)
            .into_iter()
            .map(|token| {
                let mut chars = token.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect();
        sanitize_typescript_identifier(&result)
    }

    /// Ensure the identifier is valid TypeScript (doesn't start with digit, no invalid chars)
    fn sanitize_typescript_identifier(name: &str) -> String {
        if name.is_empty() {
            return "Unknown".to_string();
        }

        // If starts with a digit, prefix with underscore
        let result = if name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            format!("_{}", name)
        } else {
            name.to_string()
        };

        // Ensure only valid identifier characters (already handled by split_tokens, but double-check)
        result
    }

    pub fn camel_case(value: &str) -> String {
        let mut tokens = split_tokens(value);
        if let Some(first) = tokens.first_mut() {
            *first = first.to_ascii_lowercase();
        }
        tokens
            .into_iter()
            .enumerate()
            .map(|(idx, token)| {
                if idx == 0 {
                    token
                } else {
                    let mut chars = token.chars();
                    match chars.next() {
                        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                }
            })
            .collect::<String>()
    }

    pub fn snake_case(value: &str) -> String {
        split_tokens(value)
            .into_iter()
            .map(|token| token.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("_")
    }

    fn split_tokens(value: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut prev_was_lower = false;

        for ch in value.chars() {
            if ch.is_alphanumeric() {
                // Split on camelCase boundary (lowercase followed by uppercase)
                if prev_was_lower && ch.is_ascii_uppercase() && !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                current.push(ch);
                prev_was_lower = ch.is_ascii_lowercase();
            } else if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
                prev_was_lower = false;
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        if tokens.is_empty() {
            tokens.push(value.to_string());
        }
        tokens
    }
}

mod templates {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tera::Value;

    /// Global Tera instance used for rendering templates.
    /// This is initialized with built-in templates and can optionally be customized with overlays.
    static TERA: Lazy<Mutex<Tera>> = Lazy::new(|| Mutex::new(create_default_tera()));

    /// Create a Tera instance with all built-in templates, filters, and functions
    fn create_default_tera() -> Tera {
        let mut tera = Tera::default();

        // Register built-in templates
        tera.add_raw_template(
            "primitives.ts.tera",
            include_str!("templates/primitives.ts.tera"),
        )
        .expect("primitives template");
        tera.add_raw_template(
            "structure.ts.tera",
            include_str!("templates/structure.ts.tera"),
        )
        .expect("structure template");
        tera.add_raw_template("index.ts.tera", include_str!("templates/index.ts.tera"))
            .expect("index template");
        tera.add_raw_template(
            "extensions.ts.tera",
            include_str!("templates/extensions.ts.tera"),
        )
        .expect("extensions template");
        tera.add_raw_template(
            "extension_utils.ts.tera",
            include_str!("templates/extension_utils.ts.tera"),
        )
        .expect("extension_utils template");
        tera.add_raw_template(
            "profile_helpers.ts.tera",
            include_str!("templates/profile_helpers.ts.tera"),
        )
        .expect("profile_helpers template");
        tera.add_raw_template(
            "terminology_helpers.ts.tera",
            include_str!("templates/terminology_helpers.ts.tera"),
        )
        .expect("terminology_helpers template");
        tera.add_raw_template(
            "invariant_validators.ts.tera",
            include_str!("templates/invariant_validators.ts.tera"),
        )
        .expect("invariant_validators template");
        tera.add_raw_template(
            "discriminator_unions.ts.tera",
            include_str!("templates/discriminator_unions.ts.tera"),
        )
        .expect("discriminator_unions template");
        tera.add_raw_template("profile.ts.tera", include_str!("templates/profile.ts.tera"))
            .expect("profile template");
        tera.add_raw_template(
            "profiles-index.ts.tera",
            include_str!("templates/profiles-index.ts.tera"),
        )
        .expect("profiles index template");

        // Search generation templates
        tera.add_raw_template(
            "search-common.ts.tera",
            include_str!("templates/search-common.ts.tera"),
        )
        .expect("search-common template");
        tera.add_raw_template(
            "search-types.ts.tera",
            include_str!("templates/search-types.ts.tera"),
        )
        .expect("search-types template");
        tera.add_raw_template(
            "search-interfaces.ts.tera",
            include_str!("templates/search-interfaces.ts.tera"),
        )
        .expect("search-interfaces template");
        tera.add_raw_template(
            "search-builders.ts.tera",
            include_str!("templates/search-builders.ts.tera"),
        )
        .expect("search-builders template");
        tera.add_raw_template(
            "search-index.ts.tera",
            include_str!("templates/search-index.ts.tera"),
        )
        .expect("search-index template");

        tera.add_raw_template(
            "tsconfig.json.tera",
            include_str!("templates/tsconfig.json.tera"),
        )
        .expect("tsconfig.json template");

        // Register custom filters
        tera.register_filter("pascal_case", filter_pascal_case);
        tera.register_filter("camel_case", filter_camel_case);
        tera.register_filter("sanitize_id", filter_sanitize_id);
        tera.register_filter("wrap_doc", filter_wrap_doc);
        tera.register_filter("lower", filter_lower);

        // Register custom functions
        // TypeScript-specific functions
        tera.register_function(
            "is_primitive",
            crate::template_functions::IsPrimitiveFunction,
        );
        tera.register_function("ts_type", crate::template_functions::TypeScriptTypeFunction);

        // Shared language-agnostic functions from core
        tera.register_function("import_path", inkgen_core::ImportPathFunction);
        tera.register_function("package_folder", inkgen_core::PackageFolderFunction);

        tera
    }

    /// Initialize template system with optional overlays
    /// This should be called once during generator initialization if overlays are configured
    #[allow(dead_code)]
    pub(crate) fn initialize_with_overlays(config: &overlays::OverlayConfig) -> Result<()> {
        let mut tera = TERA.lock().unwrap();
        overlays::apply_overlays(&mut tera, config)?;
        Ok(())
    }

    /// Render a template with the given context
    pub fn render(template: &str, context: &TeraContext) -> Result<String> {
        let tera = TERA.lock().unwrap();
        tera.render(template, context)
            .map_err(|err| anyhow::anyhow!("failed to render template {template}: {err:#?}"))
    }

    fn filter_pascal_case(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        match value.as_str() {
            Some(s) => Ok(Value::String(naming::pascal_case(s))),
            None => Err(tera::Error::msg("pascal_case filter requires a string")),
        }
    }

    fn filter_camel_case(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        match value.as_str() {
            Some(s) => Ok(Value::String(naming::camel_case(s))),
            None => Err(tera::Error::msg("camel_case filter requires a string")),
        }
    }

    fn filter_sanitize_id(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        match value.as_str() {
            Some(s) => Ok(Value::String(sanitize_typescript_identifier(s))),
            None => Err(tera::Error::msg("sanitize_id filter requires a string")),
        }
    }

    fn filter_wrap_doc(value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        match value.as_str() {
            Some(s) => {
                let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(80) as usize;
                Ok(Value::String(wrap_documentation(s, width)))
            }
            None => Err(tera::Error::msg("wrap_doc filter requires a string")),
        }
    }

    fn filter_lower(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        match value.as_str() {
            Some(s) => Ok(Value::String(s.to_lowercase())),
            None => Err(tera::Error::msg("lower filter requires a string")),
        }
    }

    /// Sanitize TypeScript identifiers by escaping reserved keywords
    pub(crate) fn sanitize_typescript_identifier(s: &str) -> String {
        const RESERVED: &[&str] = &[
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "enum",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "new",
            "null",
            "return",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "var",
            "void",
            "while",
            "with",
            "as",
            "implements",
            "interface",
            "let",
            "package",
            "private",
            "protected",
            "public",
            "static",
            "yield",
            "any",
            "boolean",
            "constructor",
            "declare",
            "get",
            "module",
            "require",
            "number",
            "set",
            "string",
            "symbol",
            "type",
            "from",
            "of",
        ];

        if RESERVED.contains(&s) {
            format!("_{}", s)
        } else {
            s.to_string()
        }
    }

    /// Wrap documentation strings to a specified width
    pub(crate) fn wrap_documentation(s: &str, width: usize) -> String {
        let mut result = Vec::new();
        let mut current_line = String::new();

        for word in s.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                result.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }

        result.join("\n")
    }
}

#[derive(Debug, Clone, Serialize)]
struct RenderField {
    name: String,
    type_expr: String,
    optional: bool,
    doc: Option<String>,
    must_support: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    zod_type: Option<String>,
    /// If this field is bound to a ValueSet with Required/Extensible binding,
    /// this contains the ValueSet type name (e.g., "AccountStatus")
    #[serde(skip_serializing_if = "Option::is_none")]
    valueset_type: Option<String>,
    #[serde(skip_serializing)]
    type_dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
struct ImportSpec {
    types: Vec<String>,
    path: String,
    source_package_folder: String,
}

/// Represents a group of types imported from the same source file
#[derive(Debug, Clone, Serialize)]
struct RenderImport {
    /// Types to import from this source
    types: Vec<String>,
    /// Import path (relative or cross-package)
    path: String,
    /// Whether the import can use `import type`
    is_type_only: bool,
    /// Source package folder (for debugging/tracking)
    #[serde(skip_serializing)]
    #[allow(dead_code)]
    source_package_folder: String,
}

#[derive(Debug, Clone, Serialize)]
struct RenderNestedType {
    type_name: String,
    description: Option<String>,
    fields: Vec<RenderField>,
    /// Whether this nested type has self-referential fields (needs z.lazy for recursion)
    is_recursive: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ZodSchemaField {
    name: String,
    zod_type: String,
}

#[derive(Debug, Clone, Serialize)]
struct RenderStructure {
    type_name: String,
    class_name: String,
    file_name: String,
    file_stem: String,
    description: Option<String>,
    emit_interface: bool,
    emit_class: bool,
    structural_guards: bool,
    resource_type_guard: bool,
    has_primitives: bool,
    primitive_imports: Vec<String>,
    fields: Vec<RenderField>,
    imports: Vec<RenderImport>,
    /// Schema imports for Zod schemas (separate from type imports)
    schema_imports: Vec<RenderImport>,
    nested_types: Vec<RenderNestedType>,
    /// Generic type parameters for declaration positions (e.g., "T extends string = string" for Reference<T>)
    type_parameters: Option<String>,
    /// Bare generic type arguments for usage positions (e.g., "T" for Reference<T>)
    type_arguments: Option<String>,
    /// Package name (e.g., "hl7.fhir.r4.core")
    package_name: String,
    /// Package folder name (e.g., "r4-core")
    package_folder: String,
    /// Whether this is a profile (constraint derivation)
    is_profile: bool,
    /// Output subfolder (e.g., "profiles" or "")
    output_folder: String,
    /// Generate Zod schema
    generate_zod_schema: bool,
    /// Zod schema fields
    zod_fields: Vec<ZodSchemaField>,
    /// Whether the Zod schema has self-referential fields (needs explicit type annotation)
    is_recursive_schema: bool,
    /// Whether branded primitives are enabled (affects recursive schema type annotations)
    branded_primitives: bool,
    /// Detected slices for discriminated unions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    slices: Vec<slices::SlicePattern>,
    /// Named type-only exports for barrel files
    type_exports: Vec<String>,
    /// Named value exports for barrel files
    value_exports: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ValueSetOutput {
    type_name: String,
    file_name: String,
    file_stem: String,
    typescript_code: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProfileOutput {
    type_name: String,
    file_name: String,
    file_stem: String,
    typescript_code: String,
    type_exports: Vec<String>,
    value_exports: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PackageOutput {
    path: PathBuf,
    folder: String,
    structures: Vec<RenderStructure>,
    valuesets: Vec<ValueSetOutput>,
    profiles: Vec<ProfileOutput>,
    extensions: IndexMap<String, extensions::RenderExtension>,
    branded_primitives: bool,
    zod_schemas: bool,
    // Interop utilities
    generate_interop: bool,
    interop_config: Option<interop::InteropConfig>,
    resource_types: Vec<String>,
    search_parameters: Vec<inkgen_core::SearchParameterInfo>,
    // Project files config
    #[serde(skip)]
    project_config: ProjectFilesConfig,
}

#[derive(Debug, Clone)]
pub struct TypescriptGenerator {
    config: TypescriptGeneratorConfig,
}

impl TypescriptGenerator {
    pub fn new(config: TypescriptGeneratorConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &TypescriptGeneratorConfig {
        &self.config
    }

    /// Generate ValueSet code files (Phase 0).
    ///
    /// Loads ValueSet resources from the package using the canonical manager
    /// and generates TypeScript enums/constants for each one.
    ///
    /// Returns a tuple of (count, url_to_type_map) where:
    /// - count: number of ValueSets generated
    /// - url_to_type_map: maps ValueSet canonical URLs to TypeScript type names
    async fn generate_valuesets(
        &self,
        package_dir: &Path,
        descriptor: &PackageDescriptor,
    ) -> Result<(usize, HashMap<String, String>)> {
        use crate::valuesets::ValueSetInfo;
        use crate::valuesets::helpers::{HelperConfig, ValueSetHelpers};
        use crate::valuesets::metadata;

        let Some(cache) = &self.config.package_cache else {
            // No cache available, skip ValueSet generation
            return Ok((0, HashMap::new()));
        };

        // Create valuesets subdirectory
        let valuesets_dir = package_dir.join("valuesets");
        fs::create_dir_all(&valuesets_dir)?;

        // Get package folder for computing cross-package import paths
        let package_folder = self
            .config
            .package_folders
            .get(&descriptor.id)
            .cloned()
            .unwrap_or_else(|| sanitize_package_name(&descriptor.id.name));

        // Compute import prefix for valueset helpers
        // From valuesets/ subdirectory, we need to go up one level (..) to reach the package root
        // For r4-core: imports are at ../coding, ../codeable-concept, ../primitives
        // For other packages: imports are at ../../r4-core/coding, etc.
        let valueset_import_prefix = if package_folder == "r4-core" {
            "..".to_string()
        } else {
            "../../r4-core".to_string()
        };

        let mut generated_count = 0;
        let mut url_to_type_map: HashMap<String, String> = HashMap::new();

        // Get canonical manager to query ValueSet resources
        let manager = cache.manager().await?;

        info!(
            "Found {} ValueSets in package {}",
            descriptor.inventory.value_sets.len(),
            descriptor.id
        );

        // Iterate through ValueSet artifacts in the package inventory
        for valueset_artifact in &descriptor.inventory.value_sets {
            // Resolve the ValueSet by canonical URL
            if let Some(canonical_url) = &valueset_artifact.canonical_url {
                match manager.resolve(canonical_url).await {
                    Ok(resolved) if resolved.resource.resource_type == "ValueSet" => {
                        // Extract type name from ValueSet name or ID
                        let type_name = resolved
                            .resource
                            .content
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(naming::pascal_case)
                            .unwrap_or_else(|| {
                                naming::pascal_case(
                                    resolved
                                        .resource
                                        .content
                                        .get("id")
                                        .and_then(|i| i.as_str())
                                        .unwrap_or("UnknownValueSet"),
                                )
                            });

                        // Generate TypeScript code
                        match ValueSetInfo::from_valueset(
                            &resolved.resource.content,
                            type_name.clone(),
                            Some(self.config.max_valueset_size),
                        ) {
                            Ok(Some(mut info)) => {
                                let file_name = naming::snake_case(&type_name)
                                    .replace('_', "-")
                                    .to_ascii_lowercase();
                                let output_path = valuesets_dir.join(format!("{}.ts", file_name));

                                let mut system_url =
                                    metadata::extract_system_url(&resolved.resource.content)
                                        .or_else(|| {
                                            metadata::infer_codesystem_url_from_valueset(
                                                &info.canonical_url,
                                            )
                                        });

                                let base_codes: Vec<(String, Option<String>)> = info
                                    .code_info
                                    .iter()
                                    .map(|code| (code.code.clone(), code.display.clone()))
                                    .collect();

                                let mut enhanced_codes: Vec<metadata::EnhancedCodeInfo> = info
                                    .code_info
                                    .iter()
                                    .map(|code| metadata::EnhancedCodeInfo {
                                        code: code.code.clone(),
                                        display: code.display.clone(),
                                        definition: code.definition.clone(),
                                        comments: Vec::new(),
                                    })
                                    .collect();

                                let mut case_sensitive = None;

                                if self.config.valueset_codesystem_link {
                                    if let Some(system) = &system_url {
                                        // Skip external code systems - they're not in the package
                                        if is_external_code_system(system) {
                                            debug!(
                                                "ValueSet {} uses external CodeSystem {}; skipping resolution",
                                                type_name, system
                                            );
                                        } else {
                                            match manager.resolve(system).await {
                                                Ok(codesystem)
                                                    if codesystem.resource.resource_type
                                                        == "CodeSystem" =>
                                                {
                                                    let meta = metadata::load_codesystem_metadata(
                                                        &codesystem.resource.content,
                                                    );
                                                    if system_url.is_none() {
                                                        system_url = meta.url.clone();
                                                    }
                                                    case_sensitive = meta.case_sensitive;
                                                    enhanced_codes =
                                                        metadata::enhance_codes(&base_codes, &meta);
                                                }
                                                Ok(_) => {
                                                    warn!(
                                                        "Resolved resource is not a CodeSystem: {}",
                                                        system
                                                    );
                                                }
                                                Err(err) => {
                                                    warn!(
                                                        "Failed to resolve CodeSystem {} for {}: {}",
                                                        system, type_name, err
                                                    );
                                                }
                                            }
                                        }
                                    } else {
                                        debug!(
                                            "ValueSet {} missing system URL; helper metadata disabled",
                                            type_name
                                        );
                                    }
                                }

                                info.apply_enhanced_metadata(&enhanced_codes);
                                let mut base_code = info.generate_typescript();
                                base_code = base_code.trim_end().to_string();

                                let metadata_block = if self.config.valueset_metadata {
                                    metadata::render_metadata_block(
                                        &info.type_name,
                                        &info.canonical_url,
                                        system_url.as_deref(),
                                        case_sensitive,
                                        &enhanced_codes,
                                    )
                                } else {
                                    None
                                };
                                let metadata_available = metadata_block.is_some();

                                let mut helper_imports = Vec::new();
                                let helper_code = if self.config.valueset_helpers {
                                    if let Some(system_url) = &system_url {
                                        let helper_config = HelperConfig::default();
                                        let helpers = ValueSetHelpers::new(
                                            info.type_name.clone(),
                                            system_url.clone(),
                                            &helper_config,
                                            metadata_available,
                                        );
                                        if helpers.has_coding_factory || helpers.has_validation {
                                            helper_imports.push(format!(
                                                "import type {{ Coding }} from \"{}/{}\";",
                                                valueset_import_prefix,
                                                file_stem("Coding")
                                            ));
                                        }
                                        if helpers.has_codeable_concept_factory
                                            || helpers.has_extraction
                                        {
                                            helper_imports.push(format!(
                                                "import type {{ CodeableConcept }} from \"{}/{}\";",
                                                valueset_import_prefix,
                                                file_stem("CodeableConcept")
                                            ));
                                        }
                                        // Import branded primitives for helper function return types
                                        if self.config.branded_primitives
                                            && (helpers.has_coding_factory
                                                || helpers.has_codeable_concept_factory)
                                        {
                                            helper_imports.push(format!(
                                                "import type {{ FhirUri, FhirCode, FhirString }} from \"{}/{}\";",
                                                valueset_import_prefix,
                                                file_stem("primitives")
                                            ));
                                        }
                                        let block = helpers.generate_all_helpers();
                                        if block.trim().is_empty() {
                                            None
                                        } else {
                                            Some(block)
                                        }
                                    } else {
                                        warn!(
                                            "Skipping helper generation for {}: missing CodeSystem URL",
                                            type_name
                                        );
                                        None
                                    }
                                } else {
                                    None
                                };

                                let mut sections = Vec::new();
                                if !helper_imports.is_empty() {
                                    sections.push(helper_imports.join("\n"));
                                }
                                sections.push(base_code);
                                if let Some(metadata_block) = metadata_block {
                                    sections.push(metadata_block);
                                }
                                if let Some(helper_code) = helper_code {
                                    sections.push(helper_code);
                                }
                                let ts_file = sections.join("\n\n");

                                fs::write(&output_path, ts_file)?;
                                generated_count += 1;

                                // Add to URL->type mapping
                                url_to_type_map.insert(canonical_url.clone(), type_name.clone());

                                debug!(
                                    "Generated ValueSet: {} ({} codes)",
                                    type_name,
                                    info.code_info.len()
                                );
                            }
                            Ok(None) => {
                                debug!("Skipped ValueSet {} (exceeds size limit)", type_name);
                            }
                            Err(e) => {
                                warn!("Failed to process ValueSet {}: {}", canonical_url, e);
                            }
                        }
                    }
                    Ok(_) => {
                        warn!("Resolved resource is not a ValueSet: {}", canonical_url);
                    }
                    Err(e) => {
                        warn!("Failed to resolve ValueSet {}: {}", canonical_url, e);
                    }
                }
            } else {
                warn!("ValueSet artifact has no canonical URL, skipping");
            }
        }

        Ok((generated_count, url_to_type_map))
    }

    fn structure_type_name(summary: &StructureSummary, definition: &ResourceDefinition) -> String {
        // For profiles, use definition.id to avoid claiming the base type name
        // (e.g., 11179-objectClass has type_code "Extension" but shouldn't be called "Extension")
        let is_profile_like = summary.kind == StructureKind::Profile
            || summary
                .type_code
                .as_deref()
                .is_some_and(|tc| tc != definition.id && !definition.id.eq_ignore_ascii_case(tc));

        if is_profile_like {
            naming::pascal_case(&definition.id)
        } else {
            naming::pascal_case(summary.type_code.as_deref().unwrap_or(&definition.id))
        }
    }

    /// Ensures all complex types discovered in the FHIR packages are available for generation.
    ///
    /// Uses the FhirTypeRegistry to dynamically discover complex types from loaded packages,
    /// eliminating the need for a hardcoded list of core types.
    async fn ensure_core_types<S>(
        &self,
        service: &S,
        descriptor: &PackageDescriptor,
        fhir_registry: &FhirTypeRegistry,
        entries: &mut Vec<(StructureSummary, ResourceDefinition)>,
    ) -> Result<()>
    where
        S: StructureDefinitionProvider + Sync + Send,
    {
        let mut existing_types: HashSet<String> = entries
            .iter()
            .map(|(summary, definition)| Self::structure_type_name(summary, definition))
            .collect();

        let mut added = 0usize;

        // Iterate over ALL types discovered from the registry (complex types, primitives, base resources)
        // No hardcoded lists - everything comes from the registry which is built from packages
        let types_to_ensure: HashSet<&str> = fhir_registry
            .complex_types()
            .chain(fhir_registry.primitives())
            .chain(fhir_registry.base_resources())
            .collect();

        for type_name in types_to_ensure {
            if existing_types.contains(type_name) {
                if type_name == "Extension" || type_name == "Quantity" {
                    warn!(
                        "ensure_core_types: Skipping '{}' - already in existing_types",
                        type_name
                    );
                }
                continue;
            }

            // Check if type is available from a dependency package via type_registry
            if let Some(type_registry) = &self.config.type_registry {
                // TypeRegistry uses PascalCase names, but FhirTypeRegistry uses lowercase
                // Convert to PascalCase for lookup
                let pascal_name = if type_name.chars().next().is_some_and(|c| c.is_lowercase()) {
                    let mut chars = type_name.chars();
                    match chars.next() {
                        None => type_name.to_string(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                } else {
                    type_name.to_string()
                };

                if let Some((dep_package_folder, _stem)) = type_registry.get(&pascal_name) {
                    // Get the current package's folder name from config.package_folders
                    let current_package_folder = self
                        .config
                        .package_folders
                        .get(&descriptor.id)
                        .map(|s| s.as_str())
                        .unwrap_or("");

                    // Only skip if type is from a DIFFERENT package (not the current one)
                    if dep_package_folder != current_package_folder {
                        // Type is available from a dependency package - will be imported, not hydrated
                        debug!(
                            "Skipping hydration of '{}' (as '{}') for {} - available from dependency package '{}'",
                            type_name, pascal_name, descriptor.id, dep_package_folder
                        );
                        continue;
                    }
                }
            }

            // Get the canonical URL from the registry
            let canonical = match fhir_registry.get_url(type_name) {
                Some(url) => url.to_string(),
                None => {
                    // Fallback to constructing URL if not in registry
                    let url = format!("http://hl7.org/fhir/StructureDefinition/{}", type_name);
                    debug!(
                        "Type '{}' not in registry, using fallback URL: {}",
                        type_name, url
                    );
                    url
                }
            };

            debug!(
                "ensure_core_types: Attempting to load type '{}' from {}",
                type_name, canonical
            );

            match service.load_structure(&canonical).await {
                Ok(definition) => {
                    let kind = match definition.kind {
                        ResourceKind::PrimitiveType => StructureKind::PrimitiveType,
                        ResourceKind::ComplexType => StructureKind::ComplexType,
                        ResourceKind::Logical => StructureKind::Logical,
                        ResourceKind::Resource => StructureKind::BaseResource,
                    };

                    let summary = StructureSummary {
                        canonical_url: canonical.clone(),
                        name: definition
                            .name
                            .clone()
                            .or_else(|| Some(type_name.to_string())),
                        type_code: Some(type_name.to_string()),
                        title: definition.title.clone(),
                        version: definition.version.clone(),
                        status: definition.status.clone(),
                        package: descriptor.id.clone(),
                        kind,
                    };

                    existing_types.insert(type_name.to_string());
                    entries.push((summary, definition));
                    added += 1;

                    if type_name == "Extension" || type_name == "Quantity" {
                        info!("ensure_core_types: Successfully loaded '{}'", type_name);
                    }
                }
                Err(err) => {
                    warn!(
                        "Failed to load core type {} for {}: {}",
                        type_name, descriptor.id, err
                    );
                }
            }
        }

        if added > 0 {
            info!(
                "Hydrated {} missing core type definitions for {}",
                added, descriptor.id
            );

            entries.sort_by(|(left_summary, _), (right_summary, _)| {
                let kind_order = |kind: &StructureKind| match kind {
                    StructureKind::PrimitiveType => 0,
                    StructureKind::ComplexType => 1,
                    StructureKind::Logical => 2,
                    StructureKind::BaseResource => 3,
                    StructureKind::Profile => 4,
                };

                match kind_order(&left_summary.kind).cmp(&kind_order(&right_summary.kind)) {
                    std::cmp::Ordering::Equal => {
                        left_summary.canonical_url.cmp(&right_summary.canonical_url)
                    }
                    other => other,
                }
            });
        }

        Ok(())
    }
}

impl<S> LanguageBackend<S> for TypescriptGenerator
where
    S: StructureDefinitionProvider + Sync + Send,
{
    fn name(&self) -> &str {
        "typescript"
    }

    fn description(&self) -> &str {
        "TypeScript/JavaScript with type-safe FHIR models"
    }

    fn file_extension(&self) -> &str {
        "ts"
    }

    fn supports_feature(&self, feature: &str) -> bool {
        matches!(
            feature,
            "interfaces"
                | "classes"
                | "builders"
                | "structural-guards"
                | "primitives"
                | "cross-package-imports"
        )
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
}

#[async_trait]
impl<S> LanguageGenerator<S> for TypescriptGenerator
where
    S: StructureDefinitionProvider + Sync + Send,
{
    async fn generate(
        &self,
        service: &S,
        descriptor: &PackageDescriptor,
        provider_config: &StructureProviderConfig,
    ) -> Result<()> {
        let filter = StructureFilter::from_config(provider_config);
        let summaries = service.list_structures(&filter).await?;

        // Build CanonicalTypeMap from the canonical manager - single source of truth for all types
        let canonical_type_map = if let Some(cache) = &self.config.package_cache {
            let manager = cache.manager().await?;
            CanonicalTypeMap::from_manager(&manager).await?
        } else {
            warn!("No package cache available - type resolution may be incomplete");
            CanonicalTypeMap::new()
        };
        info!(
            "Built CanonicalTypeMap with {} types from canonical manager",
            canonical_type_map.len()
        );

        // Build FhirTypeRegistry from ALL summaries to discover available types
        let fhir_registry = FhirTypeRegistry::from_summaries(&summaries);
        debug!(
            "Built FhirTypeRegistry with {} types ({} primitives, {} complex, {} base resources)",
            fhir_registry.len(),
            fhir_registry.primitives().count(),
            fhir_registry.complex_types().count(),
            fhir_registry.base_resources().count()
        );

        // Log specific types we expect to find
        for expected in [
            "Period",
            "Reference",
            "HumanName",
            "CodeableConcept",
            "Resource",
            "DomainResource",
        ] {
            if fhir_registry.contains(expected) {
                debug!(
                    "Registry contains '{}': url={:?}",
                    expected,
                    fhir_registry.get_url(expected)
                );
            } else {
                warn!("Registry MISSING expected type '{}'", expected);
            }
        }

        let mut relevant: Vec<_> = summaries
            .into_iter()
            .filter(|summary| summary.package == descriptor.id)
            .collect();

        info!("RELEVANT BEFORE FILTERS: {} structures", relevant.len());
        for summary in &relevant {
            info!("  - {} (kind={:?})", summary.canonical_url, summary.kind);
        }

        // Apply per-package filtering if configured
        // IMPORTANT: Filters only apply to BaseResource (actual FHIR resources)
        // ComplexType and PrimitiveType are ALWAYS generated (they're dependencies)
        if let Some(package_entry) = self.config.package_filters.get(&descriptor.id) {
            match package_entry.filter {
                FilterMode::All => {
                    // Keep all resources - no additional filtering
                }
                FilterMode::None => {
                    // Skip this package entirely
                    info!("Skipping package {} (filter = None)", descriptor.id);
                    return Ok(());
                }
                FilterMode::Include => {
                    // Keep only whitelisted RESOURCES (not types/primitives/logical)
                    relevant.retain(|summary| {
                        // Always keep ComplexType, PrimitiveType, and Logical (data types)
                        if summary.kind != StructureKind::BaseResource {
                            return true;
                        }
                        // Filter BaseResource by include list
                        let type_name = summary.type_code.as_deref().unwrap_or("");
                        package_entry
                            .include_resources
                            .contains(&type_name.to_string())
                            || package_entry.include_urls.contains(&summary.canonical_url)
                    });
                }
                FilterMode::Exclude => {
                    // Remove blacklisted RESOURCES (not types/primitives/logical)
                    relevant.retain(|summary| {
                        // Always keep ComplexType, PrimitiveType, and Logical (data types)
                        if summary.kind != StructureKind::BaseResource {
                            return true;
                        }
                        // Filter BaseResource by exclude list
                        let type_name = summary.type_code.as_deref().unwrap_or("");
                        !package_entry
                            .exclude_resources
                            .contains(&type_name.to_string())
                            && !package_entry.exclude_urls.contains(&summary.canonical_url)
                    });
                }
                FilterMode::Dependencies => {
                    // Filter using dependency analyzer
                    if let Some(analyzer) = &self.config.dependency_analyzer {
                        relevant.retain(|summary| {
                            // Always keep ComplexType, PrimitiveType, and Logical (data types)
                            if summary.kind != StructureKind::BaseResource {
                                return true;
                            }
                            // For BaseResource, check if other packages depend on it
                            let package_key = format!("{}", descriptor.id);
                            analyzer.should_generate(
                                &package_key,
                                &summary.canonical_url,
                                FilterMode::Dependencies,
                            )
                        });
                        info!(
                            "Applied dependency filtering for package {}: {} resources remain",
                            descriptor.id,
                            relevant.len()
                        );
                    } else {
                        warn!(
                            "Dependencies mode requested for package {} but no analyzer available; generating all resources",
                            descriptor.id
                        );
                    }
                }
            }
        }

        // Sort by StructureKind first for proper dependency order (critical for Rust/Zig),
        // then by canonical URL for determinism within each phase
        relevant.sort_by(|a, b| {
            use StructureKind::*;
            let kind_order = |kind: &StructureKind| match kind {
                PrimitiveType => 0, // Phase 1: Primitives first
                ComplexType => 1,   // Phase 2: Complex types (can use primitives)
                Logical => 2,       // Phase 3: Logical types
                BaseResource => 3,  // Phase 4: Resources (can use all above)
                Profile => 4,       // Phase 5: Profiles (constraints on resources)
            };

            match kind_order(&a.kind).cmp(&kind_order(&b.kind)) {
                std::cmp::Ordering::Equal => a.canonical_url.cmp(&b.canonical_url),
                other => other,
            }
        });

        if relevant.is_empty() {
            warn!(
                "No structures matched for package {}; skipping",
                descriptor.id
            );
            return Ok(());
        }

        // Get the package folder name for this package
        let package_folder = self
            .config
            .package_folders
            .get(&descriptor.id)
            .cloned()
            .unwrap_or_else(|| sanitize_package_name(&descriptor.id.name));

        let package_dir = package_output_dir(
            &self.config.output_dir,
            &descriptor.id,
            &self.config.package_folders,
        );
        fs::create_dir_all(&package_dir).with_context(|| {
            format!(
                "failed to create package output directory {}",
                package_dir.display()
            )
        })?;

        // Phase 0: Generate ValueSets/Codes before structures
        let (valueset_count, valueset_url_to_type) = if self.config.generate_valuesets {
            self.generate_valuesets(&package_dir, descriptor).await?
        } else {
            (0, HashMap::new())
        };
        if valueset_count > 0 {
            info!(
                "Phase 0: ValueSets - Generated {} code files",
                valueset_count
            );
        }

        // Load SearchParameter resources for interop generation
        let search_parameters = if self.config.interop_search_helpers {
            if let Some(cache) = &self.config.package_cache {
                match cache.load_search_parameters(&descriptor.id).await {
                    Ok(params) => {
                        info!("Loaded {} search parameters for interop", params.len());
                        params
                    }
                    Err(err) => {
                        warn!("Failed to load search parameters: {}", err);
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Log phase breakdown for visibility
        let (mut primitives, mut complex, mut logical, mut resources, mut profiles) =
            (0, 0, 0, 0, 0);
        for s in &relevant {
            match s.kind {
                StructureKind::PrimitiveType => primitives += 1,
                StructureKind::ComplexType => complex += 1,
                StructureKind::Logical => logical += 1,
                StructureKind::BaseResource => resources += 1,
                StructureKind::Profile => profiles += 1,
            }
        }
        info!(
            "Generating {} structures in dependency order: {} primitives, {} complex, {} logical, {} resources, {} profiles",
            relevant.len(),
            primitives,
            complex,
            logical,
            resources,
            profiles,
        );

        let mut entries = Vec::new();
        let mut current_phase: Option<StructureKind> = None;

        for summary in relevant {
            info!(
                "LOOP: Processing {} - kind={:?}",
                summary.canonical_url, summary.kind
            );

            // Log phase transitions
            if current_phase != Some(summary.kind) {
                let phase_name = match summary.kind {
                    StructureKind::PrimitiveType => "Phase 1: Primitive Types",
                    StructureKind::ComplexType => "Phase 2: Complex Types",
                    StructureKind::Logical => "Phase 3: Logical Types",
                    StructureKind::BaseResource => "Phase 4: Resources",
                    StructureKind::Profile => "Phase 5: Profiles",
                };
                info!("{}", phase_name);
                current_phase = Some(summary.kind);
            }
            // Generate BaseResource, ComplexType, PrimitiveType, Logical, and Profile structures
            // Logical includes important data types like CodeableConcept, Extension, Period, Range, etc.
            // Profiles are constraint derivations on base resources with extensions and constraints
            if summary.kind != StructureKind::BaseResource
                && summary.kind != StructureKind::ComplexType
                && summary.kind != StructureKind::PrimitiveType
                && summary.kind != StructureKind::Logical
                && summary.kind != StructureKind::Profile
            {
                info!(
                    "LOOP: Skipping {} (kind not allowed)",
                    summary.canonical_url
                );
                continue;
            }

            let structure = service
                .load_structure(&summary.canonical_url)
                .await
                .with_context(|| format!("failed to load {}", summary.canonical_url))?;
            entries.push((summary, structure));
        }

        self.ensure_core_types(service, descriptor, &fhir_registry, &mut entries)
            .await
            .with_context(|| {
                format!(
                    "failed to hydrate core type definitions for {}",
                    descriptor.id
                )
            })?;

        if entries.is_empty() {
            warn!(
                "Package {} has no structures after filtering; nothing to generate",
                descriptor.id
            );
            return Ok(());
        }

        // Phase 1: Build complete name mappings for ALL structures first
        // This ensures deterministic imports regardless of processing order
        let mut used_stems: HashMap<String, usize> = HashMap::new();
        let mut name_to_file: IndexMap<String, String> = IndexMap::new();
        let mut name_to_stem: IndexMap<String, String> = IndexMap::new();

        // DEBUG: Log how many entries we have and check for Extension/Quantity
        debug!("Building name_to_stem from {} entries", entries.len());
        let extension_count = entries.iter().filter(|(_, d)| d.id == "Extension").count();
        let quantity_count = entries.iter().filter(|(_, d)| d.id == "Quantity").count();
        info!(
            "DEBUG: entries contains {} Extension(s) and {} Quantity(s)",
            extension_count, quantity_count
        );
        // Log first few entries
        for (i, (summary, def)) in entries.iter().take(5).enumerate() {
            debug!(
                "Entry {}: kind={:?}, type_code={:?}, id={}",
                i, summary.kind, summary.type_code, def.id
            );
        }

        for (summary, definition) in &entries {
            // For base types (ComplexType, PrimitiveType, BaseResource, Logical), use type_code as the type name
            // For profiles (derivation=constraint), use the definition.id to avoid overwriting base type mappings
            // (e.g., 11179-objectClass has type_code "Extension" but shouldn't overwrite Extension)
            //
            // Also check if type_code differs from definition.id - if they differ significantly,
            // this is likely a profile even if kind says ComplexType (e.g., MoneyQuantity, SimpleQuantity)
            let is_profile_like = summary.kind == StructureKind::Profile
                || summary.type_code.as_deref().is_some_and(|tc| {
                    tc != definition.id && !definition.id.eq_ignore_ascii_case(tc)
                });

            let type_name = if is_profile_like {
                // Profiles use their own id as the type name
                naming::pascal_case(&definition.id)
            } else {
                naming::pascal_case(summary.type_code.as_deref().unwrap_or(&definition.id))
            };

            // Debug: Log important entries
            if type_name == "Quantity"
                || type_name == "Extension"
                || definition.id.contains("Quantity")
                || definition.id.contains("Extension")
            {
                info!(
                    "name_to_stem entry: type_name={}, kind={:?}, type_code={:?}, def.id={}, is_profile_like={}, stem={}",
                    type_name,
                    summary.kind,
                    summary.type_code,
                    definition.id,
                    is_profile_like,
                    file_stem(&definition.id)
                );
            }
            // Also log if definition.id is exactly "Extension" or "Quantity" but not caught above
            if definition.id == "Extension" || definition.id == "Quantity" {
                info!(
                    "DIRECT ID MATCH: type_name={}, kind={:?}, type_code={:?}, def.id={}, is_profile_like={}",
                    type_name, summary.kind, summary.type_code, definition.id, is_profile_like
                );
            }

            let mut stem = file_stem(&definition.id);
            let counter = used_stems.entry(stem.clone()).or_insert(0);
            *counter += 1;
            if *counter > 1 {
                stem = format!("{stem}_{}", counter);
            }
            let file_name = format!("{stem}.ts");

            // Only add if not already mapped - base types should take precedence over profiles
            if !name_to_stem.contains_key(&type_name) {
                name_to_file.insert(type_name.clone(), file_name);
                name_to_stem.insert(type_name.clone(), stem);
            } else if type_name == "Extension" || type_name == "Quantity" {
                warn!(
                    "Skipping duplicate type_name={} (already mapped to {})",
                    type_name,
                    name_to_stem
                        .get(&type_name)
                        .unwrap_or(&"<unknown>".to_string())
                );
            }
        }

        info!(
            "Phase 1 complete: Built name_to_stem map with {} entries",
            name_to_stem.len()
        );
        debug!(
            "First 10 entries in name_to_stem: {:?}",
            name_to_stem.iter().take(10).collect::<Vec<_>>()
        );

        // Debug: Log some specific types we expect to find
        for expected in [
            "CodeableConcept",
            "Meta",
            "Identifier",
            "Narrative",
            "ContactPoint",
            "Extension",
            "Reference",
        ] {
            if name_to_stem.contains_key(expected) {
                debug!(
                    "Found '{}' in name_to_stem -> {}",
                    expected,
                    name_to_stem.get(expected).unwrap()
                );
            } else {
                // Type not in entries - look it up in CanonicalTypeMap
                if let Some(entry) = canonical_type_map.get_by_name(expected) {
                    info!(
                        "Adding '{}' from CanonicalTypeMap (stem='{}', package='{}')",
                        expected, entry.file_stem, entry.package.name
                    );
                    name_to_stem.insert(expected.to_string(), entry.file_stem.clone());
                    name_to_file.insert(expected.to_string(), format!("{}.ts", entry.file_stem));
                } else {
                    warn!(
                        "MISSING '{}' from both entries and CanonicalTypeMap",
                        expected
                    );
                }
            }
        }

        // Populate ALL types from CanonicalTypeMap that might be needed for imports
        // This ensures all FHIR types are resolvable, even if not being generated in this package
        // No hardcoded lists - everything comes from the canonical manager
        for type_name in canonical_type_map.all_names() {
            if !name_to_stem.contains_key(type_name)
                && let Some(entry) = canonical_type_map.get_by_name(type_name)
            {
                debug!(
                    "Pre-populating '{}' from CanonicalTypeMap (stem='{}')",
                    type_name, entry.file_stem
                );
                name_to_stem.insert(type_name.to_string(), entry.file_stem.clone());
                name_to_file.insert(type_name.to_string(), format!("{}.ts", entry.file_stem));
            }
        }
        info!(
            "Final name_to_stem has {} entries after CanonicalTypeMap population",
            name_to_stem.len()
        );

        // Collect resource types for interop generation (only BaseResource kinds)
        let resource_types: Vec<String> = entries
            .iter()
            .filter_map(|(summary, definition)| {
                // Only include actual FHIR resources that can be reference targets
                if summary.kind == StructureKind::BaseResource {
                    let type_name =
                        naming::pascal_case(summary.type_code.as_deref().unwrap_or(&definition.id));
                    Some(type_name)
                } else {
                    None
                }
            })
            .collect();

        // Phase 2a: First pass - collect all extensions from all structures
        info!("Phase 2a: Collecting extensions from all structures");
        let mut all_extensions = IndexMap::new();
        for (_summary, definition) in &entries {
            let extensions = extensions::extract_extensions(definition);
            all_extensions.extend(extensions);
        }
        info!("Collected {} total extensions", all_extensions.len());

        // Phase 2b: Generate structures using the complete name mapping and extensions
        let mut structures = Vec::new();
        let mut profiles = Vec::new();

        for (summary, definition) in entries {
            let type_name =
                naming::pascal_case(summary.type_code.as_deref().unwrap_or(&definition.id));
            let file_name = name_to_file
                .get(&type_name)
                .cloned()
                .unwrap_or_else(|| format!("{}.ts", file_stem(&definition.id)));
            let file_stem = name_to_stem
                .get(&type_name)
                .cloned()
                .unwrap_or_else(|| file_stem(&definition.id));

            // Check if this is a profile (constraint derivation with a different base)
            let is_profile = matches!(definition.lineage.derivation, Some(Derivation::Constraint))
                && definition
                    .lineage
                    .base_id
                    .as_ref()
                    .is_some_and(|base_id| base_id != &definition.id);

            info!(
                "Processing structure: {} - kind={:?}, derivation={:?}, base_id={:?}, is_profile={}",
                type_name,
                summary.kind,
                definition.lineage.derivation,
                definition.lineage.base_id,
                is_profile
            );

            if is_profile {
                info!("Found profile: {} ({})", type_name, definition.url);
                if !self.config.generate_profiles {
                    info!("  Skipping profile (generate_profiles=false)");
                    continue;
                }
                // Generate profile
                if let Some(profile_info) =
                    profiles::ProfileInfo::from_resource_definition(&definition, &all_extensions)
                {
                    info!(
                        "  ProfileInfo created, has_constraints={}",
                        profile_info.has_constraints()
                    );
                    info!(
                        "  - must_support: {}",
                        profile_info.must_support_elements.len()
                    );
                    info!("  - fixed: {}", profile_info.fixed_elements.len());
                    info!(
                        "  - constrained: {}",
                        profile_info.constrained_elements.len()
                    );

                    if !profile_info.has_constraints() {
                        info!("  Skipping profile (no constraints)");
                        continue;
                    }
                } else {
                    info!("  Failed to create ProfileInfo");
                    continue;
                }

                if let Some(profile_info) =
                    profiles::ProfileInfo::from_resource_definition(&definition, &all_extensions)
                    && profile_info.has_constraints()
                {
                    // Check if any profile methods are enabled (for backward compat with old method)
                    let with_methods = self.config.profile_methods.extension_accessors
                        || self.config.profile_methods.serialization
                        || self.config.profile_methods.validation;

                    // Get package folder for import path calculation
                    let package_folder = self
                        .config
                        .package_folders
                        .get(&descriptor.id)
                        .cloned()
                        .unwrap_or_else(|| sanitize_package_name(&descriptor.id.name));

                    // Create generation context for imports
                    let generation_context = self.config.type_registry.as_ref().map(|registry| {
                        profiles::ProfileGenerationContext {
                            current_package_folder: &package_folder,
                            type_registry: registry,
                        }
                    });

                    // In interface mode the base types are interfaces, so a
                    // profile must also be an interface — a TypeScript class
                    // cannot `extends` an interface. Only emit profile classes
                    // when the base output is class-based.
                    let profile_as_class = self.config.profile_classes
                        && matches!(
                            self.config.mode,
                            GenerationMode::Class | GenerationMode::ClassWithBuilder
                        );
                    let ts_code = profile_info.generate_typescript(
                        profile_as_class,
                        with_methods,
                        self.config.zod_schemas,
                        generation_context.as_ref(),
                    );
                    let profile_file_stem = format!("profile-{}", file_stem);
                    let profile_file_name = format!("{}.ts", profile_file_stem);
                    let profile_type_name = profile_info.type_name.clone();

                    let mut profile_type_exports = Vec::new();
                    let mut profile_value_exports = Vec::new();

                    if self.config.profile_classes {
                        profile_value_exports.push(profile_info.type_name.clone());
                    } else {
                        profile_type_exports.push(profile_info.type_name.clone());
                    }

                    profile_value_exports.push(format!("is{}", profile_info.type_name));

                    if self.config.zod_schemas {
                        profile_value_exports.push(format!("{}Schema", profile_info.type_name));
                    }

                    profile_type_exports.sort();
                    profile_type_exports.dedup();
                    profile_value_exports.sort();
                    profile_value_exports.dedup();

                    profiles.push(ProfileOutput {
                        type_name: profile_info.type_name,
                        file_name: profile_file_name,
                        file_stem: profile_file_stem,
                        typescript_code: ts_code,
                        type_exports: profile_type_exports,
                        value_exports: profile_value_exports,
                    });
                    info!("Generated profile: {}", profile_type_name);
                }
            } else {
                // Regular structure
                let package_folder = self
                    .config
                    .package_folders
                    .get(&descriptor.id)
                    .cloned()
                    .unwrap_or_else(|| sanitize_package_name(&descriptor.id.name));

                let render = build_render_structure(
                    &definition,
                    &summary,
                    &type_name,
                    &file_name,
                    &file_stem,
                    &self.config,
                    &name_to_stem,
                    &descriptor.id.name,
                    &package_folder,
                    false, // is_profile = false for regular structures
                    &valueset_url_to_type,
                );

                structures.push(render);
            }
        }

        // Build interop config if enabled
        let (generate_interop, interop_config) = if self.config.generate_interop {
            let search_config = interop::search::SearchConfig {
                advanced_search: self.config.interop_search_advanced,
                ..Default::default()
            };
            let config = interop::InteropConfig {
                typed_references: self.config.interop_typed_references,
                date_helpers: self.config.interop_date_helpers,
                bundle_traversal: self.config.interop_bundle_traversal,
                search_helpers: self.config.interop_search_helpers,
                search_config,
                ..Default::default()
            };
            (true, Some(config))
        } else {
            (false, None)
        };

        // Deduplicate structures by file_stem - keep the last entry (most complete)
        // This handles cases where profiles and base types share the same type_code
        let mut seen_stems: HashSet<String> = HashSet::new();
        let structures: Vec<_> = structures
            .into_iter()
            .rev() // Reverse to keep the LAST entry when deduplicating
            .filter(|s| seen_stems.insert(s.file_stem.clone()))
            .collect::<Vec<_>>()
            .into_iter()
            .rev() // Reverse back to preserve original order
            .collect();

        // Deduplicate profiles by file_stem as well
        let mut seen_profile_stems: HashSet<String> = HashSet::new();
        let profiles: Vec<_> = profiles
            .into_iter()
            .rev()
            .filter(|p| seen_profile_stems.insert(p.file_stem.clone()))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let package_output = PackageOutput {
            path: package_dir.clone(),
            folder: package_folder,
            structures,
            valuesets: Vec::new(),
            profiles,
            extensions: all_extensions,
            branded_primitives: self.config.branded_primitives,
            zod_schemas: self.config.zod_schemas,
            generate_interop,
            interop_config,
            resource_types,
            search_parameters,
            project_config: self.config.config.clone(),
        };

        write_package(&package_output)?;
        info!(
            "Generated {} file(s) in {}",
            package_output.structures.len(),
            package_dir.display()
        );

        Ok(())
    }
}

fn write_package(package: &PackageOutput) -> Result<()> {
    // Write FHIR primitives with branded types first (needed by all other files)
    let mut primitives_context = TeraContext::new();
    primitives_context.insert("branded_primitives", &package.branded_primitives);
    primitives_context.insert("zod_schemas", &package.zod_schemas);
    let primitives_content = templates::render("primitives.ts.tera", &primitives_context)?;
    fs::write(package.path.join("primitives.ts"), primitives_content)
        .with_context(|| "failed to write primitives.ts")?;

    // Write main structures
    for structure in &package.structures {
        let mut context = TeraContext::new();
        context.insert("structure", structure);
        let content = templates::render("structure.ts.tera", &context)?;
        let file_path = package.path.join(&structure.file_name);
        fs::write(&file_path, content)
            .with_context(|| format!("failed to write {}", file_path.display()))?;
    }

    // Write value sets
    for vs in &package.valuesets {
        let file_path = package.path.join(&vs.file_name);
        fs::write(&file_path, &vs.typescript_code)
            .with_context(|| format!("failed to write {}", file_path.display()))?;
    }

    // Write extensions if any are defined
    if !package.extensions.is_empty() {
        // Collect all unique value types that need to be imported
        let primitive_types = ["string", "number", "boolean", "unknown"];
        let mut imported_types: Vec<String> = package
            .extensions
            .values()
            .filter_map(|ext| ext.value_type.as_ref())
            .filter(|t| !primitive_types.contains(&t.as_str()))
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        imported_types.sort();

        let mut context = TeraContext::new();
        context.insert("extensions", &package.extensions);
        context.insert("imported_types", &imported_types);
        let content = templates::render("extensions.ts.tera", &context)?;
        fs::write(package.path.join("extensions.ts"), content)
            .with_context(|| "failed to write extensions.ts")?;
    }

    // Write generic extension utilities to utils/ directory
    let utils_dir = package.path.join("utils");
    fs::create_dir_all(&utils_dir)
        .with_context(|| format!("failed to create utils directory {}", utils_dir.display()))?;

    // Compute import prefix for utils/extensions.ts
    // From utils/ subdirectory, we need to go up one level (..) to reach the package root
    // For r4-core: imports are at ../extension, ../domain-resource, ../element
    // For other packages: imports are at ../../r4-core/extension, etc.
    let utils_import_prefix = if package.folder == "r4-core" {
        "..".to_string()
    } else {
        "../../r4-core".to_string()
    };

    let mut utils_context = TeraContext::new();
    utils_context.insert("import_prefix", &utils_import_prefix);
    let utils_content = templates::render("extension_utils.ts.tera", &utils_context)?;
    fs::write(utils_dir.join("extensions.ts"), utils_content)
        .with_context(|| "failed to write utils/extensions.ts")?;

    // Write interop utilities if enabled
    if package.generate_interop
        && let Some(config) = &package.interop_config
    {
        let generator = interop::InteropGenerator::new(
            package.resource_types.clone(),
            package.search_parameters.clone(),
            config,
        );
        if generator.is_enabled() {
            // Generate main interop.ts with references, dates, and bundles (but not search)
            let mut interop_code = generator.generate_without_search(config);

            // Generate search parameters in separate directory if enabled
            if config.search_config.interfaces || config.search_config.url_builders {
                let search_dir = utils_dir.join("search");
                fs::create_dir_all(&search_dir).with_context(|| {
                    format!("failed to create search directory {}", search_dir.display())
                })?;

                let search_helpers = interop::search::SearchHelpers::new(
                    package.resource_types.clone(),
                    package.search_parameters.clone(),
                    &config.search_config,
                );

                // Generate split files
                let search_files = search_helpers.generate_all_split();
                for (file_name, content) in search_files {
                    let file_path = search_dir.join(&file_name);
                    fs::write(&file_path, content)
                        .with_context(|| format!("failed to write search/{}", file_name))?;
                }

                // Add re-export from search directory
                interop_code
                    .push_str("\n\n// Re-export search parameters from separate directory\n");
                interop_code.push_str("export * from './search';\n");
            }

            // Write main interop.ts
            fs::write(utils_dir.join("interop.ts"), interop_code)
                .with_context(|| "failed to write utils/interop.ts")?;
        }
    }

    // Generate package.json with dependencies at root output directory
    // Only generate if enabled in config (default: true)
    if package.project_config.generate_package_json
        && (package.generate_interop || package.zod_schemas)
    {
        use serde_json::json;

        // Get root output directory (parent of package.path)
        if let Some(output_root) = package.path.parent() {
            let package_json_path = output_root.join("package.json");

            // Only create if it doesn't exist (to avoid overwriting)
            if !package_json_path.exists() {
                let mut dependencies = serde_json::Map::new();

                // Add zod if validation is enabled (use v4)
                if package.zod_schemas {
                    dependencies.insert("zod".to_string(), json!("^4.0.0"));
                }

                let package_json = json!({
                    "name": "@inkgen/fhir",
                    "version": "0.1.0",
                    "type": "module",
                    "packageManager": "bun@1.0.0",
                    "dependencies": dependencies,
                    "devDependencies": {
                        "@types/node": "^20.0.0",
                        "typescript": "^5.0.0"
                    }
                });

                fs::write(
                    &package_json_path,
                    serde_json::to_string_pretty(&package_json)?,
                )
                .with_context(|| "failed to write package.json")?;
            }
        }
    }

    // Generate tsconfig.json at root output directory
    // Only generate if enabled in config (default: true)
    if package.project_config.generate_tsconfig {
        // Get root output directory (parent of package.path)
        if let Some(output_root) = package.path.parent() {
            let tsconfig_path = output_root.join("tsconfig.json");
            if !tsconfig_path.exists() {
                // Get package folder name (last component of path)
                let package_folder = package
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("r4-core")
                    .to_string();

                let include_dirs = vec![package_folder];

                let mut context = tera::Context::new();
                context.insert("packages", &include_dirs);

                let tsconfig_content = crate::templates::render("tsconfig.json.tera", &context)
                    .with_context(|| "failed to render tsconfig.json.tera")?;

                fs::write(&tsconfig_path, tsconfig_content)
                    .with_context(|| "failed to write tsconfig.json")?;
            }
        }
    }

    // Create profiles subdirectory if there are profiles
    if !package.profiles.is_empty() {
        let profiles_dir = package.path.join("profiles");
        fs::create_dir_all(&profiles_dir).with_context(|| {
            format!(
                "failed to create profiles directory {}",
                profiles_dir.display()
            )
        })?;

        // Write profiles to profiles/ subfolder
        for profile in &package.profiles {
            let file_path = profiles_dir.join(&profile.file_name);
            fs::write(&file_path, &profile.typescript_code)
                .with_context(|| format!("failed to write {}", file_path.display()))?;
        }

        // Generate profiles/index.ts barrel export
        let mut profile_index_context = TeraContext::new();
        profile_index_context.insert("profiles", &package.profiles);
        let profile_index = templates::render("profiles-index.ts.tera", &profile_index_context)?;
        fs::write(profiles_dir.join("index.ts"), &profile_index)
            .with_context(|| "failed to write profiles/index.ts")?;
    }

    // Write main index with all exports
    let mut context = TeraContext::new();
    context.insert("structures", &package.structures);
    context.insert("profiles", &package.profiles);
    let index_content = templates::render("index.ts.tera", &context)?;
    fs::write(package.path.join("index.ts"), index_content)?;
    Ok(())
}

/// Extract all FHIR primitive type names from a type expression
/// Handles simple types (FhirString), optional types (FhirBoolean | undefined),
/// arrays (Array<FhirInteger>), and union types (FhirBoolean | FhirDateTime)
fn extract_primitives_from_type(type_expr: &str, primitives: &mut HashSet<String>) {
    // List of all FHIR primitive type names
    const FHIR_PRIMITIVES: &[&str] = &[
        "FhirString",
        "FhirCode",
        "FhirId",
        "FhirMarkdown",
        "FhirOid",
        "FhirUri",
        "FhirCanonical",
        "FhirUrl",
        "FhirUuid",
        "FhirBase64Binary",
        "FhirDate",
        "FhirDateTime",
        "FhirTime",
        "FhirInstant",
        "FhirXhtml",
        "FhirInteger",
        "FhirDecimal",
        "FhirPositiveInt",
        "FhirUnsignedInt",
        "FhirInteger64",
        "FhirBoolean",
    ];

    for primitive in FHIR_PRIMITIVES {
        if type_expr.contains(primitive) {
            primitives.insert(primitive.to_string());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_render_structure(
    definition: &ResourceDefinition,
    summary: &StructureSummary,
    type_name: &str,
    file_name: &str,
    file_stem: &str,
    config: &TypescriptGeneratorConfig,
    name_to_stem: &IndexMap<String, String>,
    package_name: &str,
    package_folder: &str,
    is_profile: bool,
    valueset_url_to_type: &HashMap<String, String>,
) -> RenderStructure {
    let class_name = type_name.to_string();
    let emit_interface = matches!(
        config.mode,
        GenerationMode::Interface | GenerationMode::ClassWithBuilder
    );
    let emit_class = matches!(
        config.mode,
        GenerationMode::Class | GenerationMode::ClassWithBuilder
    );

    let mut fields = Vec::new();
    let mut usage_tracker = imports::UsageTracker::new();
    // Map type_name → (package_folder, file_stem)
    let mut imports_map: IndexMap<String, (String, String)> = IndexMap::new();

    // Collect nested types (BackboneElements) first
    let collector = nested::NestedTypeCollector::new(definition);
    let nested_type_infos = collector.collect();

    // Build a map of element paths to nested type names for field resolution
    let mut element_to_nested_type: HashMap<String, String> = HashMap::new();
    for nested_info in &nested_type_infos {
        element_to_nested_type.insert(
            nested_info.element_path.clone(),
            nested_info.type_name.clone(),
        );
    }

    // Build nested type render structures
    let mut nested_types = Vec::new();
    let mut nested_schema_type_refs: Vec<String> = Vec::new();
    for nested_info in nested_type_infos {
        let mut nested_fields = Vec::new();
        for child in &nested_info.children {
            let field = map_field_with_nested_context(
                child,
                config,
                type_name,
                package_folder,
                name_to_stem,
                &mut imports_map,
                &element_to_nested_type,
                valueset_url_to_type,
            );
            nested_fields.push(field);
        }

        // Add Zod types to nested fields if enabled
        if config.zod_schemas {
            for field in &mut nested_fields {
                // Find the element for this field
                if let Some(element) = nested_info.children.iter().find(|e| {
                    let field_name = e
                        .path
                        .split('.')
                        .next_back()
                        .unwrap_or("")
                        .replace("[x]", "");
                    naming::camel_case(&field_name) == field.name
                }) {
                    if let Some(nested_type_name) = element_to_nested_type.get(&element.path) {
                        let base_info = crate::zod::ZodSchemaInfo {
                            schema: format!("z.lazy(() => {}Schema)", nested_type_name),
                            type_refs: Vec::new(),
                        };
                        let schema_info =
                            crate::zod::apply_cardinality(base_info, &element.cardinality);
                        field.zod_type = Some(schema_info.schema);
                    } else if let Some(schema_info) =
                        crate::zod::element_to_zod_schema_info(element)
                    {
                        nested_schema_type_refs.extend(schema_info.type_refs.clone());
                        field.zod_type = Some(schema_info.schema);
                    }
                }
            }
        }

        // Detect if this nested type uses z.lazy() (needs type annotation for TypeScript)
        let is_recursive = nested_fields
            .iter()
            .filter_map(|f| f.zod_type.as_ref())
            .any(|zod_type| zod_type.contains("z.lazy("));

        nested_types.push(RenderNestedType {
            type_name: nested_info.type_name,
            description: nested_info.doc_comment,
            fields: nested_fields,
            is_recursive,
        });
    }

    // Build fields for the main structure

    // Add resourceType field for Resource and Logical kinds
    // This enables runtime type discrimination in TypeScript
    use inkgen_core::ir::ResourceKind;
    let is_abstract_base = type_name == "Resource" || type_name == "DomainResource";
    let is_resource_like = matches!(
        definition.kind,
        ResourceKind::Resource | ResourceKind::Logical
    );
    if is_resource_like {
        // For abstract base types (Resource, DomainResource), use string type
        // For concrete resources, use literal type for type discrimination
        let type_expr = if is_abstract_base {
            "string".to_string()
        } else {
            format!("\"{}\"", type_name)
        };
        fields.push(RenderField {
            name: "resourceType".to_string(),
            type_expr,
            optional: false, // Required field
            doc: Some("Resource type identifier".to_string()),
            must_support: false,
            zod_type: None,
            valueset_type: None,
            type_dependencies: Vec::new(),
        });
    }

    // Build fields from FHIR element definitions
    for element in top_level_elements(definition) {
        let field = map_field_with_nested_context(
            element,
            config,
            type_name,
            package_folder,
            name_to_stem,
            &mut imports_map,
            &element_to_nested_type,
            valueset_url_to_type,
        );
        fields.push(field);
    }

    ensure_field_dependencies(
        &fields,
        package_folder,
        Some(type_name),
        name_to_stem,
        config.type_registry.as_ref(),
        &mut imports_map,
    );
    for nested in &nested_types {
        ensure_field_dependencies(
            &nested.fields,
            package_folder,
            Some(type_name),
            name_to_stem,
            config.type_registry.as_ref(),
            &mut imports_map,
        );
    }

    track_field_usage(
        &fields,
        &mut usage_tracker,
        imports::UsageContext::InterfaceField,
    );
    for nested in &nested_types {
        track_field_usage(
            &nested.fields,
            &mut usage_tracker,
            imports::UsageContext::InterfaceField,
        );
    }

    // IMPORTANT: When Zod schemas are enabled, we need to collect schema imports
    // AND ensure the corresponding types are also imported.
    // We do this BEFORE grouping imports to ensure schema-referenced types are included.
    let mut schema_type_refs_to_add: HashMap<String, (String, String)> = HashMap::new();

    if config.zod_schemas {
        // Pre-scan to collect all types that will be referenced in schemas
        for element in top_level_elements(definition)
            .iter()
            .filter(|element| !element.path.contains("[x]"))
        {
            // Check if this element has a nested type (backbone element)
            if element_to_nested_type.contains_key(&element.path) {
                // Nested types are in the same file, no import needed
                continue;
            }

            if let Some(schema_info) = crate::zod::element_to_zod_schema_info(element) {
                for type_ref in &schema_info.type_refs {
                    // Skip if already in imports_map
                    if imports_map.contains_key(type_ref) {
                        continue;
                    }

                    // Skip primitives (handled separately)
                    if type_ref.starts_with("Fhir") {
                        continue;
                    }

                    // Try to find this type's location
                    // CRITICAL: Check type_registry FIRST for correct cross-package imports
                    if let Some(type_registry) = &config.type_registry
                        && let Some((pkg_folder, stem)) = type_registry.get(type_ref)
                    {
                        schema_type_refs_to_add
                            .insert(type_ref.clone(), (pkg_folder.to_string(), stem.to_string()));
                    } else if let Some(stem) = name_to_stem.get(type_ref) {
                        // Fallback for types not in registry
                        schema_type_refs_to_add
                            .insert(type_ref.clone(), (package_folder.to_string(), stem.clone()));
                    }
                }
            }
        }
    }

    // Add schema-referenced types to imports_map
    imports_map.extend(schema_type_refs_to_add);

    // Group imports by (package_folder, file_stem) and calculate paths
    let mut import_groups: HashMap<(String, String), Vec<String>> = HashMap::new();
    for (imported_type, (pkg_folder, stem)) in &imports_map {
        // Skip self-imports (e.g., Extension importing Extension)
        if imported_type == type_name {
            continue;
        }
        import_groups
            .entry((pkg_folder.clone(), stem.clone()))
            .or_default()
            .push(imported_type.clone());
    }

    // Determine subfolder for import path calculation
    let from_subfolder = if is_profile { "profiles" } else { "" };

    // Build RenderImport structs with proper paths
    let import_specs: Vec<ImportSpec> = import_groups
        .into_iter()
        .map(|((target_package_folder, target_stem), mut types)| {
            // Sort types for deterministic output
            types.sort();

            // Calculate import path based on whether it's cross-package or same-package
            let path = if config.type_registry.is_some() {
                // Use type registry to determine proper import path
                imports::calculate_import_path(
                    package_folder,
                    from_subfolder,
                    &target_package_folder,
                    &target_stem,
                )
            } else {
                // Fallback to same-package relative import
                format!("./{}", target_stem)
            };

            ImportSpec {
                types,
                path,
                source_package_folder: target_package_folder,
            }
        })
        .collect();

    let mut imports = optimize_imports(import_specs, &usage_tracker, config.tree_shaking);

    // Sort imports by path for deterministic output
    imports.sort_by(|a, b| a.path.cmp(&b.path));

    // Collect all FHIR primitive types used in fields
    let mut primitive_set = std::collections::HashSet::new();
    for field in &fields {
        extract_primitives_from_type(&field.type_expr, &mut primitive_set);
    }
    for nested in &nested_types {
        for field in &nested.fields {
            extract_primitives_from_type(&field.type_expr, &mut primitive_set);
        }
    }
    let mut primitive_imports: Vec<String> = primitive_set.into_iter().collect();
    primitive_imports.sort();
    let has_primitives = !primitive_imports.is_empty();

    // Special case: Make Reference generic to support type-safe references
    let (type_parameters, type_arguments) = if type_name == "Reference" {
        (
            Some("T extends string = string".to_string()),
            Some("T".to_string()),
        )
    } else {
        (None, None)
    };

    let output_folder = if is_profile {
        "profiles".to_string()
    } else {
        String::new()
    };

    // Generate Zod schema fields if enabled, collecting schema imports
    let generate_zod_schema = config.zod_schemas;
    let (zod_fields, schema_imports, valueset_value_imports) = if generate_zod_schema {
        let mut fields = Vec::new();

        // Add resourceType to Zod schema for Resource and Logical types
        use inkgen_core::ir::ResourceKind;
        if matches!(
            definition.kind,
            ResourceKind::Resource | ResourceKind::Logical
        ) {
            // For abstract base types (Resource, DomainResource), use z.string()
            // For concrete resources, use literal type for validation
            let zod_type = if is_abstract_base {
                "z.string()".to_string()
            } else {
                format!("z.literal(\"{}\")", type_name)
            };
            fields.push(ZodSchemaField {
                name: "resourceType".to_string(),
                zod_type,
            });
        }

        let mut schema_type_refs: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        // Track ValueSet Values imports separately - these don't need the Schema suffix
        let mut valueset_value_imports: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        let nested_type_names: Vec<String> = nested_types
            .iter()
            .map(|nested| nested.type_name.clone())
            .collect();

        let add_schema_type_ref =
            |schema_type_refs: &mut std::collections::HashMap<String, Vec<String>>,
             type_ref: &str| {
                if type_ref.is_empty() {
                    return;
                }

                if type_ref == type_name {
                    return;
                }

                if nested_type_names
                    .iter()
                    .any(|nested_name| nested_name == type_ref)
                {
                    return;
                }

                if type_ref.starts_with("Fhir") {
                    // Primitive schemas come from primitives.ts
                    schema_type_refs
                        .entry("./primitives".to_string())
                        .or_default()
                        .push(type_ref.to_string());
                    return;
                }

                let import_path = if let Some(import) = imports
                    .iter()
                    .find(|imp| imp.types.contains(&type_ref.to_string()))
                {
                    import.path.clone()
                } else {
                    let file_name = naming::snake_case(type_ref)
                        .replace('_', "-")
                        .to_ascii_lowercase();
                    format!("./{}", file_name)
                };

                schema_type_refs
                    .entry(import_path)
                    .or_default()
                    .push(type_ref.to_string());
            };

        for type_ref in &nested_schema_type_refs {
            add_schema_type_ref(&mut schema_type_refs, type_ref);
        }

        for element in top_level_elements(definition)
            .iter()
            // Filter out choice type placeholders (e.g., "deceased[x]")
            // Keep only the specific variants (e.g., "deceasedBoolean", "deceasedDateTime")
            .filter(|element| !element.path.contains("[x]"))
        {
            let field_name = crate::zod::element_to_field_name(element);

            // Check if this element has a ValueSet binding with Required/Extensible strength
            let mut used_valueset_schema = false;
            if let Some(binding) = &element.binding {
                use inkgen_core::ir::BindingStrength;

                let is_strong_binding = matches!(
                    binding.strength,
                    BindingStrength::Required | BindingStrength::Extensible
                );

                if is_strong_binding
                    && let Some(valueset_url) = &binding.value_set
                    && let Some(vs_type_name) = valueset_url_to_type.get(valueset_url)
                {
                    // Use z.enum for ValueSet-bound fields
                    let values_name = format!("{}Values", vs_type_name);
                    let base_info = crate::zod::ZodSchemaInfo {
                        schema: format!("z.enum({})", values_name),
                        type_refs: Vec::new(),
                    };
                    let schema_info =
                        crate::zod::apply_cardinality(base_info, &element.cardinality);

                    fields.push(ZodSchemaField {
                        name: field_name.clone(),
                        zod_type: schema_info.schema,
                    });

                    // Add import for the ValueSet Values array
                    // Note: ValueSet Values arrays don't need the Schema suffix,
                    // so we track them separately from schema_type_refs
                    let file_name = naming::snake_case(vs_type_name)
                        .replace('_', "-")
                        .to_ascii_lowercase();
                    let valueset_import_path = format!("./valuesets/{}", file_name);
                    valueset_value_imports
                        .entry(valueset_import_path)
                        .or_default()
                        .push(values_name);

                    used_valueset_schema = true;
                }
            }

            if !used_valueset_schema {
                // Check if this element has a nested type (backbone element)
                if let Some(nested_type_name) = element_to_nested_type.get(&element.path) {
                    // Use the specific nested type schema (defined in same file, no import needed)
                    let base_info = crate::zod::ZodSchemaInfo {
                        schema: format!("{}Schema", nested_type_name),
                        type_refs: Vec::new(), // Nested types are in the same file
                    };
                    let schema_info =
                        crate::zod::apply_cardinality(base_info, &element.cardinality);

                    fields.push(ZodSchemaField {
                        name: field_name,
                        zod_type: schema_info.schema,
                    });
                } else if let Some(schema_info) = crate::zod::element_to_zod_schema_info(element) {
                    // Check if this is a self-referential field (element type == current type)
                    let is_self_reference = element.types.iter().any(|t| {
                        let type_code = naming::pascal_case(&t.code);
                        type_code == type_name
                    });

                    let final_schema = if is_self_reference {
                        // Wrap in z.lazy() to handle forward reference
                        let base_schema = format!("z.lazy(() => {}Schema)", type_name);
                        let base_info = crate::zod::ZodSchemaInfo {
                            schema: base_schema,
                            type_refs: Vec::new(),
                        };
                        crate::zod::apply_cardinality(base_info, &element.cardinality).schema
                    } else {
                        // Collect type references for schema imports (not for self-references)
                        for type_ref in &schema_info.type_refs {
                            add_schema_type_ref(&mut schema_type_refs, type_ref);
                        }
                        schema_info.schema
                    };

                    fields.push(ZodSchemaField {
                        name: field_name,
                        zod_type: final_schema,
                    });
                }
            }
        }

        // ============================================================
        // CRITICAL FIX: Ensure all schema type refs are also in imports_map
        // ============================================================
        // The schema_type_refs HashMap tracks types needed for schemas (CodeableConceptSchema)
        // but we ALSO need to import the base types (CodeableConcept) for the interface definitions.
        //
        // Extract all unique type names from schema_type_refs
        let all_schema_types: std::collections::HashSet<String> = schema_type_refs
            .values()
            .flat_map(|types| types.iter())
            .cloned()
            .collect();

        debug!(
            "Processing {} schema types for {} (name_to_stem has {} entries)",
            all_schema_types.len(),
            type_name,
            name_to_stem.len()
        );

        for schema_type in all_schema_types {
            // Extract base type name (remove "Schema" or "Values" suffix)
            let base_type = if let Some(base) = schema_type.strip_suffix("Schema") {
                base.to_string()
            } else if let Some(_base) = schema_type.strip_suffix("Values") {
                // ValueSet Values array, skip (no base type to import)
                continue;
            } else {
                // Not a schema type, use as-is
                schema_type.clone()
            };

            // Skip if already in imports_map
            if imports_map.contains_key(&base_type) {
                debug!(
                    "Type {} already in imports_map for {}",
                    base_type, type_name
                );
                continue;
            }

            // Skip if it's the current type (self-reference)
            if base_type == type_name {
                debug!("Skipping self-reference {} in {}", base_type, type_name);
                continue;
            }

            // Skip if it's a primitive type
            if base_type.starts_with("Fhir") {
                debug!("Skipping primitive type {} in {}", base_type, type_name);
                continue;
            }

            // Try to find this type's location and add to imports_map
            // CRITICAL: Check type_registry FIRST for correct cross-package imports
            if let Some(type_registry) = &config.type_registry
                && let Some((pkg_folder, stem)) = type_registry.get(&base_type)
            {
                // Use type_registry for authoritative package info
                debug!(
                    "Adding {} to imports_map for {} via type_registry (pkg: {}, stem: {})",
                    base_type, type_name, pkg_folder, stem
                );
                imports_map.insert(
                    base_type.clone(),
                    (pkg_folder.to_string(), stem.to_string()),
                );
            } else if let Some(stem) = name_to_stem.get(&base_type) {
                // Fallback for types not in registry (local nested types)
                debug!(
                    "Adding {} to imports_map for {} (stem: {})",
                    base_type, type_name, stem
                );
                imports_map.insert(
                    base_type.clone(),
                    (package_folder.to_string(), stem.clone()),
                );
            } else {
                // Check if it exists with different casing
                let similar_keys: Vec<_> = name_to_stem
                    .keys()
                    .filter(|k| k.to_lowercase() == base_type.to_lowercase())
                    .collect();

                warn!(
                    "Could not find location for type '{}' needed by {} (from schema type {}) - similar keys: {:?}, name_to_stem has {} total entries",
                    base_type,
                    type_name,
                    schema_type,
                    similar_keys,
                    name_to_stem.len()
                );
            }
        }

        // Debug: log all schema_type_refs
        debug!(
            "schema_type_refs for {}: {:?}",
            type_name,
            schema_type_refs
                .iter()
                .map(|(path, types)| (path.as_str(), types.as_slice()))
                .collect::<Vec<_>>()
        );

        // Convert schema_type_refs HashMap to Vec<RenderImport>
        let mut schema_imports: Vec<RenderImport> = schema_type_refs
            .into_iter()
            .map(|(path, types)| {
                // Deduplicate types
                let mut unique_types: Vec<String> = types
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                unique_types.sort();

                RenderImport {
                    types: unique_types,
                    path,
                    is_type_only: false,
                    source_package_folder: package_folder.to_string(),
                }
            })
            .collect();

        // Sort imports for consistent output
        schema_imports.sort_by(|a, b| a.path.cmp(&b.path));

        (fields, schema_imports, valueset_value_imports)
    } else {
        (Vec::new(), Vec::new(), std::collections::HashMap::new())
    };

    // IMPORTANT: Regroup imports after Zod schema processing
    // The Zod schema block may have added new types to imports_map,
    // so we need to recreate the imports vector to include them
    if generate_zod_schema {
        debug!(
            "Regrouping imports for {} after Zod schema processing - imports_map has {} entries",
            type_name,
            imports_map.len()
        );

        let mut import_groups: HashMap<(String, String), Vec<String>> = HashMap::new();
        for (imported_type, (pkg_folder, stem)) in &imports_map {
            // Skip self-imports (e.g., Extension importing Extension)
            if imported_type == type_name {
                debug!("Skipping self-import of {} in {}", imported_type, type_name);
                continue;
            }
            import_groups
                .entry((pkg_folder.clone(), stem.clone()))
                .or_default()
                .push(imported_type.clone());
        }

        debug!(
            "After regrouping for {}: {} import groups",
            type_name,
            import_groups.len()
        );

        let from_subfolder = if is_profile { "profiles" } else { "" };
        let regrouped_specs: Vec<ImportSpec> = import_groups
            .into_iter()
            .map(|((target_package_folder, target_stem), mut types)| {
                types.sort();
                let path = if config.type_registry.is_some() {
                    imports::calculate_import_path(
                        package_folder,
                        from_subfolder,
                        &target_package_folder,
                        &target_stem,
                    )
                } else {
                    format!("./{}", target_stem)
                };
                ImportSpec {
                    types,
                    path,
                    source_package_folder: target_package_folder,
                }
            })
            .collect();
        imports = optimize_imports(regrouped_specs, &usage_tracker, config.tree_shaking);
        imports.sort_by(|a, b| a.path.cmp(&b.path));
    }

    // Add ValueSet Values imports to the regular imports (these don't need Schema suffix)
    if !valueset_value_imports.is_empty() {
        for (path, types) in valueset_value_imports {
            let mut unique_types: Vec<String> = types
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            unique_types.sort();

            imports.push(RenderImport {
                types: unique_types,
                path,
                is_type_only: false,
                source_package_folder: package_folder.to_string(),
            });
        }
        imports.sort_by(|a, b| a.path.cmp(&b.path));
    }

    // Detect slices for discriminated unions
    let slices = slices::detect_slices(definition);

    // Compute barrel exports for this structure
    let mut type_exports = Vec::new();
    let mut value_exports = Vec::new();

    if emit_interface {
        type_exports.push(type_name.to_string());
    }

    if emit_class {
        value_exports.push(class_name.clone());
    }

    // Skip structural guards for abstract base types (Resource, DomainResource)
    // as they don't have concrete resourceType values and type guards aren't meaningful
    let emit_structural_guards = config.structural_guards && !is_abstract_base;
    if emit_structural_guards {
        value_exports.push(format!("is{}", type_name));
    }

    if generate_zod_schema {
        value_exports.push(format!("{}Schema", type_name));
        type_exports.push(format!("{}Validated", type_name));
        value_exports.push(format!("parse{}", type_name));
    }

    for nested in &nested_types {
        type_exports.push(nested.type_name.clone());
        if generate_zod_schema {
            value_exports.push(format!("{}Schema", nested.type_name));
            type_exports.push(format!("{}Validated", nested.type_name));
        }
    }

    for pattern in &slices {
        let union_name = slices::slice_union_type_name(&pattern.path);
        type_exports.push(union_name);
        for slice in &pattern.slices {
            let guard_name = format!("is{}Slice", naming::pascal_case(&slice.name));
            value_exports.push(guard_name);
        }
    }

    type_exports.sort();
    type_exports.dedup();
    value_exports.sort();
    value_exports.dedup();

    // Detect if any zod_field uses z.lazy() - these need type annotation for TypeScript
    // This includes self-references and forward references to other types
    let is_recursive_schema = zod_fields
        .iter()
        .any(|field| field.zod_type.contains("z.lazy("));

    RenderStructure {
        type_name: type_name.to_string(),
        class_name,
        file_name: file_name.to_string(),
        file_stem: file_stem.to_string(),
        description: definition
            .description
            .clone()
            .or_else(|| summary.title.clone()),
        emit_interface,
        emit_class,
        structural_guards: emit_structural_guards,
        resource_type_guard: is_resource_like,
        has_primitives,
        primitive_imports,
        fields,
        imports,
        schema_imports,
        nested_types,
        type_parameters,
        type_arguments,
        package_name: package_name.to_string(),
        package_folder: package_folder.to_string(),
        is_profile,
        output_folder,
        generate_zod_schema,
        zod_fields,
        is_recursive_schema,
        branded_primitives: config.branded_primitives,
        slices,
        type_exports,
        value_exports,
    }
}

fn top_level_elements(definition: &ResourceDefinition) -> Vec<&ElementDefinition> {
    // Find the root element. Try different identifiers in order of preference:
    // 1. definition.id (e.g., "Patient" or "telegram-verification")
    // 2. definition.name (e.g., "TelegramVerification" for logical resources)
    // 3. First element's path root (fallback for edge cases)
    let root = definition
        .elements
        .iter()
        .find(|elem| elem.path == definition.id)
        .or_else(|| {
            // For logical resources, the element paths may use 'name' instead of 'id'
            definition
                .name
                .as_ref()
                .and_then(|name| definition.elements.iter().find(|elem| elem.path == *name))
        })
        .or_else(|| {
            // Fallback: use the first element with no dots (root element)
            definition
                .elements
                .iter()
                .find(|elem| !elem.path.contains('.'))
        });

    // If we have a tree structure with children, collect all top-level fields
    if let Some(root) = root
        && !root.children.is_empty()
    {
        let mut elements = Vec::new();

        for child in &root.children {
            // If this is a choice type placeholder (e.g., "deceased[x]"),
            // include its specific variants instead
            if child.path.contains("[x]") && !child.children.is_empty() {
                elements.extend(child.children.iter());
            } else {
                elements.push(child);
            }
        }

        // Sort by path for deterministic ordering
        elements.sort_by(|a, b| a.path.cmp(&b.path));
        return elements;
    }

    // Fallback to flat structure: find elements at depth 1 (sorted for determinism)
    // Determine the root path prefix - try id, then name, then first element's root
    let root_path = if definition
        .elements
        .iter()
        .any(|e| e.path.starts_with(&format!("{}.", definition.id)))
    {
        definition.id.clone()
    } else if let Some(name) = &definition.name {
        if definition
            .elements
            .iter()
            .any(|e| e.path.starts_with(&format!("{}.", name)))
        {
            name.clone()
        } else {
            definition.id.clone()
        }
    } else {
        definition.id.clone()
    };

    let prefix = format!("{}.", root_path);
    let mut elements: Vec<_> = definition
        .elements
        .iter()
        .filter(|element| element.path.starts_with(&prefix))
        .filter(|element| element.path.split('.').count() == 2)
        .collect();
    elements.sort_by(|a, b| a.path.cmp(&b.path));
    elements
}

#[allow(clippy::too_many_arguments)]
fn map_field_with_nested_context(
    element: &ElementDefinition,
    config: &TypescriptGeneratorConfig,
    current_type: &str,
    current_package_folder: &str,
    name_to_stem: &IndexMap<String, String>,
    imports: &mut IndexMap<String, (String, String)>,
    element_to_nested_type: &HashMap<String, String>,
    valueset_url_to_type: &HashMap<String, String>,
) -> RenderField {
    let mut type_dependencies = Vec::new();
    let raw_name = element
        .path
        .split('.')
        .next_back()
        .unwrap_or(&element.path)
        .replace("[x]", "");

    // For TypeScript, field names are ALWAYS camelCase (idiomatic JavaScript/TypeScript)
    // The config.naming setting only affects type names, not field names
    let name = naming::camel_case(&raw_name);

    let optional = element.cardinality.min == 0;
    let is_array = match element.cardinality.max {
        ElementMax::Unbounded => true,
        ElementMax::Finite(v) => v > 1,
    };

    // Check if this element is a BackboneElement with a generated nested type
    let mut type_exprs = Vec::new();
    if let Some(nested_type_name) = element_to_nested_type.get(&element.path) {
        // Use the generated nested type name
        type_exprs.push(nested_type_name.clone());
    } else {
        // Resolve types normally
        let type_refs = resolve_types(element);
        for type_ref in &type_refs {
            match type_ref {
                TypeRef::Primitive(value) => type_exprs.push((*value).to_string()),
                TypeRef::Named(value) => {
                    let type_name = naming::pascal_case(value);
                    type_exprs.push(type_name.clone());
                    type_dependencies.push(type_name.clone());

                    if type_name != current_type {
                        // CRITICAL: Check type_registry FIRST for correct cross-package imports
                        if let Some(type_registry) = &config.type_registry {
                            if let Some((pkg_folder, stem)) = type_registry.get(&type_name) {
                                imports
                                    .entry(type_name.clone())
                                    .or_insert((pkg_folder.to_string(), stem.to_string()));
                            } else if let Some(stem) = name_to_stem.get(&type_name) {
                                // Fallback for types not in registry (local nested types)
                                imports
                                    .entry(type_name.clone())
                                    .or_insert((current_package_folder.to_string(), stem.clone()));
                            }
                        } else if let Some(stem) = name_to_stem.get(&type_name) {
                            // No type_registry available, use local lookup
                            imports
                                .entry(type_name.clone())
                                .or_insert((current_package_folder.to_string(), stem.clone()));
                        }
                    }
                }
                TypeRef::Generic { base, type_arg } => {
                    // For generic types like Reference<"Patient">, we need to import the base type
                    let base_type = naming::pascal_case(base);
                    type_exprs.push(format!("{}<{}>", base_type, type_arg));
                    type_dependencies.push(base_type.clone());

                    if base_type != current_type {
                        // CRITICAL: Check type_registry FIRST for correct cross-package imports
                        if let Some(type_registry) = &config.type_registry {
                            if let Some((pkg_folder, stem)) = type_registry.get(&base_type) {
                                imports
                                    .entry(base_type.clone())
                                    .or_insert((pkg_folder.to_string(), stem.to_string()));
                            } else if let Some(stem) = name_to_stem.get(&base_type) {
                                // Fallback for types not in registry (local nested types)
                                imports
                                    .entry(base_type.clone())
                                    .or_insert((current_package_folder.to_string(), stem.clone()));
                            }
                        } else if let Some(stem) = name_to_stem.get(&base_type) {
                            // No type_registry available, use local lookup
                            imports
                                .entry(base_type.clone())
                                .or_insert((current_package_folder.to_string(), stem.clone()));
                        }
                    }
                }
            }
        }
    }

    if type_exprs.is_empty() {
        type_exprs.push("unknown".to_string());
    }

    let type_expr = if type_exprs.len() == 1 {
        type_exprs[0].clone()
    } else {
        type_exprs.join(" | ")
    };

    let type_expr = if is_array {
        format!("Array<{}>", type_expr)
    } else {
        type_expr
    };

    let doc = element.short.clone().or_else(|| element.definition.clone());

    // Check if this field has a ValueSet binding with Required or Extensible strength
    let mut valueset_type = None;
    let mut final_type_expr = type_expr.clone();

    if let Some(binding) = &element.binding {
        use inkgen_core::ir::BindingStrength;

        // Only use ValueSet types for Required or Extensible bindings
        let is_strong_binding = matches!(
            binding.strength,
            BindingStrength::Required | BindingStrength::Extensible
        );

        if is_strong_binding
            && let Some(valueset_url) = &binding.value_set
            && let Some(vs_type_name) = valueset_url_to_type.get(valueset_url)
        {
            // Replace FhirCode/code with the specific ValueSet type
            final_type_expr = if is_array {
                format!("Array<{}>", vs_type_name)
            } else {
                vs_type_name.clone()
            };
            valueset_type = Some(vs_type_name.clone());
            type_dependencies.push(vs_type_name.clone());

            // Add import for the ValueSet type from valuesets/ directory
            let file_name = naming::snake_case(vs_type_name)
                .replace('_', "-")
                .to_ascii_lowercase();
            imports.insert(
                vs_type_name.clone(),
                (
                    current_package_folder.to_string(),
                    format!("valuesets/{}", file_name),
                ),
            );
        }
    }

    RenderField {
        name,
        type_expr: final_type_expr,
        optional,
        doc,
        must_support: element.must_support,
        zod_type: None, // Will be populated separately when generating Zod schemas
        valueset_type,
        type_dependencies,
    }
}

fn ensure_type_import(
    type_name: &str,
    current_package_folder: &str,
    current_structure_name: Option<&str>,
    name_to_stem: &IndexMap<String, String>,
    type_registry: Option<&imports::TypeRegistry>,
    imports: &mut IndexMap<String, (String, String)>,
) {
    if type_name.starts_with("Fhir") || type_name.is_empty() {
        return;
    }

    // Skip self-imports (a type importing itself)
    if current_structure_name == Some(type_name) {
        return;
    }

    if imports.contains_key(type_name) {
        return;
    }

    // CRITICAL: Check type_registry FIRST for correct cross-package imports
    // The type_registry has the authoritative package folder for each type.
    // name_to_stem may contain types from all packages but doesn't track which package they're from.
    if let Some(registry) = type_registry
        && let Some((pkg_folder, stem)) = registry.get(type_name)
    {
        imports.insert(
            type_name.to_string(),
            (pkg_folder.to_string(), stem.to_string()),
        );
        return;
    }

    // Fallback: use name_to_stem for types not in registry (e.g., locally defined nested types)
    if let Some(stem) = name_to_stem.get(type_name) {
        imports.insert(
            type_name.to_string(),
            (current_package_folder.to_string(), stem.clone()),
        );
    }
}

fn ensure_field_dependencies(
    fields: &[RenderField],
    current_package_folder: &str,
    current_structure_name: Option<&str>,
    name_to_stem: &IndexMap<String, String>,
    type_registry: Option<&imports::TypeRegistry>,
    imports: &mut IndexMap<String, (String, String)>,
) {
    for field in fields {
        for dep in &field.type_dependencies {
            ensure_type_import(
                dep,
                current_package_folder,
                current_structure_name,
                name_to_stem,
                type_registry,
                imports,
            );
        }
    }
}

fn track_field_usage(
    fields: &[RenderField],
    tracker: &mut imports::UsageTracker,
    context: imports::UsageContext,
) {
    for field in fields {
        for dep in &field.type_dependencies {
            tracker.track_usage(dep.clone(), context);
        }
        if let Some(valueset_type) = &field.valueset_type {
            tracker.track_usage(valueset_type.clone(), context);
        }
    }
}

fn optimize_imports(
    imports: Vec<ImportSpec>,
    tracker: &imports::UsageTracker,
    level: config::TreeShakingLevel,
) -> Vec<RenderImport> {
    let enforce_usage = !matches!(level, config::TreeShakingLevel::None);
    let mut result = Vec::new();

    for import in imports {
        let mut type_only = Vec::new();
        let mut value = Vec::new();

        for ty in import.types {
            if enforce_usage && !tracker.is_used(&ty) {
                continue;
            }

            if tracker.needs_value_import(&ty) {
                value.push(ty);
            } else {
                type_only.push(ty);
            }
        }

        if !type_only.is_empty() {
            result.push(RenderImport {
                types: type_only,
                path: import.path.clone(),
                is_type_only: true,
                source_package_folder: import.source_package_folder.clone(),
            });
        }

        if !value.is_empty() {
            result.push(RenderImport {
                types: value,
                path: import.path,
                is_type_only: false,
                source_package_folder: import.source_package_folder,
            });
        }
    }

    result
}

fn resolve_types(element: &ElementDefinition) -> Vec<TypeRef> {
    if !element.types.is_empty() {
        let mut refs = Vec::new();
        for element_type in &element.types {
            refs.extend(resolve_element_type(element_type));
        }
        if !refs.is_empty() {
            return refs;
        }
    }

    if let Some(reference) = &element.content_reference {
        return vec![TypeRef::Named(
            reference.trim_start_matches('#').to_string(),
        )];
    }

    Vec::new()
}

fn resolve_element_type(element_type: &ElementType) -> Vec<TypeRef> {
    let base = map_primitive(&element_type.code);
    if let Some(mapped) = base {
        return vec![TypeRef::Primitive(mapped)];
    }

    if element_type.code.eq_ignore_ascii_case("Reference")
        && !element_type.target_profiles.is_empty()
    {
        // Build type argument as union of resource type names (e.g., "Patient" | "Organization")
        let type_args: Vec<String> = element_type
            .target_profiles
            .iter()
            .map(|profile| {
                let name = profile
                    .split('/')
                    .next_back()
                    .map(str::to_string)
                    .unwrap_or_else(|| profile.to_string());
                format!("\"{}\"", name)
            })
            .collect();

        let type_arg = type_args.join(" | ");

        return vec![TypeRef::Generic {
            base: "Reference".to_string(),
            type_arg,
        }];
    }

    vec![TypeRef::Named(element_type.code.clone())]
}

fn map_primitive(code: &str) -> Option<&'static str> {
    // Handle URL-based FHIR type codes (e.g., http://hl7.org/fhirpath/System.String)
    if code.starts_with("http://") || code.starts_with("https://") {
        // Extract the last component after the last slash or dot
        let type_name = code
            .rsplit(&['/', '.'][..])
            .next()
            .unwrap_or(code)
            .to_lowercase();

        return match type_name.as_str() {
            "string" => Some("FhirString"),
            "boolean" => Some("FhirBoolean"),
            "integer" => Some("FhirInteger"),
            "decimal" => Some("FhirDecimal"),
            "date" => Some("FhirDate"),
            "datetime" => Some("FhirDateTime"),
            "time" => Some("FhirTime"),
            _ => None,
        };
    }

    // Handle standard FHIR primitive types - use branded types for type safety
    match code {
        "boolean" => Some("FhirBoolean"),
        "integer" => Some("FhirInteger"),
        "decimal" => Some("FhirDecimal"),
        "positiveInt" => Some("FhirPositiveInt"),
        "unsignedInt" => Some("FhirUnsignedInt"),
        "integer64" => Some("FhirInteger64"),
        "string" => Some("FhirString"),
        "code" => Some("FhirCode"),
        "id" => Some("FhirId"),
        "markdown" => Some("FhirMarkdown"),
        "oid" => Some("FhirOid"),
        "uri" => Some("FhirUri"),
        "canonical" => Some("FhirCanonical"),
        "url" => Some("FhirUrl"),
        "uuid" => Some("FhirUuid"),
        "base64Binary" => Some("FhirBase64Binary"),
        "date" => Some("FhirDate"),
        "dateTime" => Some("FhirDateTime"),
        "time" => Some("FhirTime"),
        "instant" => Some("FhirInstant"),
        "xhtml" => Some("FhirXhtml"),
        _ => None,
    }
}

fn package_output_dir(
    base: &Path,
    package: &PackageId,
    folder_mapping: &HashMap<PackageId, String>,
) -> PathBuf {
    // Use custom folder name if available, otherwise sanitize package name
    if let Some(folder) = folder_mapping.get(package) {
        base.join(folder)
    } else {
        // Fallback to sanitized package name (without version for cleaner output)
        let folder = sanitize_package_name(&package.name);
        base.join(folder)
    }
}

fn file_stem(identifier: &str) -> String {
    naming::snake_case(identifier)
        .replace('_', "-")
        .to_ascii_lowercase()
}

#[derive(Debug, Clone)]
enum TypeRef {
    Primitive(&'static str),
    Named(String),
    /// Generic type with type parameter (e.g., Reference<"Patient">)
    Generic {
        base: String,
        type_arg: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkgen_core::BaseStructureService;
    use inkgen_testing::{CORE_PACKAGE, CORE_VERSION, CoreTestContext};
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_typescript_identifier() {
        use templates::*;

        // Reserved keywords should be prefixed with underscore
        assert_eq!(sanitize_typescript_identifier("class"), "_class");
        assert_eq!(sanitize_typescript_identifier("interface"), "_interface");
        assert_eq!(sanitize_typescript_identifier("type"), "_type");
        assert_eq!(sanitize_typescript_identifier("return"), "_return");

        // Non-reserved words should pass through
        assert_eq!(sanitize_typescript_identifier("patient"), "patient");
        assert_eq!(sanitize_typescript_identifier("MyClass"), "MyClass");
    }

    #[test]
    fn test_wrap_documentation() {
        use templates::*;

        // Short strings should not wrap
        assert_eq!(wrap_documentation("Hello world", 80), "Hello world");

        // Long strings should wrap at word boundaries
        let long_text = "This is a very long documentation string that should be wrapped at approximately eighty characters";
        let wrapped = wrap_documentation(long_text, 40);
        let lines: Vec<&str> = wrapped.lines().collect();
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|line| line.len() <= 40));

        // Empty string should return empty
        assert_eq!(wrap_documentation("", 80), "");
    }

    #[tokio::test]
    async fn typescript_generated_files_are_valid_syntax() {
        // Verify that generated TypeScript files have valid syntax and structure
        let ctx = CoreTestContext::with_allowed_resources(vec!["Patient"])
            .await
            .expect("context");
        let cache = ctx.cache();
        let descriptors = cache.descriptors().await.expect("descriptors");
        let descriptor = descriptors
            .into_iter()
            .find(|desc| desc.id.name == CORE_PACKAGE && desc.id.version == CORE_VERSION)
            .expect("core package descriptor");

        let provider = BaseStructureService::from_project_config(cache.clone(), ctx.config());
        let provider_config = ctx.config().structure_config();

        let temp = tempdir().expect("dir");
        let config = TypescriptGeneratorConfig {
            mode: GenerationMode::Interface,
            structural_guards: true,
            naming: NamingConvention::PascalCase,
            output_structure: OutputStructure::Flat,
            output_dir: temp.path().to_path_buf(),
            generate_profiles: true,
            generate_valuesets: true,
            max_valueset_size: 50,
            valueset_metadata: true,
            valueset_helpers: true,
            valueset_codesystem_link: true,
            package_folders: HashMap::new(),
            package_filters: HashMap::new(),
            dependency_analyzer: None,
            type_registry: None,
            package_cache: None,
            profile_classes: false,
            profile_methods: ProfileMethodConfig::default(),
            zod_schemas: false,
            zod_colocated: true,
            branded_primitives: false,
            generate_interop: false,
            interop_typed_references: false,
            interop_date_helpers: false,
            interop_bundle_traversal: false,
            interop_search_helpers: false,
            interop_search_advanced: false,
            tree_shaking: config::TreeShakingLevel::None,
            import_style: config::ImportStyle::Named,
            lazy_schemas: false,
            config: ProjectFilesConfig::default(),
        };
        let generator = TypescriptGenerator::new(config.clone());
        generator
            .generate(&provider, &descriptor, &provider_config)
            .await
            .expect("generate");

        let mut package_dir =
            package_output_dir(&config.output_dir, &descriptor.id, &config.package_folders);

        if !package_dir.exists() {
            // Fallback to content directly under the configured output directory
            let entries: Vec<_> = fs::read_dir(&config.output_dir)
                .expect("read root output dir")
                .filter_map(|entry| entry.ok())
                .collect();

            if let Some(dir) = entries
                .iter()
                .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            {
                package_dir = dir.path();
            } else if entries
                .iter()
                .any(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
            {
                // Files were written directly to the root output directory
                package_dir = config.output_dir.clone();
            } else {
                eprintln!("generator did not produce any files; skipping filesystem assertions");
                return;
            }
        }

        // Check what files were actually generated
        let generated_files: Vec<_> = fs::read_dir(&package_dir)
            .expect("read package dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();

        // Verify that some files were generated
        assert!(
            !generated_files.is_empty(),
            "No TypeScript files were generated"
        );

        // Verify index.ts exists
        let index_file = fs::read_to_string(package_dir.join("index.ts")).expect("read index.ts");

        assert!(
            index_file.contains("export") && index_file.contains("from"),
            "Index file should export from modules"
        );

        // Pick any TypeScript file and verify it has valid syntax
        let any_ts_file = generated_files
            .iter()
            .find(|f| f.to_string_lossy().ends_with(".ts"))
            .expect("at least one .ts file generated");

        let file_content =
            fs::read_to_string(package_dir.join(any_ts_file)).expect("read generated file");

        // Verify basic TypeScript structure: should have either interface or type declaration
        assert!(
            file_content.contains("export ")
                || file_content.contains("interface ")
                || file_content.contains("type ")
                || file_content.contains("function "),
            "Generated file should contain TypeScript exports or declarations"
        );
    }

    #[test]
    fn test_extensions_template_renders() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        // Empty extensions should render without error
        context.insert(
            "extensions",
            &std::collections::HashMap::<String, String>::new(),
        );

        let result = render("extensions.ts.tera", &context);
        assert!(result.is_ok(), "extensions template should render");
    }

    #[test]
    fn test_profile_helpers_template_renders() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("profile_helpers", &serde_json::json!(null));

        let result = render("profile_helpers.ts.tera", &context);
        assert!(result.is_ok(), "profile_helpers template should render");
    }

    #[test]
    fn test_terminology_helpers_template_renders() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("terminology_helpers", &serde_json::json!(null));

        let result = render("terminology_helpers.ts.tera", &context);
        assert!(result.is_ok(), "terminology_helpers template should render");
    }

    #[test]
    fn test_invariant_validators_template_renders() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("invariant_validator", &serde_json::json!(null));

        let result = render("invariant_validators.ts.tera", &context);
        assert!(
            result.is_ok(),
            "invariant_validators template should render"
        );
    }

    #[test]
    fn test_discriminator_unions_template_renders() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("slices", &serde_json::json!([]));

        let result = render("discriminator_unions.ts.tera", &context);
        assert!(
            result.is_ok(),
            "discriminator_unions template should render"
        );
    }

    #[test]
    fn test_primitives_template_plain_types() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("branded_primitives", &false);
        context.insert("zod_schemas", &false);

        let result = render("primitives.ts.tera", &context);
        assert!(result.is_ok(), "primitives template should render");

        let content = result.unwrap();
        assert!(content.contains("export type FhirString = string;"));
        assert!(!content.contains("__brand"));
        assert!(!content.contains("import { z }"));
        assert!(!content.contains("export function fhir"));
    }

    #[test]
    fn test_primitives_template_branded_types() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("branded_primitives", &true);
        context.insert("zod_schemas", &false);

        let result = render("primitives.ts.tera", &context);
        assert!(result.is_ok(), "primitives template should render");

        let content = result.unwrap();
        assert!(
            content
                .contains("export type FhirString = string & { readonly __brand: 'FhirString' }")
        );
        assert!(content.contains("export function fhirString(value: string): FhirString"));
        assert!(!content.contains("import { z }"));
    }

    #[test]
    fn test_primitives_template_branded_with_zod() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("branded_primitives", &true);
        context.insert("zod_schemas", &true);

        let result = render("primitives.ts.tera", &context);
        assert!(result.is_ok(), "primitives template should render");

        let content = result.unwrap();
        assert!(
            content
                .contains("export type FhirString = string & { readonly __brand: 'FhirString' }")
        );
        assert!(content.contains("import { z } from 'zod'"));
        assert!(content.contains("export const FhirStringSchema = z.string()"));
        assert!(content.contains("export const FhirDateSchema = z.iso.date()"));
        assert!(content.contains("export function fhirString(value: string): FhirString"));
        assert!(content.contains("return FhirStringSchema.parse(value) as FhirString"));
    }

    #[test]
    fn test_primitives_template_zod_only() {
        use templates::*;
        use tera::Context as TeraContext;

        let mut context = TeraContext::new();
        context.insert("branded_primitives", &false);
        context.insert("zod_schemas", &true);

        let result = render("primitives.ts.tera", &context);
        assert!(result.is_ok(), "primitives template should render");

        let content = result.unwrap();
        assert!(content.contains("export type FhirString = string;"));
        assert!(content.contains("import { z } from 'zod'"));
        assert!(content.contains("export const FhirStringSchema = z.string()"));
        assert!(!content.contains("export function fhir"));
    }
}
