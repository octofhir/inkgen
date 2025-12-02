use std::collections::HashMap;

/// Global registry tracking all types across all packages.
/// Used for resolving cross-package imports.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    /// Map type_name → (package_folder, file_stem)
    types: HashMap<String, TypeInfo>,
}

#[derive(Debug, Clone)]
struct TypeInfo {
    package_folder: String,
    file_stem: String,
}

impl TypeRegistry {
    /// Creates a new empty type registry
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    /// Register a type with its package and file location
    pub fn register(&mut self, type_name: String, package_folder: String, file_stem: String) {
        self.types.insert(
            type_name,
            TypeInfo {
                package_folder,
                file_stem,
            },
        );
    }

    /// Look up where a type is defined
    pub fn get(&self, type_name: &str) -> Option<(&str, &str)> {
        self.types
            .get(type_name)
            .map(|info| (info.package_folder.as_str(), info.file_stem.as_str()))
    }

    /// Check if a type is registered
    pub fn contains(&self, type_name: &str) -> bool {
        self.types.contains_key(type_name)
    }

    /// Get the total number of registered types
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

/// Calculate relative import path between packages or within the same package.
///
/// # Arguments
///
/// * `from_package_folder` - Source package folder (e.g., "r4-core" or "us-core")
/// * `from_subfolder` - Subfolder within source package (e.g., "profiles" or "")
/// * `to_package_folder` - Target package folder
/// * `to_file_stem` - Target file stem (without .ts extension)
///
/// # Returns
///
/// Import path (e.g., "./patient" or "../../r4-core/patient")
///
/// # Examples
///
/// ```
/// use inkgen_typescript::imports::calculate_import_path;
///
/// // Same package, no subfolders
/// // from: r4-core/observation.ts  to: r4-core/patient.ts
/// let path = calculate_import_path("r4-core", "", "r4-core", "patient");
/// assert_eq!(path, "./patient");
///
/// // Same package, from subfolder
/// // from: r4-core/profiles/us-core-patient.ts  to: r4-core/patient.ts
/// let path = calculate_import_path("r4-core", "profiles", "r4-core", "patient");
/// assert_eq!(path, "../patient");
///
/// // Cross-package
/// // from: us-core/profiles/us-core-patient.ts  to: r4-core/patient.ts
/// let path = calculate_import_path("us-core", "profiles", "r4-core", "patient");
/// assert_eq!(path, "../../r4-core/patient");
/// ```
pub fn calculate_import_path(
    from_package_folder: &str,
    from_subfolder: &str,
    to_package_folder: &str,
    to_file_stem: &str,
) -> String {
    if from_package_folder == to_package_folder {
        // Same package - calculate relative path
        if from_subfolder.is_empty() {
            // Same package, no subfolder navigation needed
            format!("./{}", to_file_stem)
        } else {
            // Navigate up from subfolder
            format!("../{}", to_file_stem)
        }
    } else {
        // Cross-package import
        let depth = if from_subfolder.is_empty() {
            1 // Just the package folder
        } else {
            2 // Package folder + subfolder
        };

        let up_path = "../".repeat(depth);
        format!("{}{}/{}", up_path, to_package_folder, to_file_stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_registry() {
        let mut registry = TypeRegistry::new();

        registry.register("Patient".to_string(), "r4-core".to_string(), "patient".to_string());
        registry.register("Observation".to_string(), "r4-core".to_string(), "observation".to_string());

        assert!(registry.contains("Patient"));
        assert!(registry.contains("Observation"));
        assert!(!registry.contains("Condition"));

        let (pkg, stem) = registry.get("Patient").unwrap();
        assert_eq!(pkg, "r4-core");
        assert_eq!(stem, "patient");

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_calculate_import_path_same_package() {
        let path = calculate_import_path("r4-core", "", "r4-core", "patient");
        assert_eq!(path, "./patient");
    }

    #[test]
    fn test_calculate_import_path_with_subfolder() {
        let path = calculate_import_path("r4-core", "profiles", "r4-core", "patient");
        assert_eq!(path, "../patient");
    }

    #[test]
    fn test_calculate_import_path_cross_package() {
        let path = calculate_import_path("us-core", "", "r4-core", "patient");
        assert_eq!(path, "../r4-core/patient");
    }

    #[test]
    fn test_calculate_import_path_cross_package_with_subfolder() {
        let path = calculate_import_path("us-core", "profiles", "r4-core", "patient");
        assert_eq!(path, "../../r4-core/patient");
    }
}
