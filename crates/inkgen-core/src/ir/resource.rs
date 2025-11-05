//! Resource metadata structures for IR

use serde::{Deserialize, Serialize};
use crate::ir::{ElementTree, TerminologyBinding, Invariant};

/// Complete intermediate representation of a FHIR resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceIR {
    pub metadata: ResourceMetadata,
    pub elements: ElementTree,
    pub bindings: Vec<TerminologyBinding>,
    pub invariants: Vec<Invariant>,
}

/// Metadata about a FHIR resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    pub name: String,
    pub description: Option<String>,
    pub kind: ResourceKind,
    pub base_definition: Option<String>,
    pub derivation: DerivationType,
}

/// Type of FHIR resource
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResourceKind {
    Resource,
    ComplexType,
    PrimitiveType,
    Logical,
}

/// How this resource derives from its base
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DerivationType {
    Specialization,
    Constraint,
}

impl ResourceIR {
    /// Create a new ResourceIR instance
    pub fn new(metadata: ResourceMetadata, elements: ElementTree) -> Self {
        Self {
            metadata,
            elements,
            bindings: Vec::new(),
            invariants: Vec::new(),
        }
    }

    /// Add a terminology binding to this resource
    pub fn add_binding(&mut self, binding: TerminologyBinding) {
        self.bindings.push(binding);
    }

    /// Add an invariant to this resource
    pub fn add_invariant(&mut self, invariant: Invariant) {
        self.invariants.push(invariant);
    }

    /// Get all element paths in this resource
    pub fn get_element_paths(&self) -> Vec<&String> {
        self.elements.elements.keys().collect()
    }
}