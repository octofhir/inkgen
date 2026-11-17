//! Valibot dialect snapshot tests, one per FHIR construct.
//!
//! The base IR is the shared offline backend-consumer fixture (a minimal
//! Patient package). Constructs the fixture does not exercise — required
//! fields, bounded arrays, ValueSet picklists, nested backbones, recursion,
//! required choices — are injected by mutating the fixture JSON before
//! deserialization, so every test runs the full generation pipeline.

use inkgen_core::{Backend, GenerationOutput, PackageIr};
use inkgen_typescript::profiles::{ConstrainedElement, FixedElement, ProfileInfo};
use inkgen_typescript::{TypescriptGenerator, TypescriptGeneratorConfig, ValidationLibrary};
use insta::assert_snapshot;
use serde_json::{Value, json};
use std::sync::Arc;
use tera::Tera;

const PATIENT_URL: &str = "http://example.org/fhir/StructureDefinition/Patient";
const STATUS_VS_URL: &str = "http://example.org/fhir/ValueSet/audit-status";

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

/// Run the full Valibot pipeline over a fixture-derived IR.
fn generate(ir: Value) -> GenerationOutput {
    let ir: PackageIr = serde_json::from_value(ir).expect("fixture deserializes to PackageIr");
    let ir = Arc::new(ir);
    let mut config =
        TypescriptGeneratorConfig::from_package_ir(ir.clone(), None, "generated".into())
            .expect("typescript config");
    config.validation_library = ValidationLibrary::Valibot;
    let backend = TypescriptGenerator::new(config);
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
    &output.files[matches[0]]
}

#[test]
fn valibot_fixture_patient() {
    let output = generate(fixture());
    assert_snapshot!("fixture_patient", file(&output, "patient.ts"));
    assert_snapshot!("fixture_package_json", file(&output, "package.json"));
}

#[test]
fn valibot_fixture_primitives() {
    let output = generate(fixture());
    assert_snapshot!("fixture_primitives", file(&output, "primitives.ts"));
}

#[test]
fn valibot_required_fields_and_bounded_arrays() {
    let mut ir = fixture();
    set_elements(
        &mut ir,
        vec![
            element("Patient.active", 1, finite(1), &["boolean"]),
            element("Patient.alias", 2, finite(5), &["string"]),
            element("Patient.birthDate", 1, finite(1), &["date"]),
            element("Patient.photo", 0, json!("unbounded"), &["base64Binary"]),
        ],
    );
    let output = generate(ir);
    assert_snapshot!("required_and_bounded", file(&output, "patient.ts"));
}

#[test]
fn valibot_valueset_picklist() {
    let mut ir = fixture();
    let mut status = element("Patient.status", 1, finite(1), &["code"]);
    status["binding"] = json!({
        "strength": "required",
        "value_set": STATUS_VS_URL,
        "description": null,
        "additional": {},
    });
    let mut elements = ir["types"][PATIENT_URL]["elements"]
        .as_array()
        .cloned()
        .unwrap();
    elements.push(status);
    set_elements(&mut ir, elements);

    ir["value_sets"][STATUS_VS_URL] = json!({
        "resourceType": "ValueSet",
        "id": "audit-status",
        "url": STATUS_VS_URL,
        "name": "AuditStatus",
        "title": "Audit Status",
        "status": "active",
        "compose": {
            "include": [{
                "system": "http://example.org/fhir/CodeSystem/audit-status",
                "concept": [
                    { "code": "draft", "display": "Draft" },
                    { "code": "final", "display": "Final" },
                ],
            }],
        },
    });
    ir["package_descriptor"]["inventory"]["value_sets"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "canonical_url": STATUS_VS_URL,
            "name": "AuditStatus",
            "resource_type": "ValueSet",
            "version": "1.0.0",
            "status": "active",
            "package": { "name": "audit.fhir", "version": "1.0.0" },
        }));

    let output = generate(ir);
    assert_snapshot!("valueset_patient", file(&output, "patient.ts"));
    assert_snapshot!("valueset_codes", file(&output, "valuesets/audit-status.ts"));
}

#[test]
fn valibot_nested_backbone() {
    let mut ir = fixture();
    let mut detail = element("Patient.contact.detail", 0, finite(1), &["BackboneElement"]);
    detail["is_backbone"] = json!(true);
    detail["depth"] = json!(2);
    detail["parent_path"] = json!("Patient.contact");
    detail["children"] = json!([element(
        "Patient.contact.detail.note",
        1,
        finite(1),
        &["string"]
    )]);
    detail["children"][0]["depth"] = json!(3);
    detail["children"][0]["parent_path"] = json!("Patient.contact.detail");

    let mut contact = element(
        "Patient.contact",
        0,
        json!("unbounded"),
        &["BackboneElement"],
    );
    contact["is_backbone"] = json!(true);
    contact["depth"] = json!(1);
    contact["parent_path"] = json!("Patient");
    contact["children"] = json!([
        element("Patient.contact.name", 0, finite(1), &["string"]),
        element("Patient.contact.rank", 0, finite(1), &["positiveInt"]),
        detail,
    ]);
    for child in contact["children"].as_array_mut().unwrap() {
        child["depth"] = json!(2);
        child["parent_path"] = json!("Patient.contact");
    }
    set_elements(&mut ir, vec![contact]);
    let output = generate(ir);
    assert_snapshot!("nested_backbone", file(&output, "patient.ts"));
}

#[test]
fn valibot_recursion() {
    let mut ir = fixture();
    let mut contact = element("Patient.contact", 0, finite(1), &["BackboneElement"]);
    contact["is_backbone"] = json!(true);
    contact["depth"] = json!(1);
    contact["children"] = json!([element("Patient.contact.name", 0, finite(1), &["string"])]);
    contact["children"][0]["depth"] = json!(2);

    let mut prior = element("Patient.priorContact", 0, finite(1), &[]);
    prior["content_reference"] = json!("#Patient.contact");

    set_elements(
        &mut ir,
        vec![
            element("Patient.link", 0, json!("unbounded"), &["Patient"]),
            contact,
            prior,
        ],
    );
    let output = generate(ir);
    assert_snapshot!("recursion", file(&output, "patient.ts"));
}

#[test]
fn valibot_required_choice() {
    let mut ir = fixture();
    let elements = ir["types"][PATIENT_URL]["elements"].as_array_mut().unwrap();
    for el in elements.iter_mut() {
        if el["path"] == json!("Patient.value[x]") {
            el["cardinality"]["min"] = json!(1);
        }
    }
    let elements = elements.clone();
    set_elements(&mut ir, elements);
    let output = generate(ir);
    assert_snapshot!("required_choice", file(&output, "patient.ts"));
}

fn profile_with_constraints() -> ProfileInfo {
    ProfileInfo {
        type_name: "AuditPatient".to_string(),
        canonical_url: "http://example.org/fhir/StructureDefinition/audit-patient".to_string(),
        base_type: "Patient".to_string(),
        title: Some("Audit Patient Profile".to_string()),
        description: Some("Patient constrained for audit events".to_string()),
        must_support_elements: vec!["Patient.identifier".to_string()],
        fixed_elements: vec![FixedElement {
            path: "Patient.active".to_string(),
            field_name: "active".to_string(),
            fixed_value: "true".to_string(),
            value_type: "boolean".to_string(),
        }],
        constrained_elements: vec![ConstrainedElement {
            path: "Patient.identifier".to_string(),
            field_name: "identifier".to_string(),
            min: 1,
            max: "2".to_string(),
            makes_required: true,
            choice_members: Vec::new(),
        }],
        extensions: vec![],
    }
}

#[test]
fn valibot_profile_template_schema() {
    let mut tera = Tera::default();
    tera.add_raw_template(
        "profile.ts.tera",
        include_str!("../src/templates/profile.ts.tera"),
    )
    .expect("profile template");
    let generated = profile_with_constraints()
        .generate_typescript_with_template(
            &tera,
            &inkgen_core::config::ProfileMethodConfig::default(),
            true,
            ValidationLibrary::Valibot,
        )
        .expect("profile generation");
    assert_snapshot!("profile_template", generated);
}

#[test]
fn valibot_profile_legacy_schema() {
    let generated = profile_with_constraints().generate_typescript(
        false,
        false,
        true,
        ValidationLibrary::Valibot,
        None,
    );
    assert_snapshot!("profile_legacy", generated);
}
