//! File comparison utilities for detecting changes between generated outputs
//!
//! This module provides functionality to compare two directories of generated files
//! and report differences in a human-readable format.

use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuration for diff operations
#[derive(Debug, Clone)]
pub struct DiffConfig {
    /// Directory containing the original/baseline files
    pub old_dir: PathBuf,
    /// Directory containing the new/generated files
    pub new_dir: PathBuf,
    /// Optional file extension filter (e.g., ".ts" to only compare TypeScript files)
    pub extension_filter: Option<String>,
    /// Show context lines around differences
    pub context_lines: usize,
}

impl DiffConfig {
    /// Create a new diff configuration
    pub fn new(old_dir: PathBuf, new_dir: PathBuf) -> Self {
        Self {
            old_dir,
            new_dir,
            extension_filter: None,
            context_lines: 3,
        }
    }

    /// Set an extension filter (e.g., ".ts")
    pub fn with_extension_filter(mut self, ext: String) -> Self {
        self.extension_filter = Some(ext);
        self
    }

    /// Set the number of context lines to display
    pub fn with_context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }
}

/// Results of a diff operation
#[derive(Debug)]
pub struct DiffResult {
    /// Number of files only in the old directory
    pub files_removed: usize,
    /// Number of files only in the new directory
    pub files_added: usize,
    /// Number of files that differ between directories
    pub files_changed: usize,
    /// Files that are identical
    pub files_identical: usize,
    /// Total changes (lines added + removed)
    pub total_changes: usize,
}

/// Compare two directories and report differences
pub fn diff_directories(config: &DiffConfig) -> Result<DiffResult> {
    let mut result = DiffResult {
        files_removed: 0,
        files_added: 0,
        files_changed: 0,
        files_identical: 0,
        total_changes: 0,
    };

    // Get all files from both directories
    let old_files = collect_files(&config.old_dir, config.extension_filter.as_deref())?;
    let new_files = collect_files(&config.new_dir, config.extension_filter.as_deref())?;

    // Check for removed files
    for file in &old_files {
        let rel_path = file.strip_prefix(&config.old_dir)?;
        let new_path = config.new_dir.join(rel_path);

        if !new_path.exists() {
            result.files_removed += 1;
            println!("Removed: {}", rel_path.display());
        }
    }

    // Check for added and changed files
    for file in &new_files {
        let rel_path = file.strip_prefix(&config.new_dir)?;
        let old_path = config.old_dir.join(rel_path);

        if !old_path.exists() {
            result.files_added += 1;
            println!("Added: {}", rel_path.display());
        } else {
            // Compare file contents
            let old_content = fs::read_to_string(&old_path)?;
            let new_content = fs::read_to_string(file)?;

            if old_content != new_content {
                result.files_changed += 1;
                let changes = count_changes(&old_content, &new_content);
                result.total_changes += changes;

                println!("Changed: {} ({} changes)", rel_path.display(), changes);
                print_diff(&old_content, &new_content, rel_path, config.context_lines);
            } else {
                result.files_identical += 1;
            }
        }
    }

    info!(
        "Diff complete: {} added, {} removed, {} changed, {} identical",
        result.files_added, result.files_removed, result.files_changed, result.files_identical
    );

    Ok(result)
}

/// Collect all files matching optional extension filter
fn collect_files(dir: &Path, ext_filter: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !dir.exists() {
        return Err(anyhow::anyhow!("Directory not found: {}", dir.display()));
    }

    fn traverse(dir: &Path, ext_filter: Option<&str>, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = ext_filter {
                    if path.extension().and_then(|e| e.to_str())
                        == Some(ext.trim_start_matches('.'))
                    {
                        files.push(path);
                    }
                } else {
                    files.push(path);
                }
            } else if path.is_dir() {
                traverse(&path, ext_filter, files)?;
            }
        }
        Ok(())
    }

    traverse(dir, ext_filter, &mut files)?;
    files.sort();
    Ok(files)
}

/// Count the number of changes between two text documents
fn count_changes(old: &str, new: &str) -> usize {
    let diff = TextDiff::from_lines(old, new);
    diff.iter_all_changes()
        .filter(|change| matches!(change.tag(), ChangeTag::Delete | ChangeTag::Insert))
        .count()
}

/// Print a unified diff-style output for changed files
fn print_diff(old: &str, new: &str, rel_path: &Path, _context: usize) {
    let diff = TextDiff::from_lines(old, new);

    println!("--- a/{}", rel_path.display());
    println!("+++ b/{}", rel_path.display());

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => print!("- {}", change),
            ChangeTag::Insert => print!("+ {}", change),
            ChangeTag::Equal => print!("  {}", change),
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_diff_config() {
        let config = DiffConfig::new(PathBuf::from("/tmp/old"), PathBuf::from("/tmp/new"))
            .with_extension_filter(".ts".to_string())
            .with_context_lines(5);

        assert_eq!(config.extension_filter, Some(".ts".to_string()));
        assert_eq!(config.context_lines, 5);
    }

    #[test]
    fn test_diff_identical_files() -> Result<()> {
        let old_dir = TempDir::new()?;
        let new_dir = TempDir::new()?;

        let content = "Hello\nWorld\n";
        fs::write(old_dir.path().join("file.txt"), content)?;
        fs::write(new_dir.path().join("file.txt"), content)?;

        let config = DiffConfig::new(old_dir.path().to_path_buf(), new_dir.path().to_path_buf());

        let result = diff_directories(&config)?;
        assert_eq!(result.files_identical, 1);
        assert_eq!(result.files_changed, 0);

        Ok(())
    }

    #[test]
    fn test_diff_changed_files() -> Result<()> {
        let old_dir = TempDir::new()?;
        let new_dir = TempDir::new()?;

        fs::write(old_dir.path().join("file.txt"), "Old content\n")?;
        fs::write(new_dir.path().join("file.txt"), "New content\n")?;

        let config = DiffConfig::new(old_dir.path().to_path_buf(), new_dir.path().to_path_buf());

        let result = diff_directories(&config)?;
        assert_eq!(result.files_changed, 1);
        assert!(result.total_changes > 0);

        Ok(())
    }

    #[test]
    fn test_diff_added_removed_files() -> Result<()> {
        let old_dir = TempDir::new()?;
        let new_dir = TempDir::new()?;

        fs::write(old_dir.path().join("removed.txt"), "content")?;
        fs::write(new_dir.path().join("added.txt"), "content")?;

        let config = DiffConfig::new(old_dir.path().to_path_buf(), new_dir.path().to_path_buf());

        let result = diff_directories(&config)?;
        assert_eq!(result.files_removed, 1);
        assert_eq!(result.files_added, 1);

        Ok(())
    }
}
