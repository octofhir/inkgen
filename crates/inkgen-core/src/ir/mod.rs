use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Kind of StructureDefinition represented by the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    PrimitiveType,
    ComplexType,
    Resource,
    Logical,
}

impl ResourceKind {
    pub fn from_fhir(kind: &str) -> Option<Self> {
        match kind {
            "primitive-type" => Some(Self::PrimitiveType),
            "complex-type" => Some(Self::ComplexType),
            "resource" => Some(Self::Resource),
            "logical" => Some(Self::Logical),
            _ => None,
        }
    }
}

/// Derivation strategy for a StructureDefinition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Derivation {
    Constraint,
    Specialization,
}

impl Derivation {
    pub fn from_fhir(value: &str) -> Option<Self> {
        match value {
            "constraint" => Some(Self::Constraint),
            "specialization" => Some(Self::Specialization),
            _ => None,
        }
    }
}

/// Metadata describing lineage for a profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileLineage {
    pub base_definition: Option<String>,
    pub base_id: Option<String>,
    pub derivation: Option<Derivation>,
    pub type_name: Option<String>,
}

/// Intermediate representation of a StructureDefinition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub id: String,
    pub url: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub status: Option<String>,
    pub kind: ResourceKind,
    pub fhir_type: Option<String>,
    pub date: Option<String>,
    pub lineage: ProfileLineage,
    pub elements: Vec<ElementDefinition>,
    pub extensions: Vec<ExtensionDefinition>,
    pub invariants: Vec<InvariantDefinition>,
}

impl ResourceDefinition {
    pub fn sort(&mut self) {
        self.elements
            .sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)));
        self.extensions
            .sort_by(|a, b| a.url.cmp(&b.url).then_with(|| a.id.cmp(&b.id)));
        self.invariants.sort_by(|a, b| a.key.cmp(&b.key));

        for element in &mut self.elements {
            element.sort();
        }
    }
}

/// Representation of a StructureDefinition element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinition {
    pub id: String,
    pub path: String,
    pub slice_name: Option<String>,
    pub short: Option<String>,
    pub definition: Option<String>,
    pub comment: Option<String>,
    pub requirements: Option<String>,
    pub cardinality: ElementCardinality,
    pub types: Vec<ElementType>,
    pub content_reference: Option<String>,
    pub binding: Option<BindingDefinition>,
    pub invariants: Vec<String>,
    pub fixed: Option<serde_json::Value>,
    pub pattern: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub example_values: Vec<serde_json::Value>,
    pub must_support: bool,
    pub is_summary: bool,
    pub slicing: Option<SlicingInfo>,
    pub extension: Vec<ExtensionInstance>,
    pub additional_fields: IndexMap<String, serde_json::Value>,
}

impl ElementDefinition {
    pub fn sort(&mut self) {
        self.types.sort_by(|a, b| a.code.cmp(&b.code));
        self.invariants.sort();
        self.extension.sort_by(|a, b| a.url.cmp(&b.url));
        if let Some(binding) = &mut self.binding {
            binding.sort();
        }
    }
}

/// Element cardinality constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementCardinality {
    pub min: u32,
    pub max: ElementMax,
}

/// Maximum cardinality representation (finite or unbounded).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementMax {
    Finite(u32),
    Unbounded,
}

/// Type reference for an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    pub code: String,
    pub profiles: Vec<String>,
    pub target_profiles: Vec<String>,
    pub aggregation: Vec<String>,
    pub versioning: Option<String>,
}

/// Binding information for coded elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingDefinition {
    pub strength: BindingStrength,
    pub value_set: Option<String>,
    pub description: Option<String>,
    pub additional: IndexMap<String, serde_json::Value>,
}

impl BindingDefinition {
    fn sort(&mut self) {
        // Additional map stays in insertion order via IndexMap
    }
}

/// Binding strength enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingStrength {
    Required,
    Extensible,
    Preferred,
    Example,
}

impl BindingStrength {
    pub fn from_fhir(value: &str) -> Option<Self> {
        match value {
            "required" => Some(Self::Required),
            "extensible" => Some(Self::Extensible),
            "preferred" => Some(Self::Preferred),
            "example" => Some(Self::Example),
            _ => None,
        }
    }
}

/// Slicing metadata for a repeating element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicingInfo {
    pub discriminators: Vec<SliceDiscriminator>,
    pub ordered: bool,
    pub rules: String,
}

/// Discriminator used in slicing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceDiscriminator {
    pub discriminator_type: String,
    pub path: String,
}

/// Extension instance attached to an element definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInstance {
    pub url: String,
    pub value: Option<serde_json::Value>,
}

/// IR representation of an extension definition (StructureDefinition kind = "complex-type").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDefinition {
    pub id: String,
    pub url: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub context: Vec<ExtensionContext>,
    pub elements: Vec<ElementDefinition>,
}

impl ExtensionDefinition {
    pub fn sort(&mut self) {
        self.context.sort_by(|a, b| a.context.cmp(&b.context));
        self.elements
            .sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)));
        for element in &mut self.elements {
            element.sort();
        }
    }
}

/// Context in which an extension may appear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionContext {
    pub context: String,
    pub context_type: String,
}

/// Invariant definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantDefinition {
    pub key: String,
    pub severity: Option<String>,
    pub human: Option<String>,
    pub expression: Option<String>,
    pub xpath: Option<String>,
    pub additional: IndexMap<String, serde_json::Value>,
}
