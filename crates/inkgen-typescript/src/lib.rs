use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use indexmap::IndexMap;
use inkgen_core::config::{FilterMode, PackageEntry, ProfileMethodConfig, sanitize_package_name};
use inkgen_core::ir::{Derivation, ElementDefinition, ElementMax, ElementType, ResourceDefinition};
use inkgen_core::{
    DependencyAnalyzer, LanguageBackend, LanguageGenerator, PackageCache, PackageDescriptor,
    PackageId, StructureDefinitionProvider, StructureFilter, StructureKind,
    StructureProviderConfig, StructureSummary, TypescriptLanguageConfig,
};
use once_cell::sync::Lazy;
use serde::Serialize;
use tera::{Context as TeraContext, Tera};
use tracing::{debug, info, warn};

pub use config::{GenerationMode, NamingConvention, OutputStructure, TypescriptGeneratorConfig};
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

    #[derive(Clone)]
    pub struct TypescriptGeneratorConfig {
        pub mode: GenerationMode,
        pub structural_guards: bool,
        pub naming: NamingConvention,
        pub output_structure: OutputStructure,
        pub output_dir: PathBuf,
        pub generate_profiles: bool,
        pub generate_valuesets: bool,
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

            let profile_classes = section
                .as_ref()
                .map(|s| s.profile_classes)
                .unwrap_or(true);

            let profile_methods = section
                .as_ref()
                .map(|s| s.profile_methods.clone())
                .unwrap_or_default();

            let zod_schemas = section
                .as_ref()
                .map(|s| s.zod_schemas)
                .unwrap_or(true);

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
            }
        }
    }
}


mod naming {
    pub fn pascal_case(value: &str) -> String {
        split_tokens(value)
            .into_iter()
            .map(|token| {
                let mut chars = token.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<String>()
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
        for ch in value.chars() {
            if ch.is_alphanumeric() {
                current.push(ch);
            } else if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
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

        // Register custom filters
        tera.register_filter("pascal_case", filter_pascal_case);
        tera.register_filter("camel_case", filter_camel_case);
        tera.register_filter("sanitize_id", filter_sanitize_id);
        tera.register_filter("wrap_doc", filter_wrap_doc);

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
}

/// Represents a group of types imported from the same source file
#[derive(Debug, Clone, Serialize)]
struct RenderImport {
    /// Types to import from this source
    types: Vec<String>,
    /// Import path (relative or cross-package)
    path: String,
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
    has_primitives: bool,
    primitive_imports: Vec<String>,
    fields: Vec<RenderField>,
    imports: Vec<RenderImport>,
    /// Schema imports for Zod schemas (separate from type imports)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    schema_imports: Vec<RenderImport>,
    nested_types: Vec<RenderNestedType>,
    /// Generic type parameters for the interface (e.g., "T extends string = string" for Reference<T>)
    type_parameters: Option<String>,
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

        let Some(cache) = &self.config.package_cache else {
            // No cache available, skip ValueSet generation
            return Ok((0, HashMap::new()));
        };

        // Create valuesets subdirectory
        let valuesets_dir = package_dir.join("valuesets");
        fs::create_dir_all(&valuesets_dir)?;

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
                            Some(100), // Max 100 codes per ValueSet
                        ) {
                            Ok(Some(info)) => {
                                let ts_code = info.generate_typescript();
                                let file_name = naming::snake_case(&type_name)
                                    .replace('_', "-")
                                    .to_ascii_lowercase();
                                let output_path = valuesets_dir.join(format!("{}.ts", file_name));

                                fs::write(&output_path, ts_code)?;
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

        let mut relevant: Vec<_> = summaries
            .into_iter()
            .filter(|summary| summary.package == descriptor.id)
            .collect();

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
            // Generate BaseResource, ComplexType, PrimitiveType, and Logical structures
            // Logical includes important data types like CodeableConcept, Extension, Period, Range, etc.
            if summary.kind != StructureKind::BaseResource
                && summary.kind != StructureKind::ComplexType
                && summary.kind != StructureKind::PrimitiveType
                && summary.kind != StructureKind::Logical
            {
                continue;
            }

            let structure = service
                .load_structure(&summary.canonical_url)
                .await
                .with_context(|| format!("failed to load {}", summary.canonical_url))?;
            entries.push((summary, structure));
        }

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

        for (summary, definition) in &entries {
            let type_name =
                naming::pascal_case(summary.type_code.as_deref().unwrap_or(&definition.id));
            let mut stem = file_stem(&definition.id);
            let counter = used_stems.entry(stem.clone()).or_insert(0);
            *counter += 1;
            if *counter > 1 {
                stem = format!("{stem}_{}", counter);
            }
            let file_name = format!("{stem}.ts");
            name_to_file.insert(type_name.clone(), file_name);
            name_to_stem.insert(type_name, stem);
        }

        // Collect resource types for interop generation (only BaseResource kinds)
        let resource_types: Vec<String> = entries
            .iter()
            .filter_map(|(summary, definition)| {
                // Only include actual FHIR resources that can be reference targets
                if summary.kind == StructureKind::BaseResource {
                    let type_name = naming::pascal_case(
                        summary.type_code.as_deref().unwrap_or(&definition.id),
                    );
                    Some(type_name)
                } else {
                    None
                }
            })
            .collect();

        // Phase 2: Generate structures using the complete name mapping
        let mut structures = Vec::new();
        let mut profiles = Vec::new();
        let mut all_extensions = IndexMap::new();

        for (summary, definition) in entries {
            // Extract extensions from this resource definition
            let extensions = extensions::extract_extensions(&definition);
            all_extensions.extend(extensions);

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

            if is_profile {
                if !self.config.generate_profiles {
                    continue;
                }
                // Generate profile
                if let Some(profile_info) =
                    profiles::ProfileInfo::from_resource_definition(&definition)
                    && profile_info.has_constraints()
                {
                    // Check if any profile methods are enabled (for backward compat with old method)
                    let with_methods = self.config.profile_methods.extension_accessors
                        || self.config.profile_methods.serialization
                        || self.config.profile_methods.validation;

                    let ts_code = profile_info.generate_typescript(
                        self.config.profile_classes,
                        with_methods,
                        self.config.zod_schemas,
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
                        profile_value_exports.push(format!(
                            "{}Schema",
                            profile_info.type_name
                        ));
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

        let package_output = PackageOutput {
            path: package_dir.clone(),
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
        let mut context = TeraContext::new();
        context.insert("extensions", &package.extensions);
        let content = templates::render("extensions.ts.tera", &context)?;
        fs::write(package.path.join("extensions.ts"), content)
            .with_context(|| "failed to write extensions.ts")?;
    }

    // Write generic extension utilities to utils/ directory
    let utils_dir = package.path.join("utils");
    fs::create_dir_all(&utils_dir)
        .with_context(|| format!("failed to create utils directory {}", utils_dir.display()))?;

    let utils_content = templates::render("extension_utils.ts.tera", &TeraContext::new())?;
    fs::write(utils_dir.join("extensions.ts"), utils_content)
        .with_context(|| "failed to write utils/extensions.ts")?;

    // Write interop utilities if enabled
    if package.generate_interop {
        if let Some(config) = &package.interop_config {
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
                    fs::create_dir_all(&search_dir)
                        .with_context(|| format!("failed to create search directory {}", search_dir.display()))?;

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
                    interop_code.push_str("\n\n// Re-export search parameters from separate directory\n");
                    interop_code.push_str("export * from './search';\n");
                }

                // Write main interop.ts
                fs::write(utils_dir.join("interop.ts"), interop_code)
                    .with_context(|| "failed to write utils/interop.ts")?;
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
        let profile_index =
            templates::render("profiles-index.ts.tera", &profile_index_context)?;
        fs::write(profiles_dir.join("index.ts"), profile_index)
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
                    field.zod_type = crate::zod::element_to_zod_schema(element);
                }
            }
        }

        nested_types.push(RenderNestedType {
            type_name: nested_info.type_name,
            description: nested_info.doc_comment,
            fields: nested_fields,
        });
    }

    // Build fields for the main structure
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
                    if let Some(stem) = name_to_stem.get(type_ref) {
                        schema_type_refs_to_add
                            .insert(type_ref.clone(), (package_folder.to_string(), stem.clone()));
                    } else if let Some(type_registry) = &config.type_registry
                        && let Some((pkg_folder, stem)) = type_registry.get(type_ref)
                    {
                        schema_type_refs_to_add
                            .insert(type_ref.clone(), (pkg_folder.to_string(), stem.to_string()));
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
        import_groups
            .entry((pkg_folder.clone(), stem.clone()))
            .or_default()
            .push(imported_type.clone());
    }

    // Determine subfolder for import path calculation
    let from_subfolder = if is_profile { "profiles" } else { "" };

    // Build RenderImport structs with proper paths
    let mut imports: Vec<RenderImport> = import_groups
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

            RenderImport {
                types,
                path,
                source_package_folder: target_package_folder,
            }
        })
        .collect();

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
    let type_parameters = if type_name == "Reference" {
        Some("T extends string = string".to_string())
    } else {
        None
    };

    let output_folder = if is_profile {
        "profiles".to_string()
    } else {
        String::new()
    };

    // Generate Zod schema fields if enabled, collecting schema imports
    let generate_zod_schema = config.zod_schemas;
    let (zod_fields, schema_imports) = if generate_zod_schema {
        let mut fields = Vec::new();
        let mut schema_type_refs: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

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
                    let file_name = naming::snake_case(vs_type_name)
                        .replace('_', "-")
                        .to_ascii_lowercase();
                    let valueset_import_path = format!("./valuesets/{}", file_name);
                    schema_type_refs
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
                    // Collect type references for schema imports
                    for type_ref in &schema_info.type_refs {
                        // Determine import path based on type location
                        let import_path = if let Some(import) =
                            imports.iter().find(|imp| imp.types.contains(type_ref))
                        {
                            import.path.clone()
                        } else if has_primitives && primitive_imports.contains(type_ref) {
                            "./primitives".to_string()
                        } else {
                            // Same package type
                            let file_name = naming::snake_case(type_ref)
                                .replace('_', "-")
                                .to_ascii_lowercase();
                            format!("./{}", file_name)
                        };

                        schema_type_refs
                            .entry(import_path)
                            .or_default()
                            .push(type_ref.clone());
                    }

                    fields.push(ZodSchemaField {
                        name: field_name,
                        zod_type: schema_info.schema,
                    });
                }
            }
        }

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
                    source_package_folder: package_folder.to_string(),
                }
            })
            .collect();

        // Sort imports for consistent output
        schema_imports.sort_by(|a, b| a.path.cmp(&b.path));

        (fields, schema_imports)
    } else {
        (Vec::new(), Vec::new())
    };

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

    if config.structural_guards {
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
        structural_guards: config.structural_guards,
        has_primitives,
        primitive_imports,
        fields,
        imports,
        schema_imports,
        nested_types,
        type_parameters,
        package_name: package_name.to_string(),
        package_folder: package_folder.to_string(),
        is_profile,
        output_folder,
        generate_zod_schema,
        zod_fields,
        slices,
        type_exports,
        value_exports,
    }
}

fn top_level_elements(definition: &ResourceDefinition) -> Vec<&ElementDefinition> {
    // Find the root element (e.g., "Patient")
    let root = definition
        .elements
        .iter()
        .find(|elem| elem.path == definition.id);

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
    let prefix = format!("{}.", definition.id);
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

                    if type_name != current_type {
                        // Try current package first
                        if let Some(stem) = name_to_stem.get(&type_name) {
                            imports
                                .entry(type_name.clone())
                                .or_insert((current_package_folder.to_string(), stem.clone()));
                        } else if let Some(type_registry) = &config.type_registry {
                            // Try cross-package lookup
                            if let Some((pkg_folder, stem)) = type_registry.get(&type_name) {
                                imports
                                    .entry(type_name.clone())
                                    .or_insert((pkg_folder.to_string(), stem.to_string()));
                            }
                        }
                    }
                }
                TypeRef::Generic { base, type_arg } => {
                    // For generic types like Reference<"Patient">, we need to import the base type
                    let base_type = naming::pascal_case(base);
                    type_exprs.push(format!("{}<{}>", base_type, type_arg));

                    if base_type != current_type {
                        // Try current package first
                        if let Some(stem) = name_to_stem.get(&base_type) {
                            imports
                                .entry(base_type.clone())
                                .or_insert((current_package_folder.to_string(), stem.clone()));
                        } else if let Some(type_registry) = &config.type_registry {
                            // Try cross-package lookup
                            if let Some((pkg_folder, stem)) = type_registry.get(&base_type) {
                                imports
                                    .entry(base_type.clone())
                                    .or_insert((pkg_folder.to_string(), stem.to_string()));
                            }
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
    }
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
            "integer" | "decimal" => Some("FhirDecimal"),
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
        };
        let generator = TypescriptGenerator::new(config.clone());
        generator
            .generate(&provider, &descriptor, &provider_config)
            .await
            .expect("generate");

        let package_dir =
            package_output_dir(&config.output_dir, &descriptor.id, &config.package_folders);

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
        assert!(content.contains("export const FhirDateSchema = z.string().regex"));
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
