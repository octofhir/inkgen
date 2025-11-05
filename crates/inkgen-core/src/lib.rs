//! Core FHIR processing logic for Inkgen

pub mod error;
pub mod fhir;

pub use error::CoreError;
pub use fhir::{
    FhirResource, ResourceType, Resource, Meta, Patient, Observation, 
    StructureDefinition, Reference, CodeableConcept, Coding, HumanName
};

/// Core result type for the inkgen-core crate
pub type Result<T> = std::result::Result<T, CoreError>;