//! Configuration for the Rust code generator

use serde::Deserialize;
use std::path::PathBuf;

/// Configuration for Rust code generation
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RustGeneratorConfig {
    /// Output directory for generated files
    pub output_dir: PathBuf,
}

impl RustGeneratorConfig {
    /// Create a new configuration with the given output directory
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    /// Create configuration from manifest if present
    /// Returns default configuration pointing to ./generated/rust
    pub fn from_manifest(
        _section: Option<&RustLanguageConfig>,
        default_output: PathBuf,
        override_output: Option<PathBuf>,
    ) -> Self {
        let output_dir = override_output.unwrap_or_else(|| default_output.join("rust"));

        Self { output_dir }
    }
}

/// Rust language configuration section from manifest
#[derive(Debug, Clone, Deserialize)]
pub struct RustLanguageConfig {
    /// Output directory for generated Rust code
    #[serde(default)]
    pub output_dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_config_from_path() {
        let config = RustGeneratorConfig::new(PathBuf::from("./output"));
        assert_eq!(config.output_dir, PathBuf::from("./output"));
    }

    #[test]
    fn test_rust_config_from_manifest_with_override() {
        let output = RustGeneratorConfig::from_manifest(
            None,
            PathBuf::from("./generated"),
            Some(PathBuf::from("./custom")),
        );
        assert_eq!(output.output_dir, PathBuf::from("./custom"));
    }

    #[test]
    fn test_rust_config_from_manifest_default() {
        let output = RustGeneratorConfig::from_manifest(None, PathBuf::from("./generated"), None);
        assert_eq!(output.output_dir, PathBuf::from("./generated/rust"));
    }
}
