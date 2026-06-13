//! `PackageIr` — the language-neutral, serializable aggregate handed to backends.
//!
//! This is the plugin base: a backend author receives a `&PackageIr` (every type
//! resolved once, FHIR semantics already lowered) and returns a
//! [`GenerationOutput`]. The [`Backend`] trait is a *pure function* of the IR —
//! no I/O, no async, no canonical resolver — which is what makes determinism,
//! testing, out-of-process, and WASM plugins possible from one contract.
//!
//! See `docs/analysis/rfc-plugin-base.md` for the full design.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical_map::CanonicalTypeMap;
use crate::ir::ResourceDefinition;
use crate::search::SearchParameterInfo;
use crate::services::{StructureDefinitionProvider, StructureFilter};
use crate::{
    CoreResult, PackageCache, PackageDescriptor, StructureProviderConfig, StructureSummary,
};

/// Version of the backend contract (`PackageIr` shape + [`Backend`] trait).
///
/// Bump on any breaking change so out-of-process / WASM plugins built against an
/// older contract can detect the mismatch instead of mis-parsing the IR.
pub const CONTRACT_VERSION: u32 = 1;

/// The fully-resolved, language-neutral package a backend consumes.
///
/// Built once by core (see [`build_package_ir`]); backends never call the
/// provider or canonical resolver. Deterministic: all maps are key-sorted, so
/// identical inputs produce an identical IR and thus identical output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageIr {
    /// Contract version this IR was produced under (see [`CONTRACT_VERSION`]).
    pub contract_version: u32,
    /// Package identifier (e.g. `hl7.fhir.r4.core@4.0.1`).
    pub id: String,
    /// FHIR version string (e.g. `4.0.1`) when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhir_version: Option<String>,
    /// Dependency package identifiers.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Resolved types keyed by canonical URL, resolved exactly once.
    pub types: IndexMap<String, ResourceDefinition>,
    /// Type/resource name → canonical URL, for cross-type lookups without
    /// re-resolution.
    pub canonical_index: IndexMap<String, String>,
    /// Authoritative type metadata across all loaded packages (name → URL,
    /// kind, file stem, package), used for import/path resolution. Built once.
    #[serde(default)]
    pub type_map: CanonicalTypeMap,
    /// Lightweight summaries for every structure in the filter (kind, package,
    /// name) — what a backend needs for grouping/filtering without re-listing.
    #[serde(default)]
    pub structures: Vec<StructureSummary>,
    /// Resolved terminology resources (ValueSet + CodeSystem JSON) keyed by
    /// canonical URL. Backends build coded enums from this — no tx/manager calls.
    #[serde(default)]
    pub value_sets: IndexMap<String, Value>,
    /// Search parameters defined by the package.
    #[serde(default)]
    pub search_parameters: Vec<SearchParameterInfo>,
    /// The descriptor of the package this IR was built for (id, inventory) — the
    /// target a backend generates, since `types` may include dependency packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_descriptor: Option<PackageDescriptor>,
    /// Constructs skipped or unsupported during lowering (feeds `explain`).
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl PackageIr {
    /// Iterate resolved types in deterministic (canonical-URL) order.
    pub fn types(&self) -> impl Iterator<Item = &ResourceDefinition> {
        self.types.values()
    }

    /// Look up a resolved type by canonical URL.
    pub fn get(&self, canonical: &str) -> Option<&ResourceDefinition> {
        self.types.get(canonical)
    }

    /// Look up a resolved type by its type/resource name.
    pub fn by_name(&self, name: &str) -> Option<&ResourceDefinition> {
        self.canonical_index
            .get(name)
            .and_then(|url| self.types.get(url))
    }
}

/// A construct that could not be lowered, recorded for transparency (`explain`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    /// Stable machine code (e.g. `resolve_failed`, `intensional_valueset`).
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_path: Option<String>,
}

/// Severity of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Files produced by a backend. Core owns directory layout and writing — a
/// backend only declares relative paths and contents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerationOutput {
    /// Relative path → file contents. Sorted on write for determinism.
    pub files: IndexMap<String, String>,
}

impl GenerationOutput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a generated file (relative path → contents).
    pub fn add_file(&mut self, path: impl Into<String>, content: impl Into<String>) {
        self.files.insert(path.into(), content.into());
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The contract a plugin author implements.
///
/// A pure function of `&PackageIr`: no I/O, no async, no resolver. The IR is
/// fully resolved and the FHIR semantics are already lowered in core, so an
/// implementation only maps the IR to its target language's strings.
pub trait Backend {
    /// Registry key / CLI name (e.g. `typescript`, `rust`).
    fn id(&self) -> &str;

    /// Generate output from the resolved package IR.
    fn generate(&self, ir: &PackageIr) -> Result<GenerationOutput, BackendError>;
}

/// Error returned by a [`Backend`].
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<String> for BackendError {
    fn from(value: String) -> Self {
        BackendError::Message(value)
    }
}

impl From<&str> for BackendError {
    fn from(value: &str) -> Self {
        BackendError::Message(value.to_string())
    }
}

/// Build a [`PackageIr`] by resolving every structure in the filter exactly once.
///
/// This is where the redundant re-resolution the CLI/backends do today collapses
/// to a single pass. The resulting IR is deterministic (key-sorted) and carries
/// diagnostics for any structure that failed to resolve.
pub async fn build_package_ir<S>(
    service: &S,
    cache: &PackageCache,
    descriptor: &PackageDescriptor,
    config: &StructureProviderConfig,
    fhir_version: Option<String>,
    dependencies: Vec<String>,
) -> CoreResult<PackageIr>
where
    S: StructureDefinitionProvider + Sync + Send,
{
    let filter = StructureFilter::from_config(config);
    let structures = service.list_structures(&filter).await?;

    let mut types: IndexMap<String, ResourceDefinition> = IndexMap::new();
    let mut canonical_index: IndexMap<String, String> = IndexMap::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    for summary in &structures {
        match service.load_structure(&summary.canonical_url).await {
            Ok(def) => {
                if let Some(name) = &def.name {
                    canonical_index
                        .entry(name.clone())
                        .or_insert_with(|| def.url.clone());
                }
                canonical_index
                    .entry(def.id.clone())
                    .or_insert_with(|| def.url.clone());
                types.insert(def.url.clone(), def);
            }
            Err(err) => diagnostics.push(Diagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "resolve_failed".to_string(),
                message: err.to_string(),
                canonical_url: Some(summary.canonical_url.clone()),
                element_path: None,
            }),
        }
    }

    // Authoritative cross-package type metadata (name/url/stem/kind/package).
    let manager = cache.manager().await?;
    let type_map = CanonicalTypeMap::from_manager(&manager)
        .await
        .unwrap_or_default();

    // Resolve terminology (ValueSet + CodeSystem) content once — backends build
    // coded enums from this instead of touching a manager / tx server.
    let mut value_sets: IndexMap<String, Value> = IndexMap::new();
    for artifact in descriptor
        .inventory
        .value_sets
        .iter()
        .chain(descriptor.inventory.code_systems.iter())
    {
        if let Some(url) = &artifact.canonical_url
            && let Ok(resolved) = manager.resolve(url).await
        {
            value_sets.insert(url.clone(), resolved.resource.content.clone());
        }
    }

    let search_parameters = cache
        .load_search_parameters(&descriptor.id)
        .await
        .unwrap_or_default();

    // Deterministic ordering — the D1 guarantee extends to every backend.
    types.sort_keys();
    canonical_index.sort_keys();
    value_sets.sort_keys();

    Ok(PackageIr {
        contract_version: CONTRACT_VERSION,
        id: descriptor.id.to_string(),
        fhir_version,
        dependencies,
        types,
        canonical_index,
        type_map,
        structures,
        value_sets,
        search_parameters,
        package_descriptor: Some(descriptor.clone()),
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_output_collects_files() {
        let mut out = GenerationOutput::new();
        assert!(out.is_empty());
        out.add_file("patient.ts", "export interface Patient {}");
        out.add_file("observation.ts", "export interface Observation {}");
        assert_eq!(out.len(), 2);
        assert_eq!(
            out.files.get("patient.ts").map(String::as_str),
            Some("export interface Patient {}")
        );
    }

    #[test]
    fn package_ir_empty_lookups() {
        let ir = PackageIr {
            contract_version: CONTRACT_VERSION,
            id: "hl7.fhir.r4.core@4.0.1".to_string(),
            fhir_version: Some("4.0.1".to_string()),
            dependencies: vec![],
            types: IndexMap::new(),
            canonical_index: IndexMap::new(),
            type_map: CanonicalTypeMap::default(),
            structures: vec![],
            value_sets: IndexMap::new(),
            search_parameters: vec![],
            package_descriptor: None,
            diagnostics: vec![],
        };

        assert!(ir.by_name("Patient").is_none());
        assert!(ir.get("http://example/Patient").is_none());
        assert_eq!(ir.types().count(), 0);
        assert_eq!(ir.contract_version, CONTRACT_VERSION);
    }
}
