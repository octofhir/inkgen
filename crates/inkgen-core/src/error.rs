use std::path::PathBuf;

use octofhir_canonical_manager::FcmError;
use thiserror::Error;

use crate::package::PackageId;

/// Result alias used across the core engine.
pub type CoreResult<T> = Result<T, CoreError>;

/// Unified error type for the core engine.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("canonical manager failure: {0}")]
    Canonical(#[from] FcmError),

    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("invalid package identifier: {detail}")]
    InvalidPackage { detail: String },

    #[error("package not found in cache: {package}")]
    PackageNotFound { package: PackageId },

    #[error("profile not found: {canonical}")]
    ProfileNotFound { canonical: String },

    #[error("profile resolution failed for {canonical}: {detail}")]
    ProfileResolution { canonical: String, detail: String },

    #[error("unsupported feature: {detail}")]
    Unsupported { detail: String },

    #[error("validation error: {detail}")]
    Validation { detail: String },

    #[error("offline mode cannot satisfy request; missing cached package {package}")]
    OfflineUnavailable {
        package: PackageId,
        cache_dir: PathBuf,
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl CoreError {
    pub fn validation(detail: impl Into<String>) -> Self {
        Self::Validation {
            detail: detail.into(),
        }
    }

    pub fn unsupported(detail: impl Into<String>) -> Self {
        Self::Unsupported {
            detail: detail.into(),
        }
    }

    pub fn profile_resolution(canonical: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::ProfileResolution {
            canonical: canonical.into(),
            detail: detail.into(),
        }
    }
}
