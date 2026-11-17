use std::path::PathBuf;

#[cfg(feature = "resolver")]
use octofhir_canonical_manager::FcmError;
use thiserror::Error;

use crate::package::PackageId;

/// Result alias used across the core engine.
pub type CoreResult<T> = Result<T, CoreError>;

/// Unified error type for the core engine.
#[derive(Debug, Error)]
pub enum CoreError {
    /// The canonical manager failed to install, load, or resolve a package.
    #[cfg(feature = "resolver")]
    #[error("canonical manager failure: {0}")]
    Canonical(#[from] FcmError),

    /// A filesystem or other I/O operation failed.
    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization of a FHIR resource or config failed.
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// A canonical or registry URL could not be parsed.
    #[cfg(feature = "resolver")]
    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    /// A package identifier is malformed (expected `name@version`).
    #[error("invalid package identifier: {detail}")]
    InvalidPackage {
        /// Explanation of why the identifier is invalid.
        detail: String,
    },

    /// The requested package is not installed in the local cache.
    #[error("package not found in cache: {package}")]
    PackageNotFound {
        /// Identifier of the missing package.
        package: PackageId,
    },

    /// No StructureDefinition matches the given canonical URL.
    #[error("profile not found: {canonical}")]
    ProfileNotFound {
        /// Canonical URL that could not be found.
        canonical: String,
    },

    /// Resolving a profile's snapshot or lineage failed.
    #[error("profile resolution failed for {canonical}: {detail}")]
    ProfileResolution {
        /// Canonical URL of the profile being resolved.
        canonical: String,
        /// Explanation of the resolution failure.
        detail: String,
    },

    /// The requested FHIR construct or operation is not yet supported.
    #[error("unsupported feature: {detail}")]
    Unsupported {
        /// Description of the unsupported feature.
        detail: String,
    },

    /// Input (config, resource, or request) failed validation.
    #[error("validation error: {detail}")]
    Validation {
        /// Description of the validation failure.
        detail: String,
    },

    /// Offline mode was requested but a needed package is not cached locally.
    #[error("offline mode cannot satisfy request; missing cached package {package}")]
    OfflineUnavailable {
        /// Identifier of the package missing from the cache.
        package: PackageId,
        /// Cache directory that was searched.
        cache_dir: PathBuf,
    },

    /// Any other error propagated from lower layers.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl CoreError {
    /// Construct a [`CoreError::Validation`] from a detail message.
    pub fn validation(detail: impl Into<String>) -> Self {
        Self::Validation {
            detail: detail.into(),
        }
    }

    /// Construct a [`CoreError::Unsupported`] from a detail message.
    pub fn unsupported(detail: impl Into<String>) -> Self {
        Self::Unsupported {
            detail: detail.into(),
        }
    }

    /// Construct a [`CoreError::ProfileResolution`] for the given canonical URL.
    pub fn profile_resolution(canonical: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::ProfileResolution {
            canonical: canonical.into(),
            detail: detail.into(),
        }
    }
}
