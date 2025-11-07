//! Template overlay system for customizing code generation output.
//!
//! This module handles loading and merging template overlays from the filesystem.
//! Overlays allow users to customize built-in templates by providing their own versions
//! with the same filename. Overlay templates override the built-in templates.
//!
//! # Architecture
//!
//! The overlay system works in three phases:
//!
//! 1. **Loading Built-in Templates**: Built-in templates are registered from embedded strings
//! 2. **Discovering Overlays**: Overlay directories are scanned for matching template files
//! 3. **Merging**: Overlay templates override built-in templates with the same name
//!
//! # Example
//!
//! Given a manifest with:
//! ```toml
//! [languages.typescript]
//! overlays = ["./templates/custom", "../shared-templates"]
//! ```
//!
//! And a custom template at `./templates/custom/structure.ts.tera`, this file will
//! be loaded and override the built-in `structure.ts.tera` template.

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use tera::Tera;
use tracing::{info, warn};

/// Known built-in template names that can be overridden
pub const BUILTIN_TEMPLATES: &[&str] = &[
    "structure.ts.tera",
    "index.ts.tera",
    "extensions.ts.tera",
    "profile_helpers.ts.tera",
    "terminology_helpers.ts.tera",
    "invariant_validators.ts.tera",
    "discriminator_unions.ts.tera",
];

/// Configuration for overlay loading
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Directories containing overlay templates, in priority order
    /// First directory takes precedence
    pub directories: Vec<PathBuf>,
    /// Whether to fail if overlay directory doesn't exist
    pub strict: bool,
}

impl OverlayConfig {
    /// Create a new overlay configuration with directories relative to a base path
    pub fn new(base_path: &Path, overlay_paths: &[String], strict: bool) -> Result<Self> {
        let mut directories = Vec::new();

        for path in overlay_paths {
            let abs_path = if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                base_path.join(path)
            };

            let exists = abs_path.exists();

            if strict && !exists {
                return Err(anyhow!(
                    "Overlay directory not found: {} (resolved to {})",
                    path,
                    abs_path.display()
                ));
            }

            if exists && !abs_path.is_dir() {
                return Err(anyhow!(
                    "Overlay path is not a directory: {}",
                    abs_path.display()
                ));
            }

            // Only add existing directories in non-strict mode
            if exists {
                directories.push(abs_path);
            }
        }

        Ok(Self {
            directories,
            strict,
        })
    }

    /// Create a configuration from a base path and overlay paths
    /// Returns successfully even if some directories don't exist (non-strict mode)
    pub fn from_manifest(base_path: &Path, overlay_paths: &[String]) -> Result<Self> {
        Self::new(base_path, overlay_paths, false)
    }
}

/// Load templates from overlay directories and apply them to Tera
pub fn apply_overlays(tera: &mut Tera, config: &OverlayConfig) -> Result<()> {
    let mut loaded_count = 0;
    let mut skipped_count = 0;

    for overlay_dir in &config.directories {
        if !overlay_dir.exists() {
            warn!(
                "Overlay directory does not exist (skipping): {}",
                overlay_dir.display()
            );
            continue;
        }

        // Scan for template files
        for entry in fs::read_dir(overlay_dir).with_context(|| {
            format!(
                "Failed to read overlay directory: {}",
                overlay_dir.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("tera") {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("Invalid filename in overlay directory"))?
                    .to_string();

                // Check if this is a known template
                if BUILTIN_TEMPLATES.contains(&filename.as_str()) {
                    let content = fs::read_to_string(&path).with_context(|| {
                        format!("Failed to read overlay template: {}", path.display())
                    })?;

                    tera.add_raw_template(&filename, &content)
                        .with_context(|| format!("Failed to add overlay template: {}", filename))?;

                    info!(
                        "Loaded overlay template: {} from {}",
                        filename,
                        overlay_dir.display()
                    );
                    loaded_count += 1;
                } else {
                    warn!(
                        "Unknown template filename in overlay (ignoring): {} from {}",
                        filename,
                        overlay_dir.display()
                    );
                    skipped_count += 1;
                }
            }
        }
    }

    if loaded_count > 0 {
        info!(
            "Applied {} overlay templates ({} unknown files skipped)",
            loaded_count, skipped_count
        );
    }

    Ok(())
}

/// Validate that all overlay templates are valid Tera templates
pub fn validate_overlays(config: &OverlayConfig) -> Result<()> {
    let mut errors = Vec::new();

    for overlay_dir in &config.directories {
        if !overlay_dir.exists() {
            continue;
        }

        for entry in fs::read_dir(overlay_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("tera") {
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

                if BUILTIN_TEMPLATES.contains(&filename) {
                    if let Err(e) = fs::read_to_string(&path) {
                        errors.push(format!("Cannot read {}: {}", path.display(), e));
                    } else {
                        // Try to parse as Tera template
                        let content = fs::read_to_string(&path)?;
                        let mut test_tera = Tera::default();
                        if let Err(e) = test_tera.add_raw_template(filename, &content) {
                            errors.push(format!("Invalid Tera template {}: {}", path.display(), e));
                        }
                    }
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(anyhow!("Overlay validation failed:\n{}", errors.join("\n")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_overlay_config_relative_paths() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create subdirectories
        let custom_dir = base.join("templates/custom");
        std::fs::create_dir_all(&custom_dir).unwrap();

        let overlays = vec!["templates/custom".to_string()];

        let config = OverlayConfig::from_manifest(base, &overlays).unwrap();

        assert_eq!(config.directories.len(), 1);
        assert_eq!(config.directories[0], custom_dir);
    }

    #[test]
    fn test_overlay_config_absolute_paths() {
        let temp_dir = TempDir::new().unwrap();
        let base = Path::new("/home/user/project");
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        let overlays = vec![temp_path.clone()];

        let config = OverlayConfig::from_manifest(base, &overlays).unwrap();

        assert_eq!(config.directories.len(), 1);
        assert_eq!(config.directories[0], PathBuf::from(&temp_path));
    }

    #[test]
    fn test_overlay_config_skips_nonexistent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create one subdirectory
        let sub_dir = temp_path.join("exists");
        std::fs::create_dir(&sub_dir).unwrap();

        // Mix of existent and non-existent paths
        let overlays = vec!["./does_not_exist".to_string(), "./exists".to_string()];

        // Should succeed in non-strict mode and only include existing dir
        let config = OverlayConfig::from_manifest(temp_path, &overlays).unwrap();
        assert_eq!(config.directories.len(), 1);
        assert_eq!(config.directories[0], sub_dir);
    }

    #[test]
    fn test_overlay_config_strict_mode_fails() {
        let base = Path::new("/tmp");
        let overlays = vec!["/nonexistent/path".to_string()];

        let result = OverlayConfig::new(base, &overlays, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_apply_overlays_loads_valid_templates() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("structure.ts.tera");

        // Create a simple template
        fs::write(&template_path, "Hello {{ name }}").unwrap();

        let config = OverlayConfig {
            directories: vec![temp_dir.path().to_path_buf()],
            strict: false,
        };

        let mut tera = Tera::default();
        apply_overlays(&mut tera, &config).unwrap();

        // Template should be loaded
        assert!(tera.get_template("structure.ts.tera").is_ok());
    }

    #[test]
    fn test_apply_overlays_ignores_unknown_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create files with unknown names
        fs::write(temp_dir.path().join("custom.ts.tera"), "test").unwrap();
        fs::write(temp_dir.path().join("other.txt"), "test").unwrap();

        let config = OverlayConfig {
            directories: vec![temp_dir.path().to_path_buf()],
            strict: false,
        };

        let mut tera = Tera::default();
        // Should not error, just skip unknown files
        apply_overlays(&mut tera, &config).unwrap();
    }

    #[test]
    fn test_validate_overlays_detects_invalid_templates() {
        let temp_dir = TempDir::new().unwrap();

        // Create an invalid Tera template (unclosed block)
        fs::write(
            temp_dir.path().join("structure.ts.tera"),
            "{% for item in items",
        )
        .unwrap();

        let config = OverlayConfig {
            directories: vec![temp_dir.path().to_path_buf()],
            strict: false,
        };

        let result = validate_overlays(&config);
        assert!(result.is_err());
    }
}
