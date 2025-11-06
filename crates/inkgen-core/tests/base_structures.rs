use inkgen_core::ir::{BindingStrength, ElementMax};
use inkgen_core::{InstallMode, PackageRequest, StructureDefinitionProvider};
use inkgen_testing::{CORE_PACKAGE, CORE_VERSION, CoreTestContext};
use insta::assert_json_snapshot;
use serde_json::to_value;

#[tokio::test]
async fn package_cache_supports_offline_mode_after_install() {
    let ctx = CoreTestContext::new().await.expect("context");
    let requests: Vec<PackageRequest> = ctx.config().package_requests();

    // Subsequent calls in offline mode should succeed using the cached package.
    ctx.cache()
        .ensure_packages(&requests, InstallMode::OfflineOnly)
        .await
        .expect("offline ensure uses cache");

    let installed = ctx.cache().list_installed().await.expect("list");
    assert!(
        installed
            .iter()
            .any(|pkg| pkg == &format!("{CORE_PACKAGE}@{CORE_VERSION}")),
        "expected {CORE_PACKAGE}@{CORE_VERSION} in installed packages, got {installed:?}"
    );
}

#[tokio::test]
async fn load_patient_structure_returns_ir() {
    let ctx = CoreTestContext::new().await.expect("context");
    let service = ctx.structure_service();
    let patient = service
        .load_structure("http://hl7.org/fhir/StructureDefinition/Patient")
        .await
        .expect("load patient structure");

    assert_eq!(patient.id, "Patient");
    assert_eq!(
        patient
            .lineage
            .base_definition
            .as_deref()
            .expect("base definition"),
        "http://hl7.org/fhir/StructureDefinition/DomainResource"
    );
    assert!(patient.elements.len() > 40);

    let gender = patient
        .elements
        .iter()
        .find(|element| element.path == "Patient.gender")
        .expect("find gender element");

    assert_eq!(gender.cardinality.min, 0);
    match &gender.cardinality.max {
        ElementMax::Finite(1) => {}
        other => panic!("expected max=1, got {other:?}"),
    }

    let binding = gender.binding.as_ref().expect("gender binding");
    assert_eq!(binding.strength, BindingStrength::Required);
    assert!(
        binding
            .value_set
            .as_deref()
            .is_some_and(|url| url.contains("administrative-gender"))
    );
}

#[tokio::test]
async fn patient_structure_snapshot() {
    let ctx = CoreTestContext::new().await.expect("context");
    let definition = ctx
        .structure_service()
        .load_structure("http://hl7.org/fhir/StructureDefinition/Patient")
        .await
        .expect("load patient structure");
    assert_json_snapshot!(
        "patient_structure",
        to_value(definition).expect("serialize")
    );
}

#[tokio::test]
async fn human_name_structure_snapshot() {
    let ctx = CoreTestContext::new().await.expect("context");
    let definition = ctx
        .structure_service()
        .load_structure("http://hl7.org/fhir/StructureDefinition/HumanName")
        .await
        .expect("load HumanName structure");
    assert_json_snapshot!(
        "human_name_structure",
        to_value(definition).expect("serialize")
    );
}
