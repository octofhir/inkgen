//! Integration tests for the Inkgen CLI
//! 
//! These tests verify CLI argument parsing, validation, and end-to-end command execution.

use inkgen_testing::cli::CliTester;
use inkgen_testing::snapshot;

/// Test CLI argument parsing and validation for the fetch command
#[test]
fn test_fetch_command_argument_parsing() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test valid fetch command with package name
    let output = cli_tester.run_success(&["fetch", "--package", "hl7.fhir.r4.core"])
        .expect("Fetch command should succeed with valid package name");
    
    assert!(output.contains("Fetching package: hl7.fhir.r4.core"));
    
    // Test fetch command with package name and version
    let output = cli_tester.run_success(&["fetch", "--package", "hl7.fhir.r4.core", "--version", "4.0.1"])
        .expect("Fetch command should succeed with package and version");
    
    assert!(output.contains("Fetching package: hl7.fhir.r4.core"));
    assert!(output.contains("Version: 4.0.1"));
}

/// Test CLI argument validation for missing required arguments
#[test]
fn test_fetch_command_validation_errors() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test fetch command without required package argument
    let stderr = cli_tester.run_failure(&["fetch"])
        .expect("Fetch command should fail without package argument");
    
    assert!(stderr.contains("required") || stderr.contains("package"));
}

/// Test CLI argument parsing for the generate command
#[test]
fn test_generate_command_argument_parsing() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Create test input directory
    cli_tester.fixture().create_dir("input").expect("Failed to create input dir");
    cli_tester.fixture().create_file("input/package.json", r#"{"name": "test-package"}"#)
        .expect("Failed to create test package");
    
    // Test valid generate command with all required arguments
    let output = cli_tester.run_success(&[
        "generate", 
        "--input", "input", 
        "--output", "output"
    ]).expect("Generate command should succeed with valid arguments");
    
    assert!(output.contains("Generating typescript code from: input"));
    assert!(output.contains("Output directory: output"));
    
    // Test generate command with custom language
    let output = cli_tester.run_success(&[
        "generate", 
        "--input", "input", 
        "--output", "output",
        "--language", "rust"
    ]).expect("Generate command should succeed with custom language");
    
    assert!(output.contains("Generating rust code from: input"));
}

/// Test CLI argument validation for generate command
#[test]
fn test_generate_command_validation_errors() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test generate command without required input argument
    let stderr = cli_tester.run_failure(&["generate", "--output", "output"])
        .expect("Generate command should fail without input argument");
    
    assert!(stderr.contains("required") || stderr.contains("input"));
    
    // Test generate command without required output argument
    let stderr = cli_tester.run_failure(&["generate", "--input", "input"])
        .expect("Generate command should fail without output argument");
    
    assert!(stderr.contains("required") || stderr.contains("output"));
}

/// Test CLI argument parsing for the config command
#[test]
fn test_config_command_argument_parsing() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test config command with show flag
    let output = cli_tester.run_success(&["config", "--show"])
        .expect("Config command should succeed with show flag");
    
    assert!(output.contains("Current configuration:"));
    
    // Test config command with set option
    let output = cli_tester.run_success(&["config", "--set", "key=value"])
        .expect("Config command should succeed with set option");
    
    assert!(output.contains("Setting configuration: key=value"));
}

/// Test CLI help and version commands
#[test]
fn test_help_and_version_commands() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test help command
    let output = cli_tester.run_success(&["--help"])
        .expect("Help command should succeed");
    
    assert!(output.contains("A FHIR code generator"));
    assert!(output.contains("fetch"));
    assert!(output.contains("generate"));
    assert!(output.contains("config"));
    
    // Test version command
    let output = cli_tester.run_success(&["--version"])
        .expect("Version command should succeed");
    
    assert!(output.contains("inkgen"));
}

/// Test end-to-end fetch command execution
#[test]
fn test_fetch_command_end_to_end() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test complete fetch workflow
    let output = cli_tester.run_success(&[
        "fetch", 
        "--package", "hl7.fhir.r4.core",
        "--version", "4.0.1"
    ]).expect("End-to-end fetch should succeed");
    
    // Verify expected output format
    assert!(output.contains("Fetching package: hl7.fhir.r4.core"));
    assert!(output.contains("Version: 4.0.1"));
}

/// Test end-to-end generate command execution
#[test]
fn test_generate_command_end_to_end() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Set up test environment
    cli_tester.fixture().create_dir("test_input").expect("Failed to create input dir");
    cli_tester.fixture().create_file("test_input/package.json", r#"{
        "name": "test-fhir-package",
        "version": "1.0.0",
        "fhirVersions": ["4.0.1"]
    }"#).expect("Failed to create test package");
    
    // Test complete generate workflow
    let output = cli_tester.run_success(&[
        "generate",
        "--input", "test_input",
        "--output", "test_output",
        "--language", "typescript"
    ]).expect("End-to-end generate should succeed");
    
    // Verify expected output format
    assert!(output.contains("Generating typescript code from: test_input"));
    assert!(output.contains("Output directory: test_output"));
}

/// Test end-to-end config command execution
#[test]
fn test_config_command_end_to_end() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test config show
    let output = cli_tester.run_success(&["config", "--show"])
        .expect("Config show should succeed");
    
    assert!(output.contains("Current configuration:"));
    
    // Test config set
    let output = cli_tester.run_success(&["config", "--set", "output_dir=/tmp/inkgen"])
        .expect("Config set should succeed");
    
    assert!(output.contains("Setting configuration: output_dir=/tmp/inkgen"));
}

/// Test CLI output format consistency
#[test]
fn test_cli_output_format_consistency() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test that all commands produce consistent output format
    let fetch_output = cli_tester.run_success(&["fetch", "--package", "test"])
        .expect("Fetch command should succeed");
    
    let generate_output = cli_tester.run_success(&["generate", "--input", ".", "--output", "out"])
        .expect("Generate command should succeed");
    
    let config_output = cli_tester.run_success(&["config", "--show"])
        .expect("Config command should succeed");
    
    // All outputs should be non-empty and properly formatted
    assert!(!fetch_output.trim().is_empty());
    assert!(!generate_output.trim().is_empty());
    assert!(!config_output.trim().is_empty());
}

/// Test CLI error handling and exit codes
#[test]
fn test_cli_error_handling() {
    let cli_tester = CliTester::new().expect("Failed to create CLI tester");
    
    // Test invalid command
    let stderr = cli_tester.run_failure(&["invalid-command"])
        .expect("Invalid command should fail");
    
    assert!(stderr.contains("error") || stderr.contains("invalid") || stderr.contains("unrecognized"));
    
    // Test invalid flag
    let stderr = cli_tester.run_failure(&["fetch", "--invalid-flag"])
        .expect("Invalid flag should fail");
    
    assert!(stderr.contains("error") || stderr.contains("invalid") || stderr.contains("unrecognized"));
}

/// Snapshot tests for CLI output format
#[cfg(test)]
mod snapshot_tests {
    use super::*;
    
    #[test]
    fn test_help_output_snapshot() {
        let cli_tester = CliTester::new().expect("Failed to create CLI tester");
        
        let help_output = cli_tester.run_success(&["--help"])
            .expect("Help command should succeed");
        
        snapshot::assert_code_snapshot("cli_help_output", &help_output);
    }
    
    #[test]
    fn test_fetch_output_snapshot() {
        let cli_tester = CliTester::new().expect("Failed to create CLI tester");
        
        let fetch_output = cli_tester.run_success(&[
            "fetch", 
            "--package", "hl7.fhir.r4.core",
            "--version", "4.0.1"
        ]).expect("Fetch command should succeed");
        
        snapshot::assert_code_snapshot("cli_fetch_output", &fetch_output);
    }
    
    #[test]
    fn test_generate_output_snapshot() {
        let cli_tester = CliTester::new().expect("Failed to create CLI tester");
        
        // Set up test input
        cli_tester.fixture().create_dir("snap_input").expect("Failed to create input dir");
        cli_tester.fixture().create_file("snap_input/package.json", r#"{"name": "test"}"#)
            .expect("Failed to create test package");
        
        let generate_output = cli_tester.run_success(&[
            "generate",
            "--input", "snap_input",
            "--output", "snap_output",
            "--language", "typescript"
        ]).expect("Generate command should succeed");
        
        snapshot::assert_code_snapshot("cli_generate_output", &generate_output);
    }
    
    #[test]
    fn test_config_output_snapshot() {
        let cli_tester = CliTester::new().expect("Failed to create CLI tester");
        
        let config_output = cli_tester.run_success(&["config", "--show"])
            .expect("Config command should succeed");
        
        snapshot::assert_code_snapshot("cli_config_output", &config_output);
    }
}