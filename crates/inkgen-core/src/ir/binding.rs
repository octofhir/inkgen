//! Terminology bindings and invariants for IR

use serde::{Deserialize, Serialize};

/// Terminology binding information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminologyBinding {
    pub path: String,
    pub value_set: Option<String>,
    pub strength: BindingStrength,
    pub description: Option<String>,
}

/// Strength of terminology binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BindingStrength {
    Required,
    Extensible,
    Preferred,
    Example,
}

/// Resolved terminology binding with additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedBinding {
    pub binding: TerminologyBinding,
    pub expansion: Option<ValueSetExpansion>,
}

/// Value set expansion information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetExpansion {
    pub identifier: String,
    pub timestamp: String,
    pub total: Option<u32>,
    pub contains: Vec<ValueSetConcept>,
}

/// Individual concept in a value set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetConcept {
    pub system: Option<String>,
    pub code: String,
    pub display: Option<String>,
}

/// FHIR invariant constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invariant {
    pub key: String,
    pub severity: InvariantSeverity,
    pub human: String,
    pub expression: String,
    pub xpath: Option<String>,
}

/// Severity level of an invariant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvariantSeverity {
    Error,
    Warning,
}

impl TerminologyBinding {
    /// Create a new terminology binding
    pub fn new(path: String, value_set: Option<String>, strength: BindingStrength) -> Self {
        Self {
            path,
            value_set,
            strength,
            description: None,
        }
    }

    /// Check if this binding is required
    pub fn is_required(&self) -> bool {
        matches!(self.strength, BindingStrength::Required)
    }
}

impl Invariant {
    /// Create a new invariant
    pub fn new(key: String, severity: InvariantSeverity, human: String, expression: String) -> Self {
        Self {
            key,
            severity,
            human,
            expression,
            xpath: None,
        }
    }

    /// Check if this invariant is an error-level constraint
    pub fn is_error(&self) -> bool {
        matches!(self.severity, InvariantSeverity::Error)
    }
}