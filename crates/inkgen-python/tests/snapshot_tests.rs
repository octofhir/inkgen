//! Python backend snapshot tests, one per FHIR construct.
//!
//! The base IR is the shared offline backend-consumer fixture (a minimal
//! Patient package). Constructs the fixture does not exercise — the full
//! primitive set, cardinalities, backbones, ValueSet bindings, XOR invariants
//! and cross-module cycles — are injected by mutating the fixture JSON before
//! deserialization, so every test runs the full generation pipeline.

use inkgen_core::{Backend, GenerationOutput, PackageIr};
use inkgen_python::{PythonGenerator, PythonGeneratorConfig};
use insta::assert_snapshot;
use serde_json::{Value, json};

const PATIENT_URL: &str = "http://example.org/fhir/StructureDefinition/Patient";
const EXTENSION_URL: &str = "http://example.org/fhir/StructureDefinition/Extension";
const GENDER_VS_URL: &str = "http://example.org/fhir/ValueSet/administrative-gender";

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/backend-consumer/package-ir.json"
    ))
    .expect("fixture JSON")
}

/// Build an element JSON object matching the fixture's element shape.
fn element(path: &str, min: u32, max: Value, types: &[&str]) -> Value {
    json!({
        "id": path,
        "path": path,
        "slice_name": null,
        "short": null,
        "definition": null,
        "comment": null,
        "requirements": null,
        "cardinality": { "min": min, "max": max },
        "types": types.iter().map(|code| json!({
            "code": code,
            "profiles": [],
            "target_profiles": [],
            "aggregation": [],
            "versioning": null,
        })).collect::<Vec<_>>(),
        "content_reference": null,
        "binding": null,
        "invariants": [],
        "fixed": null,
        "pattern": null,
        "default_value": null,
        "example_values": [],
        "must_support": false,
        "is_summary": false,
        "slicing": null,
        "extension": [],
        "additional_fields": {},
    })
}

fn finite(n: u32) -> Value {
    json!({ "finite": n })
}

fn unbounded() -> Value {
    json!("unbounded")
}

/// Replace the Patient element tree, keeping `flat_elements` consistent.
fn set_elements(ir: &mut Value, elements: Vec<Value>) {
    let mut flat = Vec::new();
    fn flatten(el: &Value, out: &mut Vec<Value>) {
        let mut copy = el.clone();
        let children = copy
            .as_object_mut()
            .and_then(|o| o.remove("children"))
            .and_then(|c| c.as_array().cloned())
            .unwrap_or_default();
        out.push(copy);
        for child in &children {
            flatten(child, out);
        }
    }
    for el in &elements {
        flatten(el, &mut flat);
    }
    let def = &mut ir["types"][PATIENT_URL];
    def["elements"] = Value::Array(elements);
    def["flat_elements"] = Value::Array(flat);
}

/// Add a structure definition to the fixture package.
fn add_type(ir: &mut Value, url: &str, id: &str, kind: &str, elements: Vec<Value>) -> Value {
    let def = json!({
        "id": id,
        "url": url,
        "name": id,
        "title": null,
        "description": null,
        "version": null,
        "status": "active",
        "kind": kind,
        "fhir_type": id,
        "date": null,
        "lineage": {
            "base_definition": null,
            "base_id": null,
            "derivation": null,
            "type_name": null,
        },
        "elements": elements,
        "extensions": [],
        "invariants": [],
    });
    ir["types"][url] = def.clone();
    ir["canonical_index"][id] = json!(url);
    def
}

fn binding(element: &mut Value, strength: &str, value_set: &str) {
    element["binding"] = json!({
        "strength": strength,
        "value_set": value_set,
        "description": null,
        "additional": {},
    });
}

fn add_gender_value_set(ir: &mut Value, url: &str) {
    ir["value_sets"][url] = json!({
        "resourceType": "ValueSet",
        "url": url,
        "name": "AdministrativeGender",
        "expansion": {
            "contains": [
                { "code": "male", "display": "Male" },
                { "code": "female", "display": "Female" },
                { "code": "other", "display": "Other" },
                { "code": "unknown", "display": "Unknown" },
            ]
        }
    });
}

/// Run the full pipeline, asserting determinism.
fn generate(ir: Value) -> GenerationOutput {
    let ir: PackageIr = serde_json::from_value(ir).expect("fixture deserializes to PackageIr");
    let backend = PythonGenerator::default();
    let output = backend.generate(&ir).expect("generation succeeds");
    assert_eq!(
        output.files,
        backend.generate(&ir).expect("second run").files,
        "generation must be deterministic"
    );
    output
}

/// Fetch the unique generated file whose path ends with `suffix`.
fn file<'a>(output: &'a GenerationOutput, suffix: &str) -> &'a str {
    let matches: Vec<&String> = output
        .files
        .keys()
        .filter(|path| path.ends_with(suffix))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one file ending in {suffix}, got {matches:?}"
    );
    output.files[matches[0]].as_str()
}

#[test]
fn primitives() {
    let mut ir = fixture();
    let codes = [
        "string",
        "boolean",
        "integer",
        "decimal",
        "date",
        "dateTime",
        "instant",
        "time",
        "uri",
        "url",
        "canonical",
        "code",
        "id",
        "markdown",
        "base64Binary",
        "oid",
        "uuid",
        "xhtml",
        "positiveInt",
        "unsignedInt",
        "integer64",
    ];
    let elements = codes
        .iter()
        .map(|code| {
            let name = format!("p{code}");
            element(&format!("Patient.{name}"), 0, finite(1), &[code])
        })
        .collect();
    set_elements(&mut ir, elements);
    let out = generate(ir);
    assert_snapshot!(file(&out, "patient.py"));
}

#[test]
fn cardinalities() {
    let mut ir = fixture();
    set_elements(
        &mut ir,
        vec![
            element("Patient.optionalScalar", 0, finite(1), &["string"]),
            element("Patient.requiredScalar", 1, finite(1), &["boolean"]),
            element("Patient.optionalList", 0, unbounded(), &["string"]),
            element("Patient.requiredList", 1, unbounded(), &["integer"]),
            element("Patient.boundedList", 0, finite(3), &["code"]),
            element("Patient.minTwoList", 2, unbounded(), &["id"]),
        ],
    );
    let out = generate(ir);
    assert_snapshot!(file(&out, "patient.py"));
}

#[test]
fn choice() {
    let mut ir = fixture();
    let mut required = element(
        "Patient.observed[x]",
        1,
        finite(1),
        &["boolean", "dateTime", "Quantity"],
    );
    required["id"] = json!("Patient.observed[x]");
    set_elements(
        &mut ir,
        vec![
            element(
                "Patient.deceased[x]",
                0,
                finite(1),
                &["boolean", "dateTime"],
            ),
            required,
        ],
    );
    let out = generate(ir);
    assert_snapshot!(file(&out, "patient.py"));
}

#[test]
fn backbone() {
    let mut ir = fixture();
    let mut root = element("Patient", 0, unbounded(), &[]);
    let mut contact = element("Patient.contact", 0, unbounded(), &["BackboneElement"]);
    let mut organization = element(
        "Patient.contact.organization",
        0,
        finite(1),
        &["BackboneElement"],
    );
    organization["children"] = json!([element(
        "Patient.contact.organization.label",
        0,
        finite(1),
        &["string"]
    ),]);
    let mut items = element("Patient.item", 0, unbounded(), &["BackboneElement"]);
    let mut nested = element("Patient.item.item", 0, unbounded(), &[]);
    nested["content_reference"] = json!("#Patient.item");
    items["children"] = json!([
        element("Patient.item.text", 1, finite(1), &["string"]),
        nested,
    ]);
    contact["children"] = json!([
        element("Patient.contact.name", 0, finite(1), &["string"]),
        organization,
    ]);
    root["children"] = json!([contact, items]);
    set_elements(&mut ir, vec![root]);
    let out = generate(ir);
    assert_snapshot!(file(&out, "patient.py"));
}

#[test]
fn enum_binding() {
    let mut ir = fixture();
    add_gender_value_set(&mut ir, GENDER_VS_URL);
    let mut gender = element("Patient.gender", 0, finite(1), &["code"]);
    // Version-pinned canonical: the pin must be stripped during lowering.
    binding(&mut gender, "required", &format!("{GENDER_VS_URL}|4.0.1"));
    let mut status = element("Patient.status", 1, finite(1), &["code"]);
    binding(&mut status, "extensible", GENDER_VS_URL);
    let mut advisory = element("Patient.note", 0, finite(1), &["code"]);
    binding(&mut advisory, "preferred", GENDER_VS_URL);
    set_elements(&mut ir, vec![gender, status, advisory]);
    let out = generate(ir);
    assert_snapshot!(file(&out, "patient.py"));
}

#[test]
fn enum_aliases() {
    let mut ir = fixture();
    add_gender_value_set(&mut ir, GENDER_VS_URL);
    let mut gender = element("Patient.gender", 0, finite(1), &["code"]);
    binding(&mut gender, "required", GENDER_VS_URL);
    set_elements(&mut ir, vec![gender]);
    let out = generate(ir);
    assert_snapshot!(file(&out, "enums.py"));
}

#[test]
fn invariant_ext1() {
    let mut ir = fixture();
    let mut extension = element("Extension.extension", 0, unbounded(), &["Extension"]);
    extension["id"] = json!("Extension.extension");
    let elements = vec![
        element("Extension.url", 1, finite(1), &["uri"]),
        extension,
        element("Extension.value[x]", 0, finite(1), &["boolean", "string"]),
    ];
    let mut def = add_type(
        &mut ir,
        EXTENSION_URL,
        "Extension",
        "complex_type",
        elements,
    );
    def["invariants"] = json!([
        {
            "key": "ext-1",
            "severity": "error",
            "human": "Must have either extensions or value[x], not both",
            "expression": "extension.exists() != value.exists()",
            "xpath": null,
            "additional": {},
        },
        {
            "key": "ele-1",
            "severity": "error",
            "human": "All FHIR elements must have a @value or children",
            "expression": "hasValue() or (children().count() > id.count())",
            "xpath": null,
            "additional": {},
        },
    ]);
    ir["types"][EXTENSION_URL] = def;
    let out = generate(ir);
    assert_snapshot!(file(&out, "extension.py"));
}

/// Two logical types referencing each other: forward refs must stay lazy and
/// `__init__.py` must rebuild every model after all imports.
fn cycle_ir() -> Value {
    let mut ir = fixture();
    add_type(
        &mut ir,
        "http://example.org/fhir/StructureDefinition/Alpha",
        "Alpha",
        "logical",
        vec![element("Alpha.beta", 0, finite(1), &["Beta"])],
    );
    add_type(
        &mut ir,
        "http://example.org/fhir/StructureDefinition/Beta",
        "Beta",
        "logical",
        vec![
            element("Beta.alpha", 0, finite(1), &["Alpha"]),
            element("Beta.self", 0, unbounded(), &["Beta"]),
        ],
    );
    ir
}

#[test]
fn cycle_forward_refs() {
    let out = generate(cycle_ir());
    assert_snapshot!(file(&out, "alpha.py"));
}

#[test]
fn cycle_init_rebuild() {
    let out = generate(cycle_ir());
    assert_snapshot!(file(&out, "__init__.py"));
}

/// A type whose snake_case stem collides with a Python builtin (`List` ->
/// `list`) must be renamed so the submodule cannot shadow the builtin in the
/// package namespace where `model_rebuild()` evaluates `list[...]`.
#[test]
fn builtin_shadowing_module_rename() {
    let mut ir = fixture();
    add_type(
        &mut ir,
        "http://example.org/fhir/StructureDefinition/List",
        "List",
        "logical",
        vec![element("List.entry", 0, unbounded(), &["string"])],
    );
    let out = generate(ir);
    assert_snapshot!(file(&out, "list_.py"));
    assert_snapshot!(file(&out, "__init__.py"));
}

/// The generated package directory is configurable; relative imports inside
/// the package are name-independent, so only the file paths change.
#[test]
fn custom_package_name() {
    let ir: PackageIr = serde_json::from_value(fixture()).expect("fixture deserializes");
    let backend = PythonGenerator::new(
        PythonGeneratorConfig::new("generated".into()).with_package_name("fhir_r4"),
    );
    let out = backend.generate(&ir).expect("generation succeeds");
    assert!(out.files.keys().all(|p| p.starts_with("fhir_r4/")));
    assert!(out.files.contains_key("fhir_r4/__init__.py"));
    assert!(out.files.contains_key("fhir_r4/_runtime.py"));
    assert!(out.files.contains_key("fhir_r4/patient.py"));
    assert!(!out.files.contains_key("fhir/__init__.py"));
}

#[test]
fn invalid_package_name_is_rejected() {
    let ir: PackageIr = serde_json::from_value(fixture()).expect("fixture deserializes");
    for bad in ["bad-name", "4fhir", "class", ""] {
        let backend = PythonGenerator::new(
            PythonGeneratorConfig::new("generated".into()).with_package_name(bad),
        );
        assert!(
            backend.generate(&ir).is_err(),
            "expected rejection of package name {bad:?}"
        );
    }
}
