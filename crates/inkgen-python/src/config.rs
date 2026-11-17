//! Configuration for the Python code generator.

use serde::Deserialize;
use std::path::PathBuf;

pub use inkgen_core::PythonLanguageConfig;

/// Default name of the generated importable Python package.
fn default_package_name() -> String {
    "fhir".to_string()
}

/// Configuration for Python code generation.
#[derive(Debug, Clone, Deserialize)]
pub struct PythonGeneratorConfig {
    /// Output directory for generated files.
    pub output_dir: PathBuf,

    /// Name of the generated Python package (the importable directory).
    /// Defaults to `fhir`; must be a valid Python identifier.
    #[serde(default = "default_package_name")]
    pub package_name: String,
}

impl Default for PythonGeneratorConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::new(),
            package_name: default_package_name(),
        }
    }
}

impl PythonGeneratorConfig {
    /// Create a new configuration with the given output directory and the
    /// default package name (`fhir`).
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            package_name: default_package_name(),
        }
    }

    /// Override the generated package name.
    pub fn with_package_name(mut self, package_name: impl Into<String>) -> Self {
        self.package_name = package_name.into();
        self
    }

    /// Create configuration from a manifest `[languages.python]` section.
    ///
    /// Output directory precedence: explicit override, then the section's
    /// `output_dir`, then `<default_output>/python`. The package name comes
    /// from the section's `package_name`, defaulting to `fhir`.
    pub fn from_manifest(
        section: Option<&PythonLanguageConfig>,
        default_output: PathBuf,
        override_output: Option<PathBuf>,
    ) -> Self {
        let output_dir = override_output
            .or_else(|| {
                section
                    .and_then(|s| s.output_dir.clone())
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| default_output.join("python"));
        let package_name = section
            .and_then(|s| s.package_name.clone())
            .unwrap_or_else(default_package_name);

        Self {
            output_dir,
            package_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_path() {
        let config = PythonGeneratorConfig::new(PathBuf::from("./output"));
        assert_eq!(config.output_dir, PathBuf::from("./output"));
        assert_eq!(config.package_name, "fhir");
    }

    #[test]
    fn config_from_manifest_default() {
        let output = PythonGeneratorConfig::from_manifest(None, PathBuf::from("./generated"), None);
        assert_eq!(output.output_dir, PathBuf::from("./generated/python"));
        assert_eq!(output.package_name, "fhir");
    }

    #[test]
    fn config_from_manifest_package_name() {
        let section = PythonLanguageConfig {
            output_dir: None,
            package_name: Some("fhir_r4".to_string()),
        };
        let config = PythonGeneratorConfig::from_manifest(
            Some(&section),
            PathBuf::from("./generated"),
            None,
        );
        assert_eq!(config.package_name, "fhir_r4");
        assert_eq!(config.output_dir, PathBuf::from("./generated/python"));
    }

    #[test]
    fn config_output_override_precedence() {
        let section = PythonLanguageConfig {
            output_dir: Some("from-section".to_string()),
            package_name: None,
        };
        let config = PythonGeneratorConfig::from_manifest(
            Some(&section),
            PathBuf::from("./generated"),
            Some(PathBuf::from("explicit")),
        );
        assert_eq!(config.output_dir, PathBuf::from("explicit"));
    }
}
