//! # Inkgen Core
//!
//! Core FHIR processing library for Inkgen, providing comprehensive tools for working with
//! FHIR profiles, packages, and intermediate representations.
//!
//! ## Features
//!
//! - **Profile Resolution**: Resolve and flatten FHIR profiles from packages
//! - **Package Management**: Load and manage FHIR packages with dependency resolution
//! - **Intermediate Representation**: Convert FHIR resources to structured IR for code generation
//! - **Terminology Binding**: Resolve and expand FHIR value sets and code systems
//! - **Performance Monitoring**: Built-in tracing and performance measurement
//!
//! ## Quick Start
//!
//! ```rust
//! use inkgen_core::{PackageResolver, ProfileService, CoreConfig};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a package resolver
//! let resolver = Arc::new(PackageResolver::new().await?);
//!
//! // Create a profile service with default configuration
//! let mut profile_service = ProfileService::new(resolver.clone());
//!
//! // Resolve a FHIR profile
//! let patient_ir = profile_service
//!     .resolve_profile("http://hl7.org/fhir/StructureDefinition/Patient")
//!     .await?;
//!
//! println!("Resolved profile: {}", patient_ir.metadata.name);
//! # Ok(())
//! # }
//! ```
//!
//! ## Configuration
//!
//! Use [`CoreConfig`] to customize behavior:
//!
//! ```rust
//! use inkgen_core::{CoreConfig, ProfileResolutionConfig, IROptions};
//!
//! // Use predefined configurations for common scenarios
//! let us_core_config = CoreConfig::us_core();
//! let production_config = CoreConfig::for_production();
//! let dev_config = CoreConfig::for_development();
//!
//! // Or create custom configuration
//! let config = CoreConfig {
//!     profile_resolution: ProfileResolutionConfig {
//!         include_must_support_only: true,
//!         resolve_terminology: true,
//!         flatten_choice_types: false,
//!         enable_slicing_resolution: true,
//!         include_inherited_elements: true,
//!         resolve_extensions: false,
//!         validate_cardinality: true,
//!         enable_invariant_processing: false,
//!         max_recursion_depth: 5,
//!         cache_resolved_profiles: true,
//!         parallel_resolution: true,
//!     },
//!     ir_options: IROptions {
//!         version: "1.0.0".to_string(),
//!         deterministic_serialization: true,
//!         include_debug_info: true,
//!     },
//! };
//! ```
//!
//! ## Advanced Usage
//!
//! ```rust
//! use inkgen_core::{PackageResolver, ProfileService, ProfileResolutionConfig};
//! use std::sync::Arc;
//!
//! # async fn advanced_example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create resolver and service with custom configuration
//! let resolver = Arc::new(PackageResolver::new().await?);
//! let config = ProfileResolutionConfig::for_production();
//! let mut service = ProfileService::with_config(resolver, config);
//!
//! // Resolve and flatten a profile
//! let flattened_ir = service
//!     .flatten_profile("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient")
//!     .await?;
//!
//! // Get must-support elements
//! let must_support_elements = service.get_must_support_elements(&flattened_ir);
//! println!("Found {} must-support elements", must_support_elements.len());
//!
//! // Resolve terminology bindings
//! let bindings = service.resolve_terminology_bindings(&flattened_ir)?;
//! println!("Resolved {} terminology bindings", bindings.len());
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod fhir;
pub mod package;
pub mod ir;
pub mod profile;
pub mod tracing;

// Error handling types
pub use error::{CoreError, ValidationViolation, ValidationSeverity, ErrorSeverity, RecoveryOption};

// FHIR resource types and structures
pub use fhir::{
    FhirResource, ResourceType, Resource, Meta, Patient, Observation, 
    StructureDefinition, Reference, CodeableConcept, Coding, HumanName
};

// Package management
pub use package::{PackageResolver, Package, PackageInfo, PackageManifest};

// Intermediate representation types
pub use ir::{
    ResourceIR, ResourceMetadata, ResourceKind, DerivationType,
    ElementTree, ElementNode, ElementDefinition, ElementType, SlicingInfo,
    TerminologyBinding, ResolvedBinding, Invariant,
    IRSerializer
};

// Profile processing services
pub use profile::{ProfileService, ProfileResolutionConfig, ProfileMerger, ProfileFlattener};

// Performance monitoring and tracing
pub use tracing::{PerformanceMonitor, PackageResolutionContext, ProfileProcessingContext};

/// Core result type for the inkgen-core crate
///
/// This is a convenience type alias that uses [`CoreError`] as the error type.
/// Most functions in this crate return this result type.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Core configuration for Inkgen
///
/// This structure contains all configuration options for the Inkgen core library.
/// It can be serialized/deserialized for storage in configuration files.
///
/// # Examples
///
/// ```rust
/// use inkgen_core::{CoreConfig, ProfileResolutionConfig, IROptions};
///
/// // Create with defaults
/// let config = CoreConfig::default();
///
/// // Create with custom settings
/// let config = CoreConfig {
///     profile_resolution: ProfileResolutionConfig {
///         include_must_support_only: true,
///         resolve_terminology: false,
///         flatten_choice_types: true,
///         enable_slicing_resolution: false,
///         include_inherited_elements: true,
///         resolve_extensions: false,
///         validate_cardinality: true,
///         enable_invariant_processing: false,
///         max_recursion_depth: 15,
///         cache_resolved_profiles: true,
///         parallel_resolution: true,
///     },
///     ir_options: IROptions {
///         version: "2.0.0".to_string(),
///         deterministic_serialization: true,
///         include_debug_info: false,
///     },
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoreConfig {
    /// Configuration for profile resolution behavior
    pub profile_resolution: ProfileResolutionConfig,
    /// Configuration for intermediate representation processing
    pub ir_options: IROptions,
}

/// Configuration options for IR processing
///
/// Controls how intermediate representations are generated and serialized.
///
/// # Fields
///
/// - `version`: Version string to include in generated IR
/// - `deterministic_serialization`: Whether to ensure consistent ordering in serialized output
/// - `include_debug_info`: Whether to include additional debugging information in the IR
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IROptions {
    /// Version string to include in generated IR
    pub version: String,
    /// Whether to ensure consistent ordering in serialized output
    pub deterministic_serialization: bool,
    /// Whether to include additional debugging information in the IR
    pub include_debug_info: bool,
}

impl Default for CoreConfig {
    /// Creates a default configuration suitable for most use cases
    ///
    /// The default configuration enables terminology resolution and choice type flattening,
    /// with deterministic serialization enabled for consistent output.
    fn default() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig::default(),
            ir_options: IROptions::default(),
        }
    }
}

impl CoreConfig {
    /// Creates a new CoreConfig with custom profile resolution settings
    ///
    /// # Arguments
    ///
    /// * `profile_config` - Custom profile resolution configuration
    ///
    /// # Examples
    ///
    /// ```rust
    /// use inkgen_core::{CoreConfig, ProfileResolutionConfig};
    ///
    /// let profile_config = ProfileResolutionConfig {
    ///     include_must_support_only: true,
    ///     resolve_terminology: false,
    ///     flatten_choice_types: true,
    ///     enable_slicing_resolution: false,
    ///     include_inherited_elements: true,
    ///     resolve_extensions: false,
    ///     validate_cardinality: true,
    ///     enable_invariant_processing: false,
    ///     max_recursion_depth: 5,
    ///     cache_resolved_profiles: true,
    ///     parallel_resolution: true,
    /// };
    ///
    /// let config = CoreConfig::with_profile_config(profile_config);
    /// ```
    pub fn with_profile_config(profile_config: ProfileResolutionConfig) -> Self {
        Self {
            profile_resolution: profile_config,
            ir_options: IROptions::default(),
        }
    }

    /// Creates a new CoreConfig optimized for development/debugging
    ///
    /// This configuration includes debug information and uses more verbose settings
    /// that are helpful during development but may impact performance.
    pub fn for_development() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: false,
                resolve_terminology: true,
                flatten_choice_types: true,
                enable_slicing_resolution: true,
                include_inherited_elements: true,
                resolve_extensions: true,
                validate_cardinality: true,
                enable_invariant_processing: true,
                max_recursion_depth: 20,
                cache_resolved_profiles: false, // Disable caching for fresh results
                parallel_resolution: false, // Disable parallel for easier debugging
            },
            ir_options: IROptions {
                version: "dev".to_string(),
                deterministic_serialization: true,
                include_debug_info: true,
            },
        }
    }

    /// Creates a new CoreConfig optimized for production use
    ///
    /// This configuration prioritizes performance and minimal output size.
    pub fn for_production() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: true,
                resolve_terminology: false,
                flatten_choice_types: false,
                enable_slicing_resolution: false,
                include_inherited_elements: false,
                resolve_extensions: false,
                validate_cardinality: true,
                enable_invariant_processing: false,
                max_recursion_depth: 10,
                cache_resolved_profiles: true,
                parallel_resolution: true,
            },
            ir_options: IROptions {
                version: "1.0.0".to_string(),
                deterministic_serialization: true,
                include_debug_info: false,
            },
        }
    }
}

impl Default for IROptions {
    /// Creates default IR processing options
    ///
    /// Defaults to deterministic serialization enabled and debug info disabled
    /// for optimal performance while maintaining consistent output.
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            deterministic_serialization: true,
            include_debug_info: false,
        }
    }
}

impl IROptions {
    /// Creates IROptions optimized for debugging
    pub fn for_debugging() -> Self {
        Self {
            version: "debug".to_string(),
            deterministic_serialization: true,
            include_debug_info: true,
        }
    }

    /// Creates IROptions optimized for production
    pub fn for_production() -> Self {
        Self {
            version: "1.0.0".to_string(),
            deterministic_serialization: true,
            include_debug_info: false,
        }
    }
}

/// Predefined configurations for common use cases
impl CoreConfig {
    /// Configuration for US Core implementation guide processing
    ///
    /// Optimized for processing US Core profiles with must-support elements
    /// and comprehensive terminology resolution.
    pub fn us_core() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: true,
                resolve_terminology: true,
                flatten_choice_types: true,
                enable_slicing_resolution: true,
                include_inherited_elements: true,
                resolve_extensions: true,
                validate_cardinality: true,
                enable_invariant_processing: true,
                max_recursion_depth: 12,
                cache_resolved_profiles: true,
                parallel_resolution: true,
            },
            ir_options: IROptions {
                version: "us-core-6.1.0".to_string(),
                deterministic_serialization: true,
                include_debug_info: false,
            },
        }
    }

    /// Configuration for International Patient Summary (IPS) processing
    ///
    /// Optimized for IPS profiles with international terminology support.
    pub fn international_patient_summary() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: false,
                resolve_terminology: true,
                flatten_choice_types: true,
                enable_slicing_resolution: true,
                include_inherited_elements: true,
                resolve_extensions: true,
                validate_cardinality: true,
                enable_invariant_processing: true,
                max_recursion_depth: 15,
                cache_resolved_profiles: true,
                parallel_resolution: true,
            },
            ir_options: IROptions {
                version: "ips-1.1.0".to_string(),
                deterministic_serialization: true,
                include_debug_info: false,
            },
        }
    }

    /// Configuration for FHIR R4 core resource processing
    ///
    /// Optimized for processing base FHIR R4 resources and profiles.
    pub fn fhir_r4_core() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: false,
                resolve_terminology: true,
                flatten_choice_types: false, // Keep choice types for base resources
                enable_slicing_resolution: false,
                include_inherited_elements: true,
                resolve_extensions: false,
                validate_cardinality: true,
                enable_invariant_processing: false,
                max_recursion_depth: 8,
                cache_resolved_profiles: true,
                parallel_resolution: true,
            },
            ir_options: IROptions {
                version: "fhir-r4-4.0.1".to_string(),
                deterministic_serialization: true,
                include_debug_info: false,
            },
        }
    }

    /// Configuration for FHIR R5 core resource processing
    ///
    /// Optimized for processing base FHIR R5 resources and profiles.
    pub fn fhir_r5_core() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: false,
                resolve_terminology: true,
                flatten_choice_types: false, // Keep choice types for base resources
                enable_slicing_resolution: true, // R5 has better slicing support
                include_inherited_elements: true,
                resolve_extensions: false,
                validate_cardinality: true,
                enable_invariant_processing: true, // R5 has improved invariants
                max_recursion_depth: 10,
                cache_resolved_profiles: true,
                parallel_resolution: true,
            },
            ir_options: IROptions {
                version: "fhir-r5-5.0.0".to_string(),
                deterministic_serialization: true,
                include_debug_info: false,
            },
        }
    }

    /// Configuration for testing and validation scenarios
    ///
    /// Includes comprehensive validation and debugging features.
    pub fn for_testing() -> Self {
        Self {
            profile_resolution: ProfileResolutionConfig {
                include_must_support_only: false,
                resolve_terminology: true,
                flatten_choice_types: true,
                enable_slicing_resolution: true,
                include_inherited_elements: true,
                resolve_extensions: true,
                validate_cardinality: true,
                enable_invariant_processing: true,
                max_recursion_depth: 20,
                cache_resolved_profiles: false, // Fresh resolution for each test
                parallel_resolution: false, // Sequential for predictable testing
            },
            ir_options: IROptions {
                version: "test".to_string(),
                deterministic_serialization: true,
                include_debug_info: true,
            },
        }
    }
}