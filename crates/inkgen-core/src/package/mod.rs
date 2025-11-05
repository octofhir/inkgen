//! Package management module for FHIR packages

pub mod resolver;

pub use resolver::{PackageResolver, Package, PackageInfo, PackageManifest};