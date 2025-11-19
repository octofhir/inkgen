//! InkGen core engine foundations.
//!
//! This crate provides the workspace-facing APIs for:
//! - FHIR package acquisition and caching
//! - Intermediate representation (IR) structures used by language backends
//! - Profile resolution services that transform canonical StructureDefinitions
//!   into deterministic IR snapshots

pub mod backends;
mod cache;
pub mod config;
pub mod dependencies;
mod error;
pub mod generator;
pub mod ir;
mod lineage;
mod package;
mod profile;
mod services;
pub mod template_helpers;
mod terminology;

pub use backends::{BackendRegistry, LanguageBackend};
pub use cache::{InstallMode, PackageCache, PackageCacheConfig};
pub use config::{
    sanitize_package_name, FilterMode, InkgenConfig, LanguagesSection, PackageEntry,
    TypescriptLanguageConfig,
};
pub use dependencies::DependencyAnalyzer;
pub use error::{CoreError, CoreResult};
pub use generator::LanguageGenerator;
pub use lineage::{ProfileAncestor, ProfileChain, merge_element_snapshots, resolve_full_chain};
pub use template_helpers::{
    ImportPathFunction, PackageFolderFunction, calculate_relative_import,
};
pub use package::{
    ArtifactDescriptor, ArtifactKind, PackageDescriptor, PackageId, PackageInventory,
    PackageRequest, PackageSource, StructureKind, StructureSummary,
};
pub use services::{
    BaseStructureService, PackageResolver, StructureDefinitionProvider, StructureFilter,
    StructureProviderConfig,
};
pub use terminology::{
    ResolvedValueSet, ValueSetCache, extract_codes_from_valueset, should_generate_valueset,
};
