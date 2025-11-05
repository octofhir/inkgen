//! Profile processing module for FHIR profiles

pub mod service;
pub mod merger;
pub mod flattener;

pub use service::{ProfileService, ProfileResolutionConfig};
pub use merger::ProfileMerger;
pub use flattener::ProfileFlattener;