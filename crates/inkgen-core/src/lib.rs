//! Core engine placeholder providing shared types and traits.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Represents a canonical package identifier placeholder.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageId {
    /// Canonical package name, e.g. `hl7.fhir.r4.core`.
    pub name: String,
    /// Optional package version.
    pub version: Option<String>,
}

impl PackageId {
    /// Construct a new package identifier.
    pub fn new<N: Into<String>>(name: N, version: Option<String>) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }
}

/// Placeholder structure for resolved package metadata.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Arbitrary key-value metadata describing the package contents.
    pub properties: IndexMap<String, String>,
}

/// Core service trait that future tasks will implement.
pub trait PackageService {
    /// Returns metadata for a known package.
    fn describe(&self, id: &PackageId) -> anyhow::Result<Option<PackageMetadata>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_id_round_trip() {
        let id = PackageId::new("hl7.fhir.r4.core", Some("4.0.1".into()));
        let json = serde_json::to_string(&id).expect("serialize");
        let restored: PackageId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.name, "hl7.fhir.r4.core");
        assert_eq!(restored.version.as_deref(), Some("4.0.1"));
    }
}
