//! Testing utilities for Inkgen

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Test fixture manager for temporary directories
pub struct TestFixture {
    temp_dir: TempDir,
}

impl TestFixture {
    /// Create a new test fixture with a temporary directory
    pub fn new() -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        Ok(Self { temp_dir })
    }
    
    /// Get the path to the temporary directory
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
    
    /// Create a file in the temporary directory
    pub fn create_file(&self, name: &str, content: &str) -> std::io::Result<PathBuf> {
        let file_path = self.path().join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, content)?;
        Ok(file_path)
    }
    
    /// Create a directory in the temporary directory
    pub fn create_dir(&self, name: &str) -> std::io::Result<PathBuf> {
        let dir_path = self.path().join(name);
        std::fs::create_dir_all(&dir_path)?;
        Ok(dir_path)
    }
    
    /// Read a file from the temporary directory
    pub fn read_file(&self, name: &str) -> std::io::Result<String> {
        let file_path = self.path().join(name);
        std::fs::read_to_string(file_path)
    }
    
    /// Check if a file exists in the temporary directory
    pub fn file_exists(&self, name: &str) -> bool {
        self.path().join(name).exists()
    }
}

impl Default for TestFixture {
    fn default() -> Self {
        Self::new().expect("Failed to create test fixture")
    }
}

/// Snapshot testing utilities
pub mod snapshot {
    use insta::{assert_snapshot, Settings};
    
    /// Assert that generated code matches a snapshot
    pub fn assert_code_snapshot(name: &str, code: &str) {
        let mut settings = Settings::clone_current();
        settings.set_snapshot_path("../snapshots");
        settings.bind(|| {
            assert_snapshot!(name, code);
        });
    }
    
    /// Assert that generated code matches a snapshot with custom settings
    pub fn assert_code_snapshot_with_settings<F>(name: &str, code: &str, configure: F)
    where
        F: FnOnce(&mut Settings),
    {
        let mut settings = Settings::clone_current();
        settings.set_snapshot_path("../snapshots");
        configure(&mut settings);
        settings.bind(|| {
            assert_snapshot!(name, code);
        });
    }
}

/// Test data utilities
pub mod fixtures {
    use std::path::PathBuf;
    
    /// Get the path to test fixtures directory
    pub fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
    }
    
    /// Load a test fixture file
    pub fn load_fixture(name: &str) -> std::io::Result<String> {
        let fixture_path = fixtures_dir().join(name);
        std::fs::read_to_string(fixture_path)
    }
    
    /// Create a sample FHIR resource for testing
    pub fn sample_fhir_resource() -> serde_json::Value {
        serde_json::json!({
            "resourceType": "Patient",
            "id": "example",
            "name": [{
                "family": "Doe",
                "given": ["John"]
            }],
            "gender": "male",
            "birthDate": "1990-01-01"
        })
    }
}

/// CLI testing utilities
pub mod cli {
    use std::process::{Command, Output};
    use crate::TestFixture;
    
    /// Helper for testing CLI commands
    pub struct CliTester {
        fixture: TestFixture,
    }
    
    impl CliTester {
        /// Create a new CLI tester
        pub fn new() -> std::io::Result<Self> {
            let fixture = TestFixture::new()?;
            Ok(Self { fixture })
        }
        
        /// Get the test fixture for file operations
        pub fn fixture(&self) -> &TestFixture {
            &self.fixture
        }
        
        /// Run a CLI command with arguments
        pub fn run_command(&self, args: &[&str]) -> std::io::Result<Output> {
            // Find the workspace root by looking for Cargo.toml
            let current_dir = std::env::current_dir()?;
            let workspace_root = current_dir
                .ancestors()
                .find(|path| path.join("Cargo.toml").exists())
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find workspace root with Cargo.toml"
                ))?;
            
            let mut cmd = Command::new("cargo");
            cmd.args(&["run", "--bin", "inkgen", "--"])
                .args(args)
                .current_dir(workspace_root);
            
            cmd.output()
        }
        
        /// Run a CLI command and expect success
        pub fn run_success(&self, args: &[&str]) -> std::io::Result<String> {
            let output = self.run_command(args)?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Command failed: {}", stderr)
                ));
            }
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        
        /// Run a CLI command and expect failure
        pub fn run_failure(&self, args: &[&str]) -> std::io::Result<String> {
            let output = self.run_command(args)?;
            if output.status.success() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Expected command to fail but it succeeded"
                ));
            }
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
        
        /// Create a test configuration file
        pub fn create_config(&self, content: &str) -> std::io::Result<()> {
            self.fixture.create_file("inkgen.toml", content)?;
            Ok(())
        }
        
        /// Create a test FHIR package structure
        pub fn create_fhir_package(&self, package_name: &str) -> std::io::Result<()> {
            let package_dir = format!("packages/{}", package_name);
            self.fixture.create_dir(&package_dir)?;
            
            // Create package.json
            let package_json = serde_json::json!({
                "name": package_name,
                "version": "1.0.0",
                "fhirVersions": ["4.0.1"],
                "dependencies": {}
            });
            self.fixture.create_file(
                &format!("{}/package.json", package_dir),
                &serde_json::to_string_pretty(&package_json)?
            )?;
            
            Ok(())
        }
    }
    
    impl Default for CliTester {
        fn default() -> Self {
            Self::new().expect("Failed to create CLI tester")
        }
    }
}

/// Integration test utilities
pub mod integration {
    use crate::{TestFixture, snapshot};
    
    /// End-to-end test runner for code generation workflows
    pub struct IntegrationTester {
        fixture: TestFixture,
    }
    
    impl IntegrationTester {
        /// Create a new integration tester
        pub fn new() -> std::io::Result<Self> {
            let fixture = TestFixture::new()?;
            Ok(Self { fixture })
        }
        
        /// Get the test fixture
        pub fn fixture(&self) -> &TestFixture {
            &self.fixture
        }
        
        /// Set up a complete test environment with FHIR packages and config
        pub fn setup_test_environment(&self) -> std::io::Result<()> {
            // Create basic directory structure
            self.fixture.create_dir("packages")?;
            self.fixture.create_dir("output")?;
            
            // Create a sample configuration
            let config = r#"
[general]
output_dir = "output"
package_dir = "packages"

[typescript]
enabled = true
output_file = "types.ts"
"#;
            self.fixture.create_file("inkgen.toml", config)?;
            
            Ok(())
        }
        
        /// Assert that generated code matches expected output
        pub fn assert_generated_code(&self, file_path: &str, snapshot_name: &str) -> std::io::Result<()> {
            let generated_code = self.fixture.read_file(file_path)?;
            snapshot::assert_code_snapshot(snapshot_name, &generated_code);
            Ok(())
        }
        
        /// Verify that expected files were created
        pub fn assert_files_created(&self, expected_files: &[&str]) -> Result<(), String> {
            for file in expected_files {
                if !self.fixture.file_exists(file) {
                    return Err(format!("Expected file '{}' was not created", file));
                }
            }
            Ok(())
        }
        
        /// Clean up generated files for the next test
        pub fn cleanup_generated(&self) -> std::io::Result<()> {
            let output_dir = self.fixture.path().join("output");
            if output_dir.exists() {
                std::fs::remove_dir_all(&output_dir)?;
                std::fs::create_dir(&output_dir)?;
            }
            Ok(())
        }
    }
    
    impl Default for IntegrationTester {
        fn default() -> Self {
            Self::new().expect("Failed to create integration tester")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fixture_basic_operations() {
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
    fn test_cli_tester_creation() {
        let cli_tester = cli::CliTester::new().unwrap();
        assert!(cli_tester.fixture().path().exists());
    }
    
    #[test]
    fn test_integration_tester_setup() {
        let tester = integration::IntegrationTester::new().unwrap();
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
    }
}