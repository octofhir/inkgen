//! Shared template helper functions for all language backends.
//!
//! This module provides language-agnostic utilities that can be used by any backend.
//! These are NOT tied to any specific programming language.

use std::collections::HashMap;
use tera::{Error as TeraError, Function, Result as TeraResult, Value};

use crate::sanitize_package_name;

/// Calculate a relative import path between two files.
///
/// This is language-agnostic and works for any language with file-based imports
/// (TypeScript, Rust, Python, Go, etc.).
///
/// # Usage in templates
///
/// ```tera
/// import { Patient } from '{{ import_path(from="observation.ts", to="patient.ts") }}';
/// // import { Patient } from './patient';
/// ```
///
/// # Arguments
///
/// - `from` (required): Source file path
/// - `to` (required): Target file path
///
/// # Returns
///
/// String containing the relative import path (without extension).
pub struct ImportPathFunction;

impl Function for ImportPathFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let from = args
            .get("from")
            .and_then(Value::as_str)
            .ok_or_else(|| TeraError::msg("import_path requires 'from' argument"))?;

        let to = args
            .get("to")
            .and_then(Value::as_str)
            .ok_or_else(|| TeraError::msg("import_path requires 'to' argument"))?;

        let path = calculate_relative_import(from, to);

        Ok(Value::String(path))
    }
}

/// Sanitize a package name to a valid folder name.
///
/// This is language-agnostic and used by all backends.
///
/// # Usage in templates
///
/// ```tera
/// const packageDir = '{{ package_folder(name="hl7.fhir.r4.core") }}';
/// // const packageDir = 'r4-core';
/// ```
///
/// # Arguments
///
/// - `name` (required): Package name to sanitize
///
/// # Returns
///
/// String containing the sanitized folder name.
pub struct PackageFolderFunction;

impl Function for PackageFolderFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let name = args
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| TeraError::msg("package_folder requires 'name' argument"))?;

        let folder = sanitize_package_name(name);

        Ok(Value::String(folder))
    }
}

/// Calculate a relative import path between two files.
///
/// Handles:
/// - Same directory: `./target`
/// - Parent directory: `../target`
/// - Cross-package: `../../other-package/target`
///
/// # Examples
///
/// ```
/// use inkgen_core::template_helpers::calculate_relative_import;
///
/// assert_eq!(
///     calculate_relative_import("observation.ts", "patient.ts"),
///     "./patient"
/// );
///
/// assert_eq!(
///     calculate_relative_import("profiles/us-core-patient.ts", "patient.ts"),
///     "../patient"
/// );
/// ```
pub fn calculate_relative_import(from: &str, to: &str) -> String {
    // Strip common file extensions if present
    let from = from
        .trim_end_matches(".ts")
        .trim_end_matches(".rs")
        .trim_end_matches(".py")
        .trim_end_matches(".go");
    let to = to
        .trim_end_matches(".ts")
        .trim_end_matches(".rs")
        .trim_end_matches(".py")
        .trim_end_matches(".go");

    // Split paths into components
    let from_parts: Vec<&str> = from.split('/').collect();
    let to_parts: Vec<&str> = to.split('/').collect();

    // Calculate common prefix length
    let common_len = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Calculate how many levels up we need to go
    let up_levels = from_parts.len() - common_len - 1; // -1 because we exclude the file itself

    // Build the import path
    let mut path = String::new();

    if up_levels == 0 {
        // Same directory
        path.push_str("./");
    } else {
        // Go up directories
        for _ in 0..up_levels {
            path.push_str("../");
        }
    }

    // Add the remaining path components from 'to'
    let remaining: Vec<&str> = to_parts.iter().skip(common_len).copied().collect();
    path.push_str(&remaining.join("/"));

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_relative_import_same_dir() {
        assert_eq!(
            calculate_relative_import("observation.ts", "patient.ts"),
            "./patient"
        );
    }

    #[test]
    fn test_calculate_relative_import_parent_dir() {
        assert_eq!(
            calculate_relative_import("profiles/us-core-patient.ts", "patient.ts"),
            "../patient"
        );
    }

    #[test]
    fn test_calculate_relative_import_nested() {
        assert_eq!(
            calculate_relative_import("a/b/c/file.ts", "a/b/target.ts"),
            "../target"
        );
    }

    #[test]
    fn test_calculate_relative_import_cross_package() {
        assert_eq!(
            calculate_relative_import("us-core/observation.ts", "r4-core/patient.ts"),
            "../r4-core/patient"
        );
    }

    #[test]
    fn test_calculate_relative_import_rust_files() {
        assert_eq!(
            calculate_relative_import("observation.rs", "patient.rs"),
            "./patient"
        );
    }

    #[test]
    fn test_import_path_function() {
        let func = ImportPathFunction;
        let mut args = HashMap::new();

        args.insert("from".to_string(), Value::String("observation.ts".to_string()));
        args.insert("to".to_string(), Value::String("patient.ts".to_string()));
        let result = func.call(&args).unwrap();
        assert_eq!(result, Value::String("./patient".to_string()));
    }

    #[test]
    fn test_package_folder_function() {
        let func = PackageFolderFunction;
        let mut args = HashMap::new();

        args.insert(
            "name".to_string(),
            Value::String("hl7.fhir.r4.core".to_string()),
        );
        let result = func.call(&args).unwrap();
        assert_eq!(result, Value::String("r4-core".to_string()));
    }

    #[test]
    fn test_function_missing_args() {
        let func = ImportPathFunction;
        let args = HashMap::new();
        assert!(func.call(&args).is_err());

        let func = PackageFolderFunction;
        let args = HashMap::new();
        assert!(func.call(&args).is_err());
    }
}
