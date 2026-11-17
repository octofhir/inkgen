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

/// Run the full pipeline over a fixture-derived IR with the given dialect.
fn generate_with(ir: Value, library: ValidationLibrary) -> GenerationOutput {
    let ir: PackageIr = serde_json::from_value(ir).expect("fixture deserializes to PackageIr");
    let ir = Arc::new(ir);
    let mut config =
        TypescriptGeneratorConfig::from_package_ir(ir.clone(), None, "generated".into())
            .expect("typescript config");
    config.validation_library = library;
    let backend = TypescriptGenerator::new(config);
    let output = backend.generate(&ir).expect("generation succeeds");
    assert_eq!(
        output.files,
        backend.generate(&ir).expect("second run").files,
        "generation must be deterministic"
    );
    output
}

/// Run the full Valibot pipeline over a fixture-derived IR.
fn generate(ir: Value) -> GenerationOutput {
    generate_with(ir, ValidationLibrary::Valibot)
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

/// Register FHIR primitives and a minimal `Element` complex type in the
/// fixture so primitive-extension companions (`_field`) are generated and
/// their `ElementSchema` references resolve to a real file.
fn add_companion_support(ir: &mut Value) {
    let package = json!({ "name": "audit.fhir", "version": "1.0.0" });
    for (code, name, stem) in [
        ("boolean", "Boolean", "boolean"),
        ("string", "String", "string"),
        ("date", "Date", "date"),
        ("base64Binary", "Base64Binary", "base64-binary"),
        ("code", "Code", "code"),
        ("positiveInt", "PositiveInt", "positive-int"),
    ] {
        let url = format!("http://example.org/fhir/StructureDefinition/{code}");
        ir["type_map"]["by_url"][&url] = json!({
            "canonical_url": url,
            "type_name": name,
            "file_stem": stem,
            "kind": "primitive_type",
            "package": package,
            "base_type": null,
            "type_code": code,
        });
        ir["type_map"]["by_name"][name] = json!(url);
        ir["type_map"]["by_stem"][stem] = json!(url);
    }

    let element_url = "http://example.org/fhir/StructureDefinition/Element";
    ir["types"][element_url] = json!({
        "id": "Element",
        "url": element_url,
        "name": "Element",
        "title": "Element",
        "description": "Base definition for all elements",
        "version": "1.0.0",
        "status": "active",
        "kind": "complex_type",
        "fhir_type": "Element",
        "date": null,
        "lineage": { "base_definition": null, "base_id": null, "derivation": null, "type_name": null },
        "elements": [element("Element.id", 0, finite(1), &["string"])],
        "extensions": [],
        "invariants": [],
    });
    ir["structures"].as_array_mut().unwrap().push(json!({
        "canonical_url": element_url,
        "name": "Element",
        "type_code": "Element",
        "title": "Element",
        "version": "1.0.0",
        "status": "active",
        "package": package,
        "kind": "complex_type",
    }));
    ir["canonical_index"]["Element"] = json!(element_url);
    ir["type_map"]["by_url"][element_url] = json!({
        "canonical_url": element_url,
        "type_name": "Element",
        "file_stem": "element",
        "kind": "complex_type",
        "package": package,
        "base_type": null,
        "type_code": "Element",
    });
    ir["type_map"]["by_name"]["Element"] = json!(element_url);
    ir["type_map"]["by_stem"]["element"] = json!(element_url);
    ir["package_descriptor"]["inventory"]["structures"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "canonical_url": element_url,
            "name": "Element",
            "type_code": "Element",
            "title": "Element",
            "version": "1.0.0",
            "status": "active",
            "package": package,
            "kind": "complex_type",
        }));
}

#[test]
fn primitive_companions_are_lazy_in_both_dialects() {
    let mut ir = fixture();
    add_companion_support(&mut ir);
    for (library, label) in [
        (ValidationLibrary::Zod, "zod"),
        (ValidationLibrary::Valibot, "valibot"),
    ] {
        let output = generate_with(ir.clone(), library);
        assert_snapshot!(
            format!("companions_{label}_patient"),
            file(&output, "patient.ts")
        );
        assert_snapshot!(
            format!("companions_{label}_element"),
            file(&output, "element.ts")
        );
    }
}
