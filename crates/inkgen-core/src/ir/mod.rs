//! Language-neutral intermediate representation of FHIR StructureDefinitions,
//! shared by all code-generation backends.

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
    /// A FHIR primitive datatype (e.g. `string`, `boolean`).
    PrimitiveType,
    /// A FHIR complex datatype (e.g. `HumanName`, `Extension`).
    ComplexType,
    /// A FHIR resource (e.g. `Patient`).
    Resource,
    /// A logical model, outside the resource/datatype hierarchy.
    Logical,
}

impl ResourceKind {
    /// Parses a FHIR `StructureDefinition.kind` code; `None` for unknown values.
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
    /// A profile constraining an existing definition.
    Constraint,
    /// A new definition specializing a base type.
    Specialization,
}

impl Derivation {
    /// Parses a FHIR `StructureDefinition.derivation` code; `None` for unknown values.
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
    /// Canonical URL of the definition this profile derives from.
    pub base_definition: Option<String>,
    /// Id of the base StructureDefinition, if known.
    pub base_id: Option<String>,
    /// How this definition derives from its base (constraint or specialization).
    pub derivation: Option<Derivation>,
    /// FHIR type the definition constrains or specializes (`StructureDefinition.type`).
    pub type_name: Option<String>,
}

/// Intermediate representation of a StructureDefinition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// StructureDefinition id (resource identifier within the package).
    pub id: String,
    /// Canonical URL of the StructureDefinition.
    pub url: String,
    /// Computer-friendly name (`StructureDefinition.name`).
    pub name: Option<String>,
    /// Human-friendly title.
    pub title: Option<String>,
    /// Human-readable description of the definition.
    pub description: Option<String>,
    /// Business version of the StructureDefinition.
    pub version: Option<String>,
    /// Publication status (e.g. `draft`, `active`, `retired`).
    pub status: Option<String>,
    /// Whether this is a primitive type, complex type, resource or logical model.
    pub kind: ResourceKind,
    /// FHIR type code the definition describes (`StructureDefinition.type`).
    pub fhir_type: Option<String>,
    /// Publication or last-revision date as recorded in the definition.
    pub date: Option<String>,
    /// Base definition and derivation metadata.
    pub lineage: ProfileLineage,
    /// Element tree built from the snapshot elements.
    pub elements: Vec<ElementDefinition>,
    /// Flat list of elements before tree building (preserves slice information)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flat_elements: Vec<ElementDefinition>,
    /// Extension definitions declared by this StructureDefinition.
    pub extensions: Vec<ExtensionDefinition>,
    /// Constraints (FHIRPath invariants) declared on the definition.
    pub invariants: Vec<InvariantDefinition>,
}

impl ResourceDefinition {
    /// Sorts elements, extensions and invariants (recursively) for deterministic
    /// output.
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

    /// True if this StructureDefinition defines an Extension (a `constraint`
    /// derivation on the `Extension` type). FHIR-neutral — any backend that
    /// emits extension types needs this exact check.
    pub fn is_extension_definition(&self) -> bool {
        matches!(self.lineage.derivation, Some(Derivation::Constraint))
            && self.fhir_type.as_deref() == Some("Extension")
    }

    /// True if this extension is complex, i.e. it carries sub-extensions
    /// (`Extension.extension` slices) rather than a single `value[x]`. Reads the
    /// flat element list, which preserves slices. FHIR-neutral.
    pub fn extension_is_complex(&self) -> bool {
        self.flat_elements
            .iter()
            .any(|elem| elem.path == "Extension.extension" && elem.slice_name.is_some())
    }

    /// The FHIR type code of a simple extension's single-typed `Extension.value[x]`
    /// (e.g. `dateTime`, `Reference`), or `None` when the extension is complex or
    /// the value is untyped / a multi-type choice. This is the wire-faithful type
    /// code; mapping it to a language type stays in the backend. FHIR-neutral.
    pub fn simple_extension_value_type_code(&self) -> Option<String> {
        if self.extension_is_complex() {
            return None;
        }
        self.flat_elements
            .iter()
            .find(|elem| elem.path == "Extension.value[x]")
            .filter(|elem| elem.types.len() == 1)
            .and_then(|elem| elem.types.first())
            .map(|t| t.code.clone())
    }
}

/// Representation of a StructureDefinition element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinition {
    /// Element id, unique within the StructureDefinition (e.g. `Patient.name`).
    pub id: String,
    /// Dot-separated FHIR path of the element (e.g. `Patient.birthDate`).
    pub path: String,
    /// Slice name when this element defines a slice of a repeating element.
    pub slice_name: Option<String>,
    /// Concise human-readable summary shown in specifications.
    pub short: Option<String>,
    /// Full human-readable definition of the element.
    pub definition: Option<String>,
    /// Additional explanatory notes about the element.
    pub comment: Option<String>,
    /// Why the element exists / implementation guidance.
    pub requirements: Option<String>,
    /// Minimum and maximum occurrence constraints.
    pub cardinality: ElementCardinality,
    /// Allowed datatypes; multiple entries form a choice (`[x]`) element.
    pub types: Vec<ElementType>,
    /// `#`-prefixed element id whose content definition this element reuses.
    pub content_reference: Option<String>,
    /// Value set binding for coded elements.
    pub binding: Option<BindingDefinition>,
    /// Keys of invariants that apply to this element.
    pub invariants: Vec<String>,
    /// Exact value the element must have (`fixed[x]`), as raw JSON.
    pub fixed: Option<serde_json::Value>,
    /// Value pattern the element must match (`pattern[x]`), as raw JSON.
    pub pattern: Option<serde_json::Value>,
    /// Default value when the element is absent, as raw JSON.
    pub default_value: Option<serde_json::Value>,
    /// Example values illustrating the element, as raw JSON.
    pub example_values: Vec<serde_json::Value>,
    /// Whether implementations must support the element (`mustSupport`).
    pub must_support: bool,
    /// Whether the element is part of the resource summary set (`isSummary`).
    pub is_summary: bool,
    /// Slicing rules when this element introduces slices.
    pub slicing: Option<SlicingInfo>,
    /// Extension instances declared on the element itself.
    pub extension: Vec<ExtensionInstance>,
    /// Unrecognized element properties preserved verbatim, in insertion order.
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
    /// Sorts types, invariants, extensions and child elements (recursively) for
    /// deterministic output.
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

    /// True when every declared type is a FHIR primitive datatype, so the
    /// element admits a sibling primitive-extension element (`_x`, of type
    /// `Element`) in the FHIR wire format. Primitive classification comes from
    /// the package's own `StructureDefinition` kinds (via [`CanonicalTypeMap`](crate::CanonicalTypeMap)) —
    /// not a hardcoded list — so it tracks whatever FHIR version is loaded.
    /// FHIR-neutral: every backend makes the same call; only the rendering of
    /// `_x` is language-specific.
    pub fn is_primitive(&self, type_map: &crate::canonical_map::CanonicalTypeMap) -> bool {
        !self.types.is_empty()
            && self
                .types
                .iter()
                .all(|t| type_map.is_primitive_code(&t.code))
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
                    // Each variant is individually optional on the wire: a choice
                    // serializes at most one member, so a required choice
                    // (min >= 1) must not force every member. The "at least one"
                    // constraint is enforced per choice group by the backend, not
                    // per member. Without this a required value[x] would demand
                    // all variants simultaneously.
                    variant.cardinality.min = 0;
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
    /// Minimum number of occurrences (0 means optional).
    pub min: u32,
    /// Maximum number of occurrences, finite or unbounded (`*`).
    pub max: ElementMax,
}

/// Maximum cardinality representation (finite or unbounded).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementMax {
    /// A fixed upper bound on occurrences.
    Finite(u32),
    /// No upper bound (FHIR `*`).
    Unbounded,
}

/// Type reference for an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    /// FHIR datatype or resource code (e.g. `string`, `Reference`).
    pub code: String,
    /// Canonical URLs of profiles the type must conform to.
    pub profiles: Vec<String>,
    /// Canonical URLs allowed as `Reference` targets.
    pub target_profiles: Vec<String>,
    /// Allowed reference aggregation modes (`contained`, `referenced`, `bundled`).
    pub aggregation: Vec<String>,
    /// Reference versioning policy (`either`, `independent` or `specific`).
    pub versioning: Option<String>,
}

/// Binding information for coded elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingDefinition {
    /// How strictly the bound value set must be followed.
    pub strength: BindingStrength,
    /// Canonical URL of the bound ValueSet.
    pub value_set: Option<String>,
    /// Human-readable explanation of the binding.
    pub description: Option<String>,
    /// Unrecognized binding properties preserved verbatim, in insertion order.
    pub additional: IndexMap<String, serde_json::Value>,
}

impl BindingDefinition {
    fn sort(&mut self) {
        // Additional map stays in insertion order via IndexMap
    }

    /// True when this binding constrains the value space enough to lower to a
    /// closed/enumerated type (the bound ValueSet). Per FHIR, `required` and
    /// `extensible` bindings enumerate; `preferred`/`example` are advisory and
    /// lower to the base primitive (`string`/`code`). FHIR-neutral — every
    /// backend (TS union, Rust enum, Python `Literal`) shares this decision.
    pub fn is_enumerable(&self) -> bool {
        matches!(
            self.strength,
            BindingStrength::Required | BindingStrength::Extensible
        )
    }
}

/// How a coded element's binding lowers to a type.
///
/// FHIR-neutral: every backend makes the same call from the same rule
/// (`required`/`extensible` + a resolvable ValueSet → enumerate; otherwise keep
/// the base primitive), then maps the canonical name to its own identifier.
/// Resolved once by [`crate::package_ir::PackageIr::lower_binding`] against the
/// package's resolved ValueSets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingLowering {
    /// The binding enumerates (required/extensible) and its ValueSet resolves to
    /// a concrete set of codes — lower to a closed type named after the ValueSet.
    Enumerated {
        /// Canonical URL of the bound ValueSet.
        value_set_url: String,
        /// FHIR-neutral canonical type name (ValueSet `name`, else `id`, else
        /// `UnknownValueSet`). The backend applies its own identifier casing and
        /// sanitization — this string is deliberately not pre-cased.
        type_name: String,
    },
    /// The element keeps its base primitive type. `reason` is provenance for the
    /// `explain` surface.
    Open {
        /// Why the binding stayed open.
        reason: OpenReason,
    },
}

/// Why a binding did not lower to an enumerated type (provenance for `explain`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenReason {
    /// Binding is `preferred`/`example` — advisory only, no closed type.
    AdvisoryStrength,
    /// Strong binding, but the bound ValueSet could not be resolved to codes.
    ValueSetUnresolved,
    /// No binding, or the binding carries no ValueSet URL.
    NoValueSet,
}

/// Binding strength enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingStrength {
    /// Only codes from the bound value set are allowed.
    Required,
    /// Bound codes should be used, but other codes are permitted when none fit.
    Extensible,
    /// Bound codes are recommended for interoperability but not enforced.
    Preferred,
    /// Bound codes are merely examples of the intended concept.
    Example,
}

impl BindingStrength {
    /// Parses a FHIR binding strength code; `None` for unknown values.
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
    /// Paths and types used to tell slices apart.
    pub discriminators: Vec<SliceDiscriminator>,
    /// Whether the order of slices is significant.
    pub ordered: bool,
    /// Slicing rules (`closed`, `open` or `openAtEnd`) as recorded in the definition.
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
    /// Parses a FHIR discriminator type code; `None` for unknown values.
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
    /// Raw discriminator type code as written in the definition.
    pub discriminator_type: String,
    /// Element path, relative to the sliced element, used for discrimination.
    pub path: String,
    /// Parsed discriminator type for type-safe operations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<DiscriminatorType>,
}

/// Extension instance attached to an element definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInstance {
    /// Canonical URL identifying the extension.
    pub url: String,
    /// Extension value as raw JSON, if present.
    pub value: Option<serde_json::Value>,
}

/// IR representation of an extension definition (StructureDefinition kind = "complex-type").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDefinition {
    /// StructureDefinition id of the extension.
    pub id: String,
    /// Canonical URL identifying the extension.
    pub url: String,
    /// Computer-friendly extension name.
    pub name: Option<String>,
    /// Human-readable description of the extension.
    pub description: Option<String>,
    /// Contexts in which the extension is allowed to appear.
    pub context: Vec<ExtensionContext>,
    /// Elements describing the extension's structure.
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
    /// Sorts contexts and elements (recursively) for deterministic output.
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
    /// Context expression (element path, type or FHIRPath) where the extension applies.
    pub context: String,
    /// Kind of context (`element`, `datatype`, `fhirpath` or `resource`).
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
    /// Unique invariant key within the definition (e.g. `ele-1`).
    pub key: String,
    /// Severity of a violation (`error` or `warning`).
    pub severity: Option<String>,
    /// Human-readable description of the constraint.
    pub human: Option<String>,
    /// FHIRPath expression that must evaluate to true for valid instances.
    pub expression: Option<String>,
    /// XPath equivalent of the constraint, if provided.
    pub xpath: Option<String>,
    /// Parsed FHIRPath expression metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhirpath: Option<FHIRPathExpression>,
    /// Unrecognized invariant properties preserved verbatim, in insertion order.
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
    fn is_primitive_uses_package_type_kinds() {
        use crate::canonical_map::{CanonicalTypeMap, TypeEntry};
        use crate::package::{PackageId, StructureKind};

        let mut map = CanonicalTypeMap::new();
        for (name, code, kind) in [
            ("Date", "date", StructureKind::PrimitiveType),
            ("Boolean", "boolean", StructureKind::PrimitiveType),
            ("HumanName", "HumanName", StructureKind::ComplexType),
        ] {
            map.insert(TypeEntry {
                canonical_url: format!("http://hl7.org/fhir/StructureDefinition/{code}"),
                type_name: name.to_string(),
                file_stem: name.to_lowercase(),
                kind,
                package: PackageId::new("hl7.fhir.r4.core", "4.0.1"),
                base_type: None,
                type_code: Some(code.to_string()),
            });
        }

        assert!(choice_element("Patient.birthDate", &["date"]).is_primitive(&map));
        assert!(!choice_element("Patient.name", &["HumanName"]).is_primitive(&map));
        // An element with no declared type is not a primitive.
        assert!(!choice_element("Patient.x", &[]).is_primitive(&map));
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
    fn expanded_variants_are_individually_optional() {
        // A required choice (min=1) must not force every variant.
        let mut required = choice_element("Q.answer[x]", &["boolean", "string"]);
        required.cardinality.min = 1;
        let expanded = expand_choice_elements(vec![required]);
        assert_eq!(expanded.len(), 2);
        for variant in &expanded {
            assert_eq!(variant.cardinality.min, 0);
        }
    }

    #[test]
    fn binding_is_enumerable_by_strength() {
        let binding = |strength| BindingDefinition {
            strength,
            value_set: Some("http://hl7.org/fhir/ValueSet/x".to_string()),
            description: None,
            additional: IndexMap::new(),
        };
        assert!(binding(BindingStrength::Required).is_enumerable());
        assert!(binding(BindingStrength::Extensible).is_enumerable());
        assert!(!binding(BindingStrength::Preferred).is_enumerable());
        assert!(!binding(BindingStrength::Example).is_enumerable());
    }

    #[test]
    fn non_choice_elements_pass_through() {
        let expanded = expand_choice_elements(vec![choice_element("Patient.active", &["boolean"])]);
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].path, "Patient.active");
    }

    fn extension_resource(
        fhir_type: &str,
        derivation: Option<Derivation>,
        flat: Vec<ElementDefinition>,
    ) -> ResourceDefinition {
        ResourceDefinition {
            id: "ext".to_string(),
            url: "http://example.org/ext".to_string(),
            name: None,
            title: None,
            description: None,
            version: None,
            status: None,
            kind: ResourceKind::ComplexType,
            fhir_type: Some(fhir_type.to_string()),
            date: None,
            lineage: ProfileLineage {
                base_definition: None,
                base_id: None,
                derivation,
                type_name: None,
            },
            elements: Vec::new(),
            flat_elements: flat,
            extensions: Vec::new(),
            invariants: Vec::new(),
        }
    }

    fn slice(path: &str, name: &str) -> ElementDefinition {
        let mut e = choice_element(path, &[]);
        e.slice_name = Some(name.to_string());
        e
    }

    #[test]
    fn is_extension_definition_requires_constraint_on_extension() {
        assert!(
            extension_resource("Extension", Some(Derivation::Constraint), vec![])
                .is_extension_definition()
        );
        assert!(
            !extension_resource("Patient", Some(Derivation::Constraint), vec![])
                .is_extension_definition()
        );
        assert!(
            !extension_resource("Extension", Some(Derivation::Specialization), vec![])
                .is_extension_definition()
        );
    }

    #[test]
    fn extension_complexity_from_sub_extension_slices() {
        let complex = extension_resource(
            "Extension",
            Some(Derivation::Constraint),
            vec![slice("Extension.extension", "race")],
        );
        assert!(complex.extension_is_complex());
        assert_eq!(complex.simple_extension_value_type_code(), None);

        let simple = extension_resource(
            "Extension",
            Some(Derivation::Constraint),
            vec![choice_element("Extension.value[x]", &["dateTime"])],
        );
        assert!(!simple.extension_is_complex());
        assert_eq!(
            simple.simple_extension_value_type_code(),
            Some("dateTime".to_string())
        );
    }

    #[test]
    fn multi_typed_extension_value_has_no_single_code() {
        let multi = extension_resource(
            "Extension",
            Some(Derivation::Constraint),
            vec![choice_element(
                "Extension.value[x]",
                &["string", "Reference"],
            )],
        );
        assert_eq!(multi.simple_extension_value_type_code(), None);
    }
}
