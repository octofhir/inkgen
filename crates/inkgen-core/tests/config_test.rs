use inkgen_core::{FilterMode, InkgenConfig, PackageEntry};

#[test]
fn test_sanitize_package_name() {
    use inkgen_core::config::sanitize_package_name;

    // Test hl7.fhir prefix removal
    assert_eq!(sanitize_package_name("hl7.fhir.r4.core"), "r4-core");
    assert_eq!(sanitize_package_name("hl7.fhir.r5.core"), "r5-core");
    assert_eq!(sanitize_package_name("hl7.fhir.us.core"), "us-core");

    // Test hl7 prefix removal
    assert_eq!(sanitize_package_name("hl7.terminology"), "terminology");

    // Test ihe prefix removal
    assert_eq!(sanitize_package_name("ihe.iti.pix"), "iti-pix");

    // Test org prefix removal
    assert_eq!(
        sanitize_package_name("org.example.custom"),
        "example-custom"
    );

    // Test no prefix (already clean)
    assert_eq!(sanitize_package_name("custom-package"), "custom-package");

    // Test generic version handling (no hardcoded r4/r5)
    assert_eq!(sanitize_package_name("hl7.fhir.r6.core"), "r6-core");
    assert_eq!(sanitize_package_name("hl7.fhir.r7.core"), "r7-core");
}

#[test]
fn test_package_entry_folder_name() {
    // Custom folder name
    let entry = PackageEntry {
        name: "hl7.fhir.r4.core".to_string(),
        version: "4.0.1".to_string(),
        folder: Some("my-custom-folder".to_string()),
        filter: FilterMode::All,
        include_resources: vec![],
        include_urls: vec![],
        exclude_resources: vec![],
        exclude_urls: vec![],
    };
    assert_eq!(entry.folder_name(), "my-custom-folder");

    // Auto-sanitized folder name
    let entry = PackageEntry {
        name: "hl7.fhir.us.core".to_string(),
        version: "7.0.0".to_string(),
        folder: None,
        filter: FilterMode::All,
        include_resources: vec![],
        include_urls: vec![],
        exclude_resources: vec![],
        exclude_urls: vec![],
    };
    assert_eq!(entry.folder_name(), "us-core");
}

#[test]
fn test_config_parsing_all_filter_modes() {
    // Test All mode
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
filter = "all"
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.packages[0].filter, FilterMode::All);

    // Test Dependencies mode
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
filter = "dependencies"
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.packages[0].filter, FilterMode::Dependencies);

    // Test None mode
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
filter = "none"
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.packages[0].filter, FilterMode::None);

    // Test Include mode
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
filter = "include"
include_resources = ["Patient", "Observation"]
include_urls = ["http://hl7.org/fhir/StructureDefinition/Patient"]
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.packages[0].filter, FilterMode::Include);
    assert_eq!(config.packages[0].include_resources.len(), 2);
    assert_eq!(config.packages[0].include_urls.len(), 1);

    // Test Exclude mode
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
filter = "exclude"
exclude_resources = ["Bundle"]
exclude_urls = ["http://hl7.org/fhir/StructureDefinition/Bundle"]
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.packages[0].filter, FilterMode::Exclude);
    assert_eq!(config.packages[0].exclude_resources.len(), 1);
    assert_eq!(config.packages[0].exclude_urls.len(), 1);
}

#[test]
fn test_config_default_values() {
    // Test default filter mode is All
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.packages[0].filter, FilterMode::All);
    assert!(config.packages[0].include_resources.is_empty());
    assert!(config.packages[0].include_urls.is_empty());
    assert!(config.packages[0].exclude_resources.is_empty());
    assert!(config.packages[0].exclude_urls.is_empty());
    assert!(config.packages[0].folder.is_none());
}

#[test]
fn test_should_include_resource_all_mode() {
    let entry = PackageEntry {
        name: "hl7.fhir.r4.core".to_string(),
        version: "4.0.1".to_string(),
        folder: None,
        filter: FilterMode::All,
        include_resources: vec![],
        include_urls: vec![],
        exclude_resources: vec![],
        exclude_urls: vec![],
    };

    assert!(
        entry.should_include_resource("Patient", "http://hl7.org/fhir/StructureDefinition/Patient")
    );
    assert!(entry.should_include_resource(
        "Observation",
        "http://hl7.org/fhir/StructureDefinition/Observation"
    ));
}

#[test]
fn test_should_include_resource_none_mode() {
    let entry = PackageEntry {
        name: "hl7.fhir.r4.core".to_string(),
        version: "4.0.1".to_string(),
        folder: None,
        filter: FilterMode::None,
        include_resources: vec![],
        include_urls: vec![],
        exclude_resources: vec![],
        exclude_urls: vec![],
    };

    assert!(
        !entry
            .should_include_resource("Patient", "http://hl7.org/fhir/StructureDefinition/Patient")
    );
}

#[test]
fn test_should_include_resource_include_mode() {
    let entry = PackageEntry {
        name: "hl7.fhir.r4.core".to_string(),
        version: "4.0.1".to_string(),
        folder: None,
        filter: FilterMode::Include,
        include_resources: vec!["Patient".to_string()],
        include_urls: vec!["http://hl7.org/fhir/StructureDefinition/Observation".to_string()],
        exclude_resources: vec![],
        exclude_urls: vec![],
    };

    // Include by name
    assert!(
        entry.should_include_resource("Patient", "http://hl7.org/fhir/StructureDefinition/Patient")
    );

    // Include by URL
    assert!(entry.should_include_resource(
        "Observation",
        "http://hl7.org/fhir/StructureDefinition/Observation"
    ));

    // Not in whitelist
    assert!(!entry.should_include_resource(
        "Condition",
        "http://hl7.org/fhir/StructureDefinition/Condition"
    ));
}

#[test]
fn test_should_include_resource_exclude_mode() {
    let entry = PackageEntry {
        name: "hl7.fhir.r4.core".to_string(),
        version: "4.0.1".to_string(),
        folder: None,
        filter: FilterMode::Exclude,
        include_resources: vec![],
        include_urls: vec![],
        exclude_resources: vec!["Bundle".to_string()],
        exclude_urls: vec!["http://hl7.org/fhir/StructureDefinition/Binary".to_string()],
    };

    // Not in blacklist - should include
    assert!(
        entry.should_include_resource("Patient", "http://hl7.org/fhir/StructureDefinition/Patient")
    );

    // In blacklist by name - should exclude
    assert!(
        !entry.should_include_resource("Bundle", "http://hl7.org/fhir/StructureDefinition/Bundle")
    );

    // In blacklist by URL - should exclude
    assert!(
        !entry.should_include_resource("Binary", "http://hl7.org/fhir/StructureDefinition/Binary")
    );
}

#[test]
fn test_should_include_by_filter_dependencies_mode() {
    let entry = PackageEntry {
        name: "hl7.fhir.r4.core".to_string(),
        version: "4.0.1".to_string(),
        folder: None,
        filter: FilterMode::Dependencies,
        include_resources: vec![],
        include_urls: vec![],
        exclude_resources: vec![],
        exclude_urls: vec![],
    };

    // Should only include if it's a dependency
    assert!(
        entry.should_include_by_filter("http://hl7.org/fhir/StructureDefinition/Patient", true)
    );
    assert!(
        !entry.should_include_by_filter("http://hl7.org/fhir/StructureDefinition/Patient", false)
    );
}

#[test]
fn test_multiple_packages_config() {
    let toml = r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
folder = "r4-core"
filter = "dependencies"

[[packages]]
name = "hl7.fhir.us.core"
version = "7.0.0"
filter = "all"
"#;
    let config: InkgenConfig = toml::from_str(toml).unwrap();

    assert_eq!(config.packages.len(), 2);
    assert_eq!(config.packages[0].name, "hl7.fhir.r4.core");
    assert_eq!(config.packages[0].filter, FilterMode::Dependencies);
    assert_eq!(config.packages[0].folder, Some("r4-core".to_string()));

    assert_eq!(config.packages[1].name, "hl7.fhir.us.core");
    assert_eq!(config.packages[1].filter, FilterMode::All);
    assert!(config.packages[1].folder.is_none());
}
