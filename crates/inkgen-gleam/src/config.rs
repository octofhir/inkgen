//! Configuration for the Gleam code generator.

use serde::Deserialize;
use std::path::PathBuf;

/// Configuration for Gleam code generation.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GleamGeneratorConfig {
    /// Output directory for generated files.
    pub output_dir: PathBuf,
}

impl GleamGeneratorConfig {
    /// Create a new configuration with the given output directory.
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    /// Create configuration from manifest if present.
    /// Returns default configuration pointing to `./generated/gleam`.
    pub fn from_manifest(
        _section: Option<&GleamLanguageConfig>,
        default_output: PathBuf,
        override_output: Option<PathBuf>,
    ) -> Self {
        let output_dir = override_output.unwrap_or_else(|| default_output.join("gleam"));

        Self { output_dir }
    }
}

/// Gleam language configuration section from manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct GleamLanguageConfig {
    /// Output directory for generated Gleam code.
    #[serde(default)]
    pub output_dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_path() {
        let config = GleamGeneratorConfig::new(PathBuf::from("./output"));
        assert_eq!(config.output_dir, PathBuf::from("./output"));
    }

    #[test]
    fn config_from_manifest_default() {
        let output =
            GleamGeneratorConfig::from_manifest(None, PathBuf::from("./generated"), None);
        assert_eq!(output.output_dir, PathBuf::from("./generated/gleam"));
    }
}
