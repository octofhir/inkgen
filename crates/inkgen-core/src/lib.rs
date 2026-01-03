//! InkGen core engine foundations.
//!
//! This crate provides the workspace-facing APIs for:
//! - FHIR package acquisition and caching
//! - Intermediate representation (IR) structures used by language backends
//! - Profile resolution services that transform canonical StructureDefinitions
//!   into deterministic IR snapshots

pub mod backends;
mod cache;
pub mod canonical_map;
pub mod config;
pub mod dependencies;
pub mod dependency_graph;
mod error;
pub mod generator;
pub mod ir;
mod lineage;
mod package;
mod profile;
pub mod search;
mod services;
pub mod template_helpers;
mod terminology;
mod type_registry;

pub use backends::{BackendRegistry, LanguageBackend};
pub use cache::{InstallMode, PackageCache, PackageCacheConfig};
pub use canonical_map::{CanonicalTypeMap, TypeEntry};
pub use config::{
    FilterMode, InkgenConfig, LanguagesSection, PackageEntry, ProjectFilesConfig,
    TypescriptLanguageConfig, sanitize_package_name,
};
pub use dependencies::DependencyAnalyzer;
pub use dependency_graph::TypeDependencyGraph;
pub use error::{CoreError, CoreResult};
pub use generator::LanguageGenerator;
pub use lineage::{ProfileAncestor, ProfileChain, merge_element_snapshots, resolve_full_chain};
pub use package::{
    ArtifactDescriptor, ArtifactKind, PackageDescriptor, PackageId, PackageInventory,
    PackageRequest, PackageSource, StructureKind, StructureSummary,
};
pub use search::SearchParameterInfo;
pub use services::{
    BaseStructureService, PackageResolver, StructureDefinitionProvider, StructureFilter,
    StructureProviderConfig,
};
pub use template_helpers::{ImportPathFunction, PackageFolderFunction, calculate_relative_import};
pub use terminology::{
    ResolvedValueSet, ValueSetCache, extract_codes_from_valueset, should_generate_valueset,
};
pub use type_registry::{FhirTypeInfo, FhirTypeRegistry};
