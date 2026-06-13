pub mod profile;
pub mod slicing;
pub mod terminology;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub use profile::{
    ExtensionValueType, FixedValue, MustSupportElement, ProfileDefinition, ProfileExtension,
};
pub use slicing::{SliceInfo, SlicePattern, detect_slices};
pub use terminology::{infer_codesystem_url_from_valueset, url_segment_to_pascal_name};

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
    /// Flat list of elements before tree building (preserves slice information)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flat_elements: Vec<ElementDefinition>,
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

    // Hierarchical structure fields
    /// Child elements in the element tree
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ElementDefinition>,
    /// Path to parent element (e.g., "Patient.contact" has parent "Patient")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_path: Option<String>,
    /// Nesting depth (0 for root-level elements like "Patient", 1 for "Patient.name", etc.)
    #[serde(default)]
    pub depth: usize,
    /// True if this is a BackboneElement (complex type with no type.code but has children)
    #[serde(default)]
    pub is_backbone: bool,
}

impl ElementDefinition {
    pub fn sort(&mut self) {
        self.types.sort_by(|a, b| a.code.cmp(&b.code));
        self.invariants.sort();
        self.extension.sort_by(|a, b| a.url.cmp(&b.url));
        if let Some(binding) = &mut self.binding {
            binding.sort();
        }
        // Sort children recursively
        self.children
            .sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)));
        for child in &mut self.children {
            child.sort();
        }
    }

    /// True if this element is a FHIR choice placeholder (`value[x]`, `effective[x]`).
    pub fn is_choice(&self) -> bool {
        self.path
            .rsplit('.')
            .next()
            .unwrap_or(self.path.as_str())
            .ends_with("[x]")
    }
}

/// Build the wire member name for one FHIR choice (`value[x]`) variant, per the
/// FHIR rule: the base element name + the type code with its first letter
/// upper-cased (`value` + `dateTime` -> `valueDateTime`, `value` + `Quantity` ->
/// `valueQuantity`, `value` + `CodeableConcept` -> `valueCodeableConcept`).
///
/// `base` is the choice element name with the `[x]` suffix already removed.
/// Language-neutral: this is the FHIR JSON element-naming rule, shared by every
/// backend.
pub fn choice_variant_name(base: &str, type_code: &str) -> String {
    let mut chars = type_code.chars();
    let upper_first = match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    };
    format!("{base}{upper_first}")
}

/// Expand FHIR choice (`value[x]`) elements into one single-typed element per
/// allowed type, named per [`choice_variant_name`]. This is the wire-faithful
/// lowering: FHIR JSON serializes `value[x]` as `valueQuantity`/`valueString`/…
/// (there is no `value` key on the wire), so each variant becomes its own typed
/// element. Language-neutral — every backend consumes the expanded elements.
///
/// Elements that already carry expanded children (a snapshot that pre-expanded
/// the choice) and non-choice elements pass through unchanged. Variant order is
/// deterministic (sorted by the emitted member name).
pub fn expand_choice_elements(elements: Vec<ElementDefinition>) -> Vec<ElementDefinition> {
    let mut out = Vec::with_capacity(elements.len());
    for element in elements {
        let last_segment = element
            .path
            .rsplit('.')
            .next()
            .unwrap_or(element.path.as_str());

        let is_choice = last_segment.ends_with("[x]");
        if is_choice && element.children.is_empty() && !element.types.is_empty() {
            let base = last_segment.trim_end_matches("[x]");
            // Path prefix up to and including the trailing '.'.
            let prefix = &element.path[..element.path.len() - last_segment.len()];

            let mut variants: Vec<ElementDefinition> = element
                .types
                .iter()
                .map(|ty| {
                    let member = choice_variant_name(base, &ty.code);
                    let mut variant = element.clone();
                    variant.path = format!("{prefix}{member}");
                    variant.types = vec![ty.clone()];
                    variant
                })
                .collect();
            variants.sort_by(|a, b| a.path.cmp(&b.path));
            out.extend(variants);
        } else {
            out.push(element);
        }
    }
    out
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

/// Kind of discriminator used in slicing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DiscriminatorType {
    /// Discriminate by element type (type.code)
    Type,
    /// Discriminate by fixed value
    Value,
    /// Discriminate by pattern match
    Pattern,
    /// Discriminate by profile URL
    Profile,
    /// Discriminate by exists (has value or not)
    Exists,
}

impl DiscriminatorType {
    pub fn from_fhir(value: &str) -> Option<Self> {
        match value {
            "type" => Some(Self::Type),
            "value" => Some(Self::Value),
            "pattern" => Some(Self::Pattern),
            "profile" => Some(Self::Profile),
            "exists" => Some(Self::Exists),
            _ => None,
        }
    }
}

/// Discriminator used in slicing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceDiscriminator {
    pub discriminator_type: String,
    pub path: String,
    /// Parsed discriminator type for type-safe operations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<DiscriminatorType>,
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
    /// Cardinality constraints for the extension value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<ElementCardinality>,
    /// Element containing the actual extension value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_element: Option<String>,
    /// Whether this extension uses a complex type (has children) or simple value
    #[serde(default)]
    pub is_complex: bool,
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
    /// Whether this context is invariant (required for all versions)
    #[serde(default)]
    pub invariant: bool,
}

/// FHIRPath expression used in invariants and other constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FHIRPathExpression {
    /// The FHIRPath expression string
    pub expression: String,
    /// True if this expression is evaluable at runtime
    #[serde(default)]
    pub is_evaluable: bool,
    /// Complexity level: "simple" (single property check), "moderate" (basic operators), "complex" (functions/recursion)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<String>,
}

/// Invariant definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantDefinition {
    pub key: String,
    pub severity: Option<String>,
    pub human: Option<String>,
    pub expression: Option<String>,
    pub xpath: Option<String>,
    /// Parsed FHIRPath expression metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhirpath: Option<FHIRPathExpression>,
    pub additional: IndexMap<String, serde_json::Value>,
}

#[cfg(test)]
mod choice_tests {
    use super::*;

    fn choice_element(path: &str, type_codes: &[&str]) -> ElementDefinition {
        ElementDefinition {
            id: path.to_string(),
            path: path.to_string(),
            slice_name: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Finite(1),
            },
            types: type_codes
                .iter()
                .map(|code| ElementType {
                    code: (*code).to_string(),
                    profiles: Vec::new(),
                    target_profiles: Vec::new(),
                    aggregation: Vec::new(),
                    versioning: None,
                })
                .collect(),
            content_reference: None,
            binding: None,
            invariants: Vec::new(),
            fixed: None,
            pattern: None,
            default_value: None,
            example_values: Vec::new(),
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: Vec::new(),
            additional_fields: IndexMap::new(),
            children: Vec::new(),
            parent_path: None,
            depth: 0,
            is_backbone: false,
        }
    }

    #[test]
    fn choice_variant_name_uppercases_type_code() {
        assert_eq!(choice_variant_name("value", "dateTime"), "valueDateTime");
        assert_eq!(choice_variant_name("value", "Quantity"), "valueQuantity");
        assert_eq!(
            choice_variant_name("value", "CodeableConcept"),
            "valueCodeableConcept"
        );
    }

    #[test]
    fn expand_emits_one_variant_per_type() {
        let expanded = expand_choice_elements(vec![choice_element(
            "Patient.deceased[x]",
            &["boolean", "dateTime"],
        )]);
        let paths: Vec<&str> = expanded.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["Patient.deceasedBoolean", "Patient.deceasedDateTime"]
        );
    }

    /// The TS at-most-one Zod refinement groups variants by their shared origin
    /// id, so expansion must keep the original `[x]` id on every variant.
    #[test]
    fn expanded_variants_share_the_choice_id() {
        let expanded = expand_choice_elements(vec![choice_element(
            "Patient.deceased[x]",
            &["boolean", "dateTime"],
        )]);
        assert_eq!(expanded.len(), 2);
        for variant in &expanded {
            assert_eq!(variant.id, "Patient.deceased[x]");
            assert!(!variant.path.contains("[x]"));
        }
    }

    #[test]
    fn non_choice_elements_pass_through() {
        let expanded = expand_choice_elements(vec![choice_element("Patient.active", &["boolean"])]);
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].path, "Patient.active");
    }
}
