//! InkGen core engine foundations.
//!
//! This crate provides the workspace-facing APIs for:
//! - FHIR package acquisition and caching
//! - Intermediate representation (IR) structures used by language backends
//! - Profile resolution services that transform canonical StructureDefinitions
//!   into deterministic IR snapshots

mod cache;
mod config;
mod error;
pub mod ir;
mod package;
mod profile;
mod services;

pub use cache::{InstallMode, PackageCache, PackageCacheConfig};
pub use config::{InkgenConfig, PackageEntry, TreeShakingSection};
pub use error::{CoreError, CoreResult};
pub use package::{
    ArtifactDescriptor, ArtifactKind, PackageDescriptor, PackageId, PackageInventory,
    PackageRequest, PackageSource, StructureKind, StructureSummary,
};
pub use services::{
    BaseStructureService, PackageResolver, StructureDefinitionProvider, StructureFilter,
    StructureProviderConfig,
};
