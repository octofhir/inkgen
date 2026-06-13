//! Canonical Type Map - single source of truth for all FHIR types.
//!
//! This module provides a complete map of all FHIR types across all loaded packages,
//! built from the CanonicalManager. It serves as the authoritative source for:
//! - Type name resolution (e.g., "Extension" → canonical URL)
//! - File stem calculation (e.g., "Extension" → "extension")
//! - Import path generation between types
//!
//! Unlike the previous multi-system approach, this provides ONE source of truth
//! that is built upfront and never gets out of sync.

use std::collections::HashMap;
use std::sync::Arc;

use octofhir_canonical_manager::CanonicalManager;
use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::package::{PackageId, StructureKind};

/// Information about a single FHIR type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeEntry {
    /// The canonical URL (e.g., "http://hl7.org/fhir/StructureDefinition/Extension")
    pub canonical_url: String,
    /// The type name in PascalCase (e.g., "Extension")
    pub type_name: String,
    /// The file stem in kebab-case (e.g., "extension")
    pub file_stem: String,
    /// The kind of structure (PrimitiveType, ComplexType, BaseResource, etc.)
    pub kind: StructureKind,
    /// The package this type belongs to
    pub package: PackageId,
    /// For profiles: the base type's canonical URL
    pub base_type: Option<String>,
    /// The raw type code from the StructureDefinition (e.g., "Extension" for profiles)
    pub type_code: Option<String>,
}

/// Single source of truth for all FHIR types across all packages.
///
/// Built from the CanonicalManager, this map provides fast lookups (<1ms)
/// for any type by canonical URL, type name, or file stem.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CanonicalTypeMap {
    /// Primary lookup: canonical_url → TypeEntry
    by_url: HashMap<String, TypeEntry>,
    /// Reverse lookup: type_name → canonical_url
    by_name: HashMap<String, String>,
    /// Reverse lookup: file_stem → canonical_url (for import resolution)
    by_stem: HashMap<String, String>,
}

impl CanonicalTypeMap {
    /// Create a new empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the type map from all StructureDefinitions in the CanonicalManager.
    ///
    /// This queries ALL packages and extracts type information, providing a complete
    /// picture of all available types before any filtering is applied.
    pub async fn from_manager(manager: &Arc<CanonicalManager>) -> CoreResult<Self> {
        let mut map = Self::new();

        // Get all installed packages
        let packages = manager.list_packages().await?;

        for package_spec in packages {
            // Parse package spec (name@version)
            let package_id = parse_package_spec(&package_spec)?;

            // Collect all StructureDefinitions from this package
            let mut offset = 0usize;
            loop {
                let builder = manager.search().await;
                let result = builder
                    .package(&package_spec)
                    .resource_type("StructureDefinition")
                    .offset(offset)
                    .limit(1000)
                    .execute()
                    .await?;

                if result.resources.is_empty() {
                    break;
                }

                for resource_match in &result.resources {
                    if let Some(entry) =
                        Self::extract_type_entry(&package_id, &resource_match.resource)
                    {
                        map.insert(entry);
                    }
                }

                offset += result.resources.len();
                if offset >= result.total_count {
                    break;
                }
            }
        }

        tracing::info!(
            "Built CanonicalTypeMap with {} types ({} by name, {} by stem)",
            map.by_url.len(),
            map.by_name.len(),
            map.by_stem.len()
        );

        Ok(map)
    }

    /// Insert a type entry into the map.
    ///
    /// For base types (not profiles), this also registers the type name and file stem mappings.
    /// Profiles don't overwrite base type mappings.
    pub(crate) fn insert(&mut self, entry: TypeEntry) {
        let canonical_url = entry.canonical_url.clone();
        let type_name = entry.type_name.clone();
        let file_stem = entry.file_stem.clone();
        let is_profile = entry.kind == StructureKind::Profile;

        // Always insert by URL
        self.by_url.insert(canonical_url.clone(), entry);

        // Only register name/stem mappings for non-profiles, or if not already mapped
        // This ensures base types take precedence over profiles
        if !is_profile {
            self.by_name
                .entry(type_name)
                .or_insert(canonical_url.clone());
            self.by_stem.entry(file_stem).or_insert(canonical_url);
        }
    }

    /// Extract a TypeEntry from a FHIR resource.
    fn extract_type_entry(
        package: &PackageId,
        resource: &octofhir_canonical_manager::package::FhirResource,
    ) -> Option<TypeEntry> {
        // Get canonical URL
        let canonical_url = resource.url.clone().or_else(|| {
            resource
                .content
                .get("url")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })?;

        // Determine structure kind
        let kind_raw = resource
            .content
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let derivation = resource.content.get("derivation").and_then(Value::as_str);
        let kind = match (kind_raw, derivation) {
            (_, Some("constraint")) => StructureKind::Profile,
            ("resource", _) => StructureKind::BaseResource,
            ("complex-type", _) => StructureKind::ComplexType,
            ("primitive-type", _) => StructureKind::PrimitiveType,
            ("logical", _) => StructureKind::Logical,
            _ => StructureKind::Profile, // Default to profile if unknown
        };

        // Get type code and name
        let type_code = resource
            .content
            .get("type")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        let name = resource
            .content
            .get("name")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        let id = resource
            .content
            .get("id")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .or_else(|| {
                // Extract from URL as fallback
                canonical_url.rsplit('/').next().map(|s| s.to_string())
            })?;

        // Get base type URL for profiles
        let base_type = resource
            .content
            .get("baseDefinition")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        // Determine type name:
        // - For base types: use type_code or name or id
        // - For profiles: use id (the profile's own identifier)
        let type_name = if kind == StructureKind::Profile {
            // Profiles use their own id
            pascal_case(&id)
        } else {
            // Base types use type_code if available
            pascal_case(type_code.as_deref().or(name.as_deref()).unwrap_or(&id))
        };

        // File stem is always based on the id
        let file_stem = to_file_stem(&id);

        Some(TypeEntry {
            canonical_url,
            type_name,
            file_stem,
            kind,
            package: package.clone(),
            base_type,
            type_code,
        })
    }

    /// Get a type entry by canonical URL.
    pub fn get_by_url(&self, url: &str) -> Option<&TypeEntry> {
        self.by_url.get(url)
    }

    /// Get a type entry by type name (e.g., "Extension").
    pub fn get_by_name(&self, name: &str) -> Option<&TypeEntry> {
        self.by_name.get(name).and_then(|url| self.by_url.get(url))
    }

    /// Get a type entry by file stem (e.g., "extension").
    pub fn get_by_stem(&self, stem: &str) -> Option<&TypeEntry> {
        self.by_stem.get(stem).and_then(|url| self.by_url.get(url))
    }

    /// True if a raw FHIR type code (e.g. `date`, `string`) is a primitive type,
    /// per the package's own `StructureDefinition` kinds. Type entries are keyed
    /// by PascalCase type name, so the code is normalized the same way.
    pub fn is_primitive_code(&self, type_code: &str) -> bool {
        self.get_by_name(&pascal_case(type_code))
            .map(|entry| entry.kind == StructureKind::PrimitiveType)
            .unwrap_or(false)
    }

    /// Get the file stem for a type name.
    pub fn stem_for_name(&self, name: &str) -> Option<&str> {
        self.get_by_name(name).map(|e| e.file_stem.as_str())
    }

    /// Get all type entries.
    pub fn all_types(&self) -> impl Iterator<Item = &TypeEntry> {
        self.by_url.values()
    }

    /// Get all type names.
    pub fn all_names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(|s| s.as_str())
    }

    /// Check if a type name exists.
    pub fn contains_name(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// Check if a canonical URL exists.
    pub fn contains_url(&self, url: &str) -> bool {
        self.by_url.contains_key(url)
    }

    /// Get the number of types in the map.
    pub fn len(&self) -> usize {
        self.by_url.len()
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.by_url.is_empty()
    }

    /// Calculate the import path from one type to another.
    ///
    /// # Arguments
    /// * `from_package` - The package folder of the importing file
    /// * `from_stem` - The file stem of the importing file
    /// * `to_type_name` - The type name to import (e.g., "Extension")
    ///
    /// # Returns
    /// The relative import path (e.g., "./extension" or "../other-package/extension")
    pub fn import_path(
        &self,
        from_package: &str,
        _from_stem: &str,
        to_type_name: &str,
    ) -> Option<String> {
        let entry = self.get_by_name(to_type_name)?;

        // Get the target package folder
        let to_package = crate::config::sanitize_package_name(&entry.package.name);

        if from_package == to_package {
            // Same package - relative import
            Some(format!("./{}", entry.file_stem))
        } else {
            // Cross-package import
            Some(format!("../{}/{}", to_package, entry.file_stem))
        }
    }
}

/// Parse a package spec (name@version) into a PackageId.
fn parse_package_spec(spec: &str) -> CoreResult<PackageId> {
    use std::str::FromStr;
    PackageId::from_str(spec).map_err(|detail| CoreError::InvalidPackage { detail })
}

/// Convert an identifier to PascalCase.
fn pascal_case(value: &str) -> String {
    split_tokens(value)
        .into_iter()
        .map(|token| {
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Convert an identifier to a file stem (kebab-case).
fn to_file_stem(identifier: &str) -> String {
    split_tokens(identifier)
        .into_iter()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

/// Split an identifier into tokens for case conversion.
fn split_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_was_lower = false;

    for ch in value.chars() {
        if ch == '-' || ch == '_' || ch == '.' || ch == ' ' {
            // Separator - finish current token
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            prev_was_lower = false;
        } else if ch.is_ascii_uppercase() && prev_was_lower {
            // CamelCase boundary
            tokens.push(std::mem::take(&mut current));
            current.push(ch.to_ascii_lowercase());
            prev_was_lower = false;
        } else {
            current.push(ch.to_ascii_lowercase());
            prev_was_lower = ch.is_ascii_lowercase();
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pascal_case() {
        assert_eq!(pascal_case("extension"), "Extension");
        assert_eq!(pascal_case("Extension"), "Extension");
        assert_eq!(pascal_case("my-type"), "MyType");
        assert_eq!(pascal_case("my_type"), "MyType");
        assert_eq!(pascal_case("MyType"), "MyType");
        assert_eq!(pascal_case("myType"), "MyType");
        assert_eq!(pascal_case("11179-objectClass"), "11179ObjectClass");
    }

    #[test]
    fn test_to_file_stem() {
        assert_eq!(to_file_stem("Extension"), "extension");
        assert_eq!(to_file_stem("MyType"), "my-type");
        assert_eq!(to_file_stem("my_type"), "my-type");
        assert_eq!(to_file_stem("my-type"), "my-type");
        assert_eq!(to_file_stem("11179-objectClass"), "11179-object-class");
    }

    #[test]
    fn test_type_entry_insert_precedence() {
        let mut map = CanonicalTypeMap::new();

        // Insert base type first
        map.insert(TypeEntry {
            canonical_url: "http://hl7.org/fhir/StructureDefinition/Extension".to_string(),
            type_name: "Extension".to_string(),
            file_stem: "extension".to_string(),
            kind: StructureKind::ComplexType,
            package: PackageId::new("hl7.fhir.r4.core", "4.0.1"),
            base_type: None,
            type_code: Some("Extension".to_string()),
        });

        // Insert profile that would claim same name
        map.insert(TypeEntry {
            canonical_url: "http://example.org/Extension-profile".to_string(),
            type_name: "Extension".to_string(), // Same name!
            file_stem: "extension-profile".to_string(),
            kind: StructureKind::Profile,
            package: PackageId::new("example", "1.0.0"),
            base_type: Some("http://hl7.org/fhir/StructureDefinition/Extension".to_string()),
            type_code: Some("Extension".to_string()),
        });

        // Base type should still be mapped by name
        let entry = map.get_by_name("Extension").unwrap();
        assert_eq!(
            entry.canonical_url,
            "http://hl7.org/fhir/StructureDefinition/Extension"
        );
        assert_eq!(entry.kind, StructureKind::ComplexType);
    }

    #[test]
    fn is_primitive_code_reads_package_kinds() {
        let mut map = CanonicalTypeMap::new();
        map.insert(TypeEntry {
            canonical_url: "http://hl7.org/fhir/StructureDefinition/date".to_string(),
            type_name: "Date".to_string(),
            file_stem: "date".to_string(),
            kind: StructureKind::PrimitiveType,
            package: PackageId::new("hl7.fhir.r4.core", "4.0.1"),
            base_type: None,
            type_code: Some("date".to_string()),
        });
        map.insert(TypeEntry {
            canonical_url: "http://hl7.org/fhir/StructureDefinition/HumanName".to_string(),
            type_name: "HumanName".to_string(),
            file_stem: "human-name".to_string(),
            kind: StructureKind::ComplexType,
            package: PackageId::new("hl7.fhir.r4.core", "4.0.1"),
            base_type: None,
            type_code: Some("HumanName".to_string()),
        });

        // Raw lowercase codes resolve through the PascalCase index.
        assert!(map.is_primitive_code("date"));
        assert!(!map.is_primitive_code("HumanName"));
        // Unknown codes (e.g. the fhirpath System.* types, absent from the
        // package as StructureDefinitions) are not primitives.
        assert!(!map.is_primitive_code("http://hl7.org/fhirpath/System.String"));
    }
}
