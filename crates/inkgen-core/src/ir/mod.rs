//! Intermediate Representation module for FHIR resources

pub mod resource;
pub mod element;
pub mod binding;
pub mod serialization;

#[cfg(test)]
mod tests;

pub use resource::{ResourceIR, ResourceMetadata, ResourceKind, DerivationType};
pub use element::{ElementTree, ElementNode, ElementDefinition, ElementType, SlicingInfo, Discriminator, DiscriminatorType, SlicingRules};
pub use binding::{TerminologyBinding, ResolvedBinding, Invariant, BindingStrength, InvariantSeverity, ValueSetExpansion, ValueSetConcept};
pub use serialization::IRSerializer;