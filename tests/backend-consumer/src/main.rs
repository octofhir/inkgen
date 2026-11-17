use inkgen_core::{Backend, PackageIr};
use inkgen_typescript::{TypescriptGenerator, TypescriptGeneratorConfig, ValidationLibrary};
use std::sync::Arc;

fn main() {
    let ir: PackageIr = serde_json::from_str(include_str!("../package-ir.json")).unwrap();
    let json = serde_json::to_string(&ir).unwrap();
    let ir: PackageIr = serde_json::from_str(&json).unwrap();
    let ir = Arc::new(ir);

    check_backend(&inkgen_rust::RustGenerator::default(), &ir);
    check_backend(&inkgen_gleam::GleamGenerator::default(), &ir);

    for library in [ValidationLibrary::Zod, ValidationLibrary::Valibot] {
        let mut config =
            TypescriptGeneratorConfig::from_package_ir(ir.clone(), None, "generated".into())
                .expect("typescript config");
        config.validation_library = library;
        check_backend(&TypescriptGenerator::new(config), &ir);
    }
}

fn check_backend(backend: &dyn Backend, ir: &Arc<PackageIr>) {
    let output = backend.generate(ir).unwrap();
    assert!(!output.files.is_empty());
    assert_eq!(output.files, backend.generate(ir).unwrap().files);
    println!(
        "{}\t{}",
        backend.id(),
        serde_json::to_string(&output.files).unwrap()
    );
    let content = output
        .files
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        content.contains("Patient"),
        "{} omitted Patient",
        backend.id()
    );
    assert!(
        content.contains("active"),
        "{} omitted active",
        backend.id()
    );
    assert!(
        content.contains("alias"),
        "{} omitted repeating alias",
        backend.id()
    );
    assert!(
        content.contains("Пациент — 日本語"),
        "{} lost Unicode",
        backend.id()
    );
    let normalized = content.replace('_', "").to_lowercase();
    assert!(
        normalized.contains("valueboolean"),
        "{} omitted boolean choice",
        backend.id()
    );
    assert!(
        normalized.contains("valuestring"),
        "{} omitted string choice",
        backend.id()
    );
}