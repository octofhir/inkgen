//! InkGen core engine foundations.
//!
//! This crate provides the workspace-facing APIs for:
//! - FHIR package acquisition and caching
//! - Intermediate representation (IR) structures used by language backends
//! - Profile resolution services that transform canonical StructureDefinitions
//!   into deterministic IR snapshots

#[cfg(feature = "resolver")]
mod cache;
pub mod canonical_map;
/// Project configuration (`inkgen.toml`) types and per-package filtering.
pub mod config;
pub mod dependencies;
pub mod dependency_graph;
mod error;
/// Intermediate representation types consumed by language backends.
pub mod ir;
mod lineage;
mod package;
pub mod package_ir;
#[cfg(any(feature = "resolver", test))]
mod profile;
pub mod search;
mod services;
#[cfg(feature = "templates")]
pub mod template_helpers;
mod terminology;
mod type_registry;

#[cfg(feature = "resolver")]
pub use cache::{InstallMode, PackageCache, PackageCacheConfig};
pub use canonical_map::{CanonicalTypeMap, TypeEntry};
pub use config::{
    FilterMode, InkgenConfig, LanguagesSection, PackageEntry, ProjectFilesConfig,
    TypescriptLanguageConfig, sanitize_package_name,
};
pub use dependencies::DependencyAnalyzer;
pub use dependency_graph::TypeDependencyGraph;
pub use error::{CoreError, CoreResult};
pub use lineage::{ProfileAncestor, ProfileChain, merge_element_snapshots, resolve_full_chain};
pub use package::{
    ArtifactDescriptor, ArtifactKind, PackageDescriptor, PackageId, PackageInventory,
    PackageRequest, PackageSource, StructureKind, StructureSummary,
};
#[cfg(feature = "resolver")]
pub use package_ir::build_package_ir;
pub use package_ir::{
    Backend, BackendError, CONTRACT_VERSION, Diagnostic, DiagnosticSeverity, GenerationOutput,
    PackageIr,
};
pub use search::SearchParameterInfo;
#[cfg(feature = "resolver")]
pub use services::{BaseStructureService, PackageResolver};
pub use services::{StructureDefinitionProvider, StructureFilter, StructureProviderConfig};
#[cfg(feature = "templates")]
pub use template_helpers::{ImportPathFunction, PackageFolderFunction, calculate_relative_import};
pub use terminology::{
    ResolvedValueSet, ValueSetCache, extract_codes_from_valueset, should_generate_valueset,
};
pub use type_registry::{FhirTypeInfo, FhirTypeRegistry};
