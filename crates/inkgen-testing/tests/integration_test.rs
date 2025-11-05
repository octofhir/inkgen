use inkgen_testing::{TestFixture, cli::CliTester, integration::IntegrationTester, fixtures};

#[test]
fn test_fixture_operations() {
    let fixture = TestFixture::new().unwrap();
    
    // Test file creation and reading
    fixture.create_file("test.txt", "Hello, World!").unwrap();
    assert!(fixture.file_exists("test.txt"));
    
    let content = fixture.read_file("test.txt").unwrap();
    assert_eq!(content, "Hello, World!");
    
    // Test directory creation
    fixture.create_dir("subdir").unwrap();
    assert!(fixture.path().join("subdir").exists());
}

#[test]
fn test_cli_tester() {
    let cli_tester = CliTester::new().unwrap();
    assert!(cli_tester.fixture().path().exists());
    
    // Test config creation
    cli_tester.create_config("[general]\noutput_dir = \"output\"").unwrap();
    assert!(cli_tester.fixture().file_exists("inkgen.toml"));
}

#[test]
fn test_integration_tester() {
    let tester = IntegrationTester::new().unwrap();
    tester.setup_test_environment().unwrap();
    
    assert!(tester.fixture().file_exists("inkgen.toml"));
    assert!(tester.fixture().path().join("packages").exists());
    assert!(tester.fixture().path().join("output").exists());
}

#[test]
fn test_sample_fhir_resource() {
    let resource = fixtures::sample_fhir_resource();
    assert_eq!(resource["resourceType"], "Patient");
    assert_eq!(resource["id"], "example");
    assert_eq!(resource["gender"], "male");
}