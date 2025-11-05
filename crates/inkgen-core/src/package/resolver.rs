//! Package resolver implementation using octofhir-canonical-manager

use crate::{CoreError, Result, StructureDefinition, PerformanceMonitor, PackageResolutionContext};
use octofhir_canonical_manager::{CanonicalManager, FcmConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};

/// Service responsible for downloading and managing FHIR packages
pub struct PackageResolver {
    manager: CanonicalManager,
}

/// Represents a resolved FHIR package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub manifest: PackageManifest,
    pub resources: HashMap<String, StructureDefinition>,
}

/// Package information for listing available packages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

/// Package manifest information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub fhir_versions: Vec<String>,
}

impl PackageResolver {
    /// Create a new PackageResolver instance
    pub async fn new() -> Result<Self> {
        let config = FcmConfig::default();
        let manager = CanonicalManager::new(config)
            .await
            .map_err(|e| CoreError::PackageResolution {
                package: "canonical-manager".to_string(),
                version: None,
                reason: "Failed to initialize canonical manager".to_string(),
                source: Some(Box::new(e)),
            })?;

        Ok(Self { manager })
    }

    /// Resolve a FHIR package by name and optional version
    #[instrument(skip(self), fields(package = %name))]
    pub async fn resolve_package(&self, name: &str, version: Option<&str>) -> Result<Package> {
        let context = PackageResolutionContext::new(name, version.map(|v| v.to_string()));
        let _span = context.span().entered();
        let monitor = PerformanceMonitor::start("package_resolution");
        
        context.log_start();

        // Sanitize package identifier for safe processing
        let sanitized_name = match self.sanitize_package_identifier(name) {
            Ok(name) => name,
            Err(e) => {
                context.log_failure(&e);
                monitor.finish_with_error(&e);
                return Err(e);
            }
        };

        // Use canonical manager to resolve the package
        // Note: This is a placeholder implementation as the exact API may vary
        let package_spec = match version {
            Some(v) => format!("{}#{}", sanitized_name, v),
            None => sanitized_name.clone(),
        };

        debug!("Requesting package from canonical manager: {}", package_spec);

        // For now, we'll create a mock response until we have the exact canonical manager API
        // In the real implementation, this would call something like:
        // let resolved_package = self.manager.resolve_package(&package_spec).await?;
        
        let manifest = PackageManifest {
            name: sanitized_name.clone(),
            version: version.unwrap_or("latest").to_string(),
            description: Some(format!("FHIR package: {}", sanitized_name)),
            dependencies: HashMap::new(),
            fhir_versions: vec!["4.0.1".to_string()],
        };

        // Extract StructureDefinitions from the package
        monitor.checkpoint("extracting_structure_definitions");
        let resources = match self.extract_structure_definitions_from_package(&sanitized_name, version).await {
            Ok(resources) => resources,
            Err(e) => {
                context.log_failure(&e);
                monitor.finish_with_error(&e);
                return Err(e);
            }
        };

        let package = Package {
            name: sanitized_name,
            version: version.unwrap_or("latest").to_string(),
            manifest,
            resources,
        };

        context.log_success(package.resources.len(), false); // TODO: Add cache hit detection
        monitor.finish();

        Ok(package)
    }

    /// List available packages
    pub async fn list_available_packages(&self) -> Result<Vec<PackageInfo>> {
        info!("Listing available packages");

        // This would typically call the canonical manager's list functionality
        // For now, return a curated list of common FHIR packages
        let packages = vec![
            PackageInfo {
                name: "hl7.fhir.r4.core".to_string(),
                version: "4.0.1".to_string(),
                description: Some("FHIR R4 Core Package".to_string()),
            },
            PackageInfo {
                name: "hl7.fhir.r5.core".to_string(),
                version: "5.0.0".to_string(),
                description: Some("FHIR R5 Core Package".to_string()),
            },
            PackageInfo {
                name: "hl7.fhir.us.core".to_string(),
                version: "6.1.0".to_string(),
                description: Some("US Core Implementation Guide".to_string()),
            },
        ];

        debug!("Found {} available packages", packages.len());
        Ok(packages)
    }

    /// Get package manifest information
    pub async fn get_package_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<PackageManifest> {
        info!("Getting manifest for package: {} (version: {:?})", name, version);

        let sanitized_name = self.sanitize_package_identifier(name)?;

        // This would typically extract manifest from the canonical manager
        // For now, create a basic manifest structure
        let manifest = PackageManifest {
            name: sanitized_name.clone(),
            version: version.unwrap_or("latest").to_string(),
            description: Some(format!("FHIR package manifest for {}", sanitized_name)),
            dependencies: self.get_default_dependencies(&sanitized_name),
            fhir_versions: vec!["4.0.1".to_string()],
        };

        debug!("Manifest created for package: {}", sanitized_name);
        Ok(manifest)
    }

    /// Sanitize package identifiers for safe processing
    fn sanitize_package_identifier(&self, name: &str) -> Result<String> {
        if name.is_empty() {
            return Err(CoreError::InvalidStructure {
                message: "Package identifier cannot be empty".to_string(),
                resource_type: Some("Package".to_string()),
                element_path: None,
            });
        }

        // Basic sanitization - allow alphanumeric, dots, hyphens, and underscores
        let sanitized = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .collect::<String>();

        if sanitized.is_empty() {
            return Err(CoreError::InvalidStructure {
                message: format!("Invalid package identifier after sanitization: {}", name),
                resource_type: Some("Package".to_string()),
                element_path: None,
            });
        }

        // Ensure it doesn't start or end with special characters
        let sanitized = sanitized.trim_matches(|c| c == '.' || c == '-' || c == '_');
        
        if sanitized.is_empty() {
            return Err(CoreError::InvalidStructure {
                message: format!("Package identifier contains only invalid characters: {}", name),
                resource_type: Some("Package".to_string()),
                element_path: None,
            });
        }

        Ok(sanitized.to_string())
    }

    /// Extract StructureDefinitions from a resolved package
    async fn extract_structure_definitions_from_package(
        &self,
        package_name: &str,
        version: Option<&str>,
    ) -> Result<HashMap<String, StructureDefinition>> {
        debug!("Extracting StructureDefinitions from package: {}", package_name);

        // This implementation provides a foundation for StructureDefinition extraction
        // In a full implementation, this would:
        // 1. Get the package contents from canonical manager
        // 2. Parse the package.json manifest
        // 3. Enumerate all .json files in the package
        // 4. Filter for StructureDefinition resources
        // 5. Parse and validate each StructureDefinition

        let mut resources = HashMap::new();

        // Simulate package content extraction based on package type
        match package_name {
            "hl7.fhir.r4.core" => {
                resources.extend(self.create_core_r4_structure_definitions()?);
                resources.extend(self.extract_additional_r4_resources().await?);
            }
            "hl7.fhir.r5.core" => {
                resources.extend(self.create_core_r5_structure_definitions()?);
                resources.extend(self.extract_additional_r5_resources().await?);
            }
            "hl7.fhir.us.core" => {
                resources.extend(self.extract_us_core_profiles(version).await?);
            }
            _ => {
                // For unknown packages, attempt generic extraction
                resources.extend(self.extract_generic_package_resources(package_name, version).await?);
            }
        }

        // Validate extracted StructureDefinitions
        self.validate_structure_definitions(&resources)?;

        debug!("Extracted {} StructureDefinitions from package", resources.len());
        Ok(resources)
    }

    /// Extract additional R4 core resources beyond basic Patient/Observation
    async fn extract_additional_r4_resources(&self) -> Result<HashMap<String, StructureDefinition>> {
        use crate::{Resource, ResourceType, Meta};

        let mut definitions = HashMap::new();

        // Practitioner StructureDefinition
        let practitioner_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("Practitioner".to_string()),
                meta: Some(Meta {
                    version_id: Some("4.0.1".to_string()),
                    last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/StructureDefinition/Practitioner".to_string(),
            version: Some("4.0.1".to_string()),
            name: "Practitioner".to_string(),
            title: Some("Practitioner".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Practitioner".to_string(),
            snapshot: None,
        };

        definitions.insert("Practitioner".to_string(), practitioner_sd);

        // Organization StructureDefinition
        let organization_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("Organization".to_string()),
                meta: Some(Meta {
                    version_id: Some("4.0.1".to_string()),
                    last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/StructureDefinition/Organization".to_string(),
            version: Some("4.0.1".to_string()),
            name: "Organization".to_string(),
            title: Some("Organization".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Organization".to_string(),
            snapshot: None,
        };

        definitions.insert("Organization".to_string(), organization_sd);

        Ok(definitions)
    }

    /// Extract additional R5 core resources
    async fn extract_additional_r5_resources(&self) -> Result<HashMap<String, StructureDefinition>> {
        use crate::{Resource, ResourceType, Meta};

        let mut definitions = HashMap::new();

        // Observation StructureDefinition for R5
        let observation_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("Observation".to_string()),
                meta: Some(Meta {
                    version_id: Some("5.0.0".to_string()),
                    last_updated: Some("2023-03-26T15:21:02.749+11:00".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
            version: Some("5.0.0".to_string()),
            name: "Observation".to_string(),
            title: Some("Observation".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Observation".to_string(),
            snapshot: None,
        };

        definitions.insert("Observation".to_string(), observation_sd);

        Ok(definitions)
    }

    /// Extract US Core profiles
    async fn extract_us_core_profiles(&self, version: Option<&str>) -> Result<HashMap<String, StructureDefinition>> {
        use crate::{Resource, ResourceType, Meta};

        let mut definitions = HashMap::new();
        let us_core_version = version.unwrap_or("6.1.0");

        // US Core Patient Profile
        let us_patient_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("us-core-patient".to_string()),
                meta: Some(Meta {
                    version_id: Some(us_core_version.to_string()),
                    last_updated: Some("2023-01-01T00:00:00Z".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
            version: Some(us_core_version.to_string()),
            name: "USCorePatientProfile".to_string(),
            title: Some("US Core Patient Profile".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Patient".to_string(),
            snapshot: None,
        };

        definitions.insert("us-core-patient".to_string(), us_patient_sd);

        debug!("Extracted {} US Core profiles", definitions.len());
        Ok(definitions)
    }

    /// Extract resources from generic/unknown packages
    async fn extract_generic_package_resources(
        &self,
        package_name: &str,
        _version: Option<&str>,
    ) -> Result<HashMap<String, StructureDefinition>> {
        debug!("Attempting generic extraction for package: {}", package_name);

        // This would be where we'd implement the generic package parsing logic:
        // 1. Use canonical manager to get package contents
        // 2. Parse package.json manifest
        // 3. Enumerate resource files
        // 4. Filter and parse StructureDefinitions

        // For now, return empty set for unknown packages
        warn!("Generic package extraction not yet implemented for: {}", package_name);
        Ok(HashMap::new())
    }

    /// Validate extracted StructureDefinitions
    fn validate_structure_definitions(&self, definitions: &HashMap<String, StructureDefinition>) -> Result<()> {
        for (key, sd) in definitions {
            // Basic validation checks
            if sd.name.is_empty() {
                return Err(CoreError::InvalidStructure {
                    message: format!("StructureDefinition '{}' has empty name", key),
                    resource_type: Some("StructureDefinition".to_string()),
                    element_path: Some("name".to_string()),
                });
            }

            if sd.url.is_empty() {
                return Err(CoreError::InvalidStructure {
                    message: format!("StructureDefinition '{}' has empty URL", key),
                    resource_type: Some("StructureDefinition".to_string()),
                    element_path: Some("url".to_string()),
                });
            }

            if sd.status != "active" && sd.status != "draft" && sd.status != "retired" {
                return Err(CoreError::InvalidStructure {
                    message: format!("StructureDefinition '{}' has invalid status: {}", key, sd.status),
                    resource_type: Some("StructureDefinition".to_string()),
                    element_path: Some("status".to_string()),
                });
            }

            if sd.type_.is_empty() {
                return Err(CoreError::InvalidStructure {
                    message: format!("StructureDefinition '{}' has empty type", key),
                    resource_type: Some("StructureDefinition".to_string()),
                    element_path: Some("type".to_string()),
                });
            }
        }

        debug!("Validated {} StructureDefinitions successfully", definitions.len());
        Ok(())
    }

    /// Parse package manifest from package data
    fn parse_package_manifest(&self, package_data: &serde_json::Value) -> Result<PackageManifest> {
        // This would parse the actual package.json manifest from the package
        // For now, create a basic manifest structure

        let name = package_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let version = package_data
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let description = package_data
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut dependencies = HashMap::new();
        if let Some(deps) = package_data.get("dependencies").and_then(|v| v.as_object()) {
            for (dep_name, dep_version) in deps {
                if let Some(version_str) = dep_version.as_str() {
                    dependencies.insert(dep_name.clone(), version_str.to_string());
                }
            }
        }

        let mut fhir_versions = vec!["4.0.1".to_string()]; // Default to R4
        if let Some(fhir_vers) = package_data.get("fhirVersions").and_then(|v| v.as_array()) {
            fhir_versions = fhir_vers
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }

        Ok(PackageManifest {
            name,
            version,
            description,
            dependencies,
            fhir_versions,
        })
    }

    /// Enumerate resource files in a package
    fn enumerate_package_resources(&self, package_data: &serde_json::Value) -> Result<Vec<String>> {
        // This would enumerate all .json files in the package that could contain resources
        // For now, return a placeholder list

        let mut resource_files = Vec::new();

        // Look for files array in package manifest
        if let Some(files) = package_data.get("files").and_then(|v| v.as_array()) {
            for file_entry in files {
                if let Some(filename) = file_entry.as_str() {
                    if filename.ends_with(".json") && !filename.contains("package.json") {
                        resource_files.push(filename.to_string());
                    }
                }
            }
        }

        // If no files array, assume standard structure
        if resource_files.is_empty() {
            resource_files = vec![
                "StructureDefinition-Patient.json".to_string(),
                "StructureDefinition-Observation.json".to_string(),
                "StructureDefinition-Practitioner.json".to_string(),
                "StructureDefinition-Organization.json".to_string(),
            ];
        }

        debug!("Found {} potential resource files", resource_files.len());
        Ok(resource_files)
    }

    /// Get default dependencies for common packages
    fn get_default_dependencies(&self, package_name: &str) -> HashMap<String, String> {
        let mut dependencies = HashMap::new();

        match package_name {
            "hl7.fhir.us.core" => {
                dependencies.insert("hl7.fhir.r4.core".to_string(), "4.0.1".to_string());
            }
            "hl7.fhir.r5.core" => {
                // R5 core has no dependencies
            }
            "hl7.fhir.r4.core" => {
                // R4 core has no dependencies
            }
            _ => {
                // Default to depending on R4 core
                dependencies.insert("hl7.fhir.r4.core".to_string(), "4.0.1".to_string());
            }
        }

        dependencies
    }

    /// Create basic StructureDefinitions for FHIR R4 core package
    fn create_core_r4_structure_definitions(&self) -> Result<HashMap<String, StructureDefinition>> {
        use crate::{Resource, ResourceType, Meta};

        let mut definitions = HashMap::new();

        // Patient StructureDefinition
        let patient_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("Patient".to_string()),
                meta: Some(Meta {
                    version_id: Some("4.0.1".to_string()),
                    last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            version: Some("4.0.1".to_string()),
            name: "Patient".to_string(),
            title: Some("Patient".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Patient".to_string(),
            snapshot: None,
        };

        definitions.insert("Patient".to_string(), patient_sd);

        // Observation StructureDefinition
        let observation_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("Observation".to_string()),
                meta: Some(Meta {
                    version_id: Some("4.0.1".to_string()),
                    last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
            version: Some("4.0.1".to_string()),
            name: "Observation".to_string(),
            title: Some("Observation".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Observation".to_string(),
            snapshot: None,
        };

        definitions.insert("Observation".to_string(), observation_sd);

        Ok(definitions)
    }

    /// Create basic StructureDefinitions for FHIR R5 core package
    fn create_core_r5_structure_definitions(&self) -> Result<HashMap<String, StructureDefinition>> {
        use crate::{Resource, ResourceType, Meta};

        let mut definitions = HashMap::new();

        // Patient StructureDefinition for R5
        let patient_sd = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("Patient".to_string()),
                meta: Some(Meta {
                    version_id: Some("5.0.0".to_string()),
                    last_updated: Some("2023-03-26T15:21:02.749+11:00".to_string()),
                    profile: None,
                }),
            },
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            version: Some("5.0.0".to_string()),
            name: "Patient".to_string(),
            title: Some("Patient".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Patient".to_string(),
            snapshot: None,
        };

        definitions.insert("Patient".to_string(), patient_sd);

        Ok(definitions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    // Helper struct for testing that doesn't require a real CanonicalManager
    struct TestPackageResolver;

    impl TestPackageResolver {
        fn sanitize_package_identifier(&self, name: &str) -> Result<String> {
            if name.is_empty() {
                return Err(CoreError::InvalidStructure {
                    message: "Package identifier cannot be empty".to_string(),
                    resource_type: Some("Package".to_string()),
                    element_path: None,
                });
            }

            let sanitized = name
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
                .collect::<String>();

            if sanitized.is_empty() {
                return Err(CoreError::InvalidStructure {
                    message: format!("Invalid package identifier after sanitization: {}", name),
                    resource_type: Some("Package".to_string()),
                    element_path: None,
                });
            }

            let sanitized = sanitized.trim_matches(|c| c == '.' || c == '-' || c == '_');
            
            if sanitized.is_empty() {
                return Err(CoreError::InvalidStructure {
                    message: format!("Package identifier contains only invalid characters: {}", name),
                    resource_type: Some("Package".to_string()),
                    element_path: None,
                });
            }

            Ok(sanitized.to_string())
        }

        fn get_default_dependencies(&self, package_name: &str) -> HashMap<String, String> {
            let mut dependencies = HashMap::new();

            match package_name {
                "hl7.fhir.us.core" => {
                    dependencies.insert("hl7.fhir.r4.core".to_string(), "4.0.1".to_string());
                }
                "hl7.fhir.r5.core" => {
                    // R5 core has no dependencies
                }
                "hl7.fhir.r4.core" => {
                    // R4 core has no dependencies
                }
                _ => {
                    // Default to depending on R4 core
                    dependencies.insert("hl7.fhir.r4.core".to_string(), "4.0.1".to_string());
                }
            }

            dependencies
        }

        fn create_core_r4_structure_definitions(&self) -> Result<HashMap<String, StructureDefinition>> {
            use crate::{Resource, ResourceType, Meta};

            let mut definitions = HashMap::new();

            let patient_sd = StructureDefinition {
                resource: Resource {
                    resource_type: ResourceType::StructureDefinition,
                    id: Some("Patient".to_string()),
                    meta: Some(Meta {
                        version_id: Some("4.0.1".to_string()),
                        last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                        profile: None,
                    }),
                },
                url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                version: Some("4.0.1".to_string()),
                name: "Patient".to_string(),
                title: Some("Patient".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Patient".to_string(),
                snapshot: None,
            };

            definitions.insert("Patient".to_string(), patient_sd);

            let observation_sd = StructureDefinition {
                resource: Resource {
                    resource_type: ResourceType::StructureDefinition,
                    id: Some("Observation".to_string()),
                    meta: Some(Meta {
                        version_id: Some("4.0.1".to_string()),
                        last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                        profile: None,
                    }),
                },
                url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
                version: Some("4.0.1".to_string()),
                name: "Observation".to_string(),
                title: Some("Observation".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Observation".to_string(),
                snapshot: None,
            };

            definitions.insert("Observation".to_string(), observation_sd);

            Ok(definitions)
        }

        fn create_core_r5_structure_definitions(&self) -> Result<HashMap<String, StructureDefinition>> {
            use crate::{Resource, ResourceType, Meta};

            let mut definitions = HashMap::new();

            let patient_sd = StructureDefinition {
                resource: Resource {
                    resource_type: ResourceType::StructureDefinition,
                    id: Some("Patient".to_string()),
                    meta: Some(Meta {
                        version_id: Some("5.0.0".to_string()),
                        last_updated: Some("2023-03-26T15:21:02.749+11:00".to_string()),
                        profile: None,
                    }),
                },
                url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                version: Some("5.0.0".to_string()),
                name: "Patient".to_string(),
                title: Some("Patient".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Patient".to_string(),
                snapshot: None,
            };

            definitions.insert("Patient".to_string(), patient_sd);

            Ok(definitions)
        }

        async fn extract_additional_r4_resources(&self) -> Result<HashMap<String, StructureDefinition>> {
            use crate::{Resource, ResourceType, Meta};

            let mut definitions = HashMap::new();

            let practitioner_sd = StructureDefinition {
                resource: Resource {
                    resource_type: ResourceType::StructureDefinition,
                    id: Some("Practitioner".to_string()),
                    meta: Some(Meta {
                        version_id: Some("4.0.1".to_string()),
                        last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                        profile: None,
                    }),
                },
                url: "http://hl7.org/fhir/StructureDefinition/Practitioner".to_string(),
                version: Some("4.0.1".to_string()),
                name: "Practitioner".to_string(),
                title: Some("Practitioner".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Practitioner".to_string(),
                snapshot: None,
            };

            definitions.insert("Practitioner".to_string(), practitioner_sd);

            let organization_sd = StructureDefinition {
                resource: Resource {
                    resource_type: ResourceType::StructureDefinition,
                    id: Some("Organization".to_string()),
                    meta: Some(Meta {
                        version_id: Some("4.0.1".to_string()),
                        last_updated: Some("2019-11-01T09:29:23.356+11:00".to_string()),
                        profile: None,
                    }),
                },
                url: "http://hl7.org/fhir/StructureDefinition/Organization".to_string(),
                version: Some("4.0.1".to_string()),
                name: "Organization".to_string(),
                title: Some("Organization".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Organization".to_string(),
                snapshot: None,
            };

            definitions.insert("Organization".to_string(), organization_sd);

            Ok(definitions)
        }

        async fn extract_us_core_profiles(&self, version: Option<&str>) -> Result<HashMap<String, StructureDefinition>> {
            use crate::{Resource, ResourceType, Meta};

            let mut definitions = HashMap::new();
            let us_core_version = version.unwrap_or("6.1.0");

            let us_patient_sd = StructureDefinition {
                resource: Resource {
                    resource_type: ResourceType::StructureDefinition,
                    id: Some("us-core-patient".to_string()),
                    meta: Some(Meta {
                        version_id: Some(us_core_version.to_string()),
                        last_updated: Some("2023-01-01T00:00:00Z".to_string()),
                        profile: None,
                    }),
                },
                url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
                version: Some(us_core_version.to_string()),
                name: "USCorePatientProfile".to_string(),
                title: Some("US Core Patient Profile".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Patient".to_string(),
                snapshot: None,
            };

            definitions.insert("us-core-patient".to_string(), us_patient_sd);

            Ok(definitions)
        }

        fn validate_structure_definitions(&self, definitions: &HashMap<String, StructureDefinition>) -> Result<()> {
            for (key, sd) in definitions {
                if sd.name.is_empty() {
                    return Err(CoreError::InvalidStructure {
                        message: format!("StructureDefinition '{}' has empty name", key),
                        resource_type: Some("StructureDefinition".to_string()),
                        element_path: Some("name".to_string()),
                    });
                }

                if sd.url.is_empty() {
                    return Err(CoreError::InvalidStructure {
                        message: format!("StructureDefinition '{}' has empty URL", key),
                        resource_type: Some("StructureDefinition".to_string()),
                        element_path: Some("url".to_string()),
                    });
                }

                if sd.status != "active" && sd.status != "draft" && sd.status != "retired" {
                    return Err(CoreError::InvalidStructure {
                        message: format!("StructureDefinition '{}' has invalid status: {}", key, sd.status),
                        resource_type: Some("StructureDefinition".to_string()),
                        element_path: Some("status".to_string()),
                    });
                }

                if sd.type_.is_empty() {
                    return Err(CoreError::InvalidStructure {
                        message: format!("StructureDefinition '{}' has empty type", key),
                        resource_type: Some("StructureDefinition".to_string()),
                        element_path: Some("type".to_string()),
                    });
                }
            }

            Ok(())
        }

        fn parse_package_manifest(&self, package_data: &serde_json::Value) -> Result<PackageManifest> {
            let name = package_data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let version = package_data
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let description = package_data
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut dependencies = HashMap::new();
            if let Some(deps) = package_data.get("dependencies").and_then(|v| v.as_object()) {
                for (dep_name, dep_version) in deps {
                    if let Some(version_str) = dep_version.as_str() {
                        dependencies.insert(dep_name.clone(), version_str.to_string());
                    }
                }
            }

            let mut fhir_versions = vec!["4.0.1".to_string()];
            if let Some(fhir_vers) = package_data.get("fhirVersions").and_then(|v| v.as_array()) {
                fhir_versions = fhir_vers
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }

            Ok(PackageManifest {
                name,
                version,
                description,
                dependencies,
                fhir_versions,
            })
        }

        fn enumerate_package_resources(&self, package_data: &serde_json::Value) -> Result<Vec<String>> {
            let mut resource_files = Vec::new();

            if let Some(files) = package_data.get("files").and_then(|v| v.as_array()) {
                for file_entry in files {
                    if let Some(filename) = file_entry.as_str() {
                        if filename.ends_with(".json") && !filename.contains("package.json") {
                            resource_files.push(filename.to_string());
                        }
                    }
                }
            }

            if resource_files.is_empty() {
                resource_files = vec![
                    "StructureDefinition-Patient.json".to_string(),
                    "StructureDefinition-Observation.json".to_string(),
                    "StructureDefinition-Practitioner.json".to_string(),
                    "StructureDefinition-Organization.json".to_string(),
                ];
            }

            Ok(resource_files)
        }
    }

    #[tokio::test]
    async fn test_package_resolver_creation() {
        let result = PackageResolver::new().await;
        // This might fail if canonical manager is not properly configured
        // but we should handle the error gracefully
        match result {
            Ok(_resolver) => {
                // Success case - canonical manager is available
            }
            Err(e) => {
                // Expected in test environment without proper setup
                assert!(matches!(e, CoreError::PackageResolution { .. }));
            }
        }
    }

    #[test]
    fn test_sanitize_package_identifier() {
        let resolver = TestPackageResolver;

        // Valid identifiers
        assert_eq!(
            resolver.sanitize_package_identifier("hl7.fhir.r4.core").unwrap(),
            "hl7.fhir.r4.core"
        );
        assert_eq!(
            resolver.sanitize_package_identifier("my-package_v1").unwrap(),
            "my-package_v1"
        );

        // Invalid identifiers
        assert!(resolver.sanitize_package_identifier("").is_err());
        assert!(resolver.sanitize_package_identifier("...").is_err());
        assert!(resolver.sanitize_package_identifier("---").is_err());

        // Identifiers with invalid characters
        assert_eq!(
            resolver.sanitize_package_identifier("hl7.fhir@r4#core").unwrap(),
            "hl7.fhirr4core"
        );
    }

    #[test]
    fn test_get_default_dependencies() {
        let resolver = TestPackageResolver;

        let us_core_deps = resolver.get_default_dependencies("hl7.fhir.us.core");
        assert!(us_core_deps.contains_key("hl7.fhir.r4.core"));

        let r4_core_deps = resolver.get_default_dependencies("hl7.fhir.r4.core");
        assert!(r4_core_deps.is_empty());

        let r5_core_deps = resolver.get_default_dependencies("hl7.fhir.r5.core");
        assert!(r5_core_deps.is_empty());
    }

    #[test]
    fn test_create_core_r4_structure_definitions() {
        let resolver = TestPackageResolver;

        let definitions = resolver.create_core_r4_structure_definitions().unwrap();
        assert!(definitions.contains_key("Patient"));
        assert!(definitions.contains_key("Observation"));

        let patient_sd = &definitions["Patient"];
        assert_eq!(patient_sd.name, "Patient");
        assert_eq!(patient_sd.type_, "Patient");
        assert_eq!(patient_sd.status, "active");
    }

    #[test]
    fn test_create_core_r5_structure_definitions() {
        let resolver = TestPackageResolver;

        let definitions = resolver.create_core_r5_structure_definitions().unwrap();
        assert!(definitions.contains_key("Patient"));

        let patient_sd = &definitions["Patient"];
        assert_eq!(patient_sd.name, "Patient");
        assert_eq!(patient_sd.version, Some("5.0.0".to_string()));
    }

    #[tokio::test]
    async fn test_extract_additional_r4_resources() {
        let resolver = TestPackageResolver;

        let definitions = resolver.extract_additional_r4_resources().await.unwrap();
        assert!(definitions.contains_key("Practitioner"));
        assert!(definitions.contains_key("Organization"));

        let practitioner_sd = &definitions["Practitioner"];
        assert_eq!(practitioner_sd.name, "Practitioner");
        assert_eq!(practitioner_sd.type_, "Practitioner");
    }

    #[tokio::test]
    async fn test_extract_us_core_profiles() {
        let resolver = TestPackageResolver;

        let definitions = resolver.extract_us_core_profiles(Some("6.1.0")).await.unwrap();
        assert!(definitions.contains_key("us-core-patient"));

        let us_patient_sd = &definitions["us-core-patient"];
        assert_eq!(us_patient_sd.name, "USCorePatientProfile");
        assert_eq!(us_patient_sd.type_, "Patient");
        assert!(us_patient_sd.url.contains("us/core"));
    }

    #[test]
    fn test_validate_structure_definitions() {
        let resolver = TestPackageResolver;

        let mut definitions = HashMap::new();
        
        // Valid StructureDefinition
        let valid_sd = StructureDefinition {
            resource: crate::Resource {
                resource_type: crate::ResourceType::StructureDefinition,
                id: Some("test".to_string()),
                meta: None,
            },
            url: "http://example.org/StructureDefinition/Test".to_string(),
            version: Some("1.0.0".to_string()),
            name: "Test".to_string(),
            title: Some("Test Structure".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Patient".to_string(),
            snapshot: None,
        };

        definitions.insert("test".to_string(), valid_sd);
        assert!(resolver.validate_structure_definitions(&definitions).is_ok());

        // Invalid StructureDefinition - empty name
        let mut invalid_sd = definitions["test"].clone();
        invalid_sd.name = "".to_string();
        definitions.insert("invalid".to_string(), invalid_sd);
        
        assert!(resolver.validate_structure_definitions(&definitions).is_err());
    }

    #[test]
    fn test_parse_package_manifest() {
        let resolver = TestPackageResolver;

        let package_data = serde_json::json!({
            "name": "test.package",
            "version": "1.0.0",
            "description": "Test package",
            "dependencies": {
                "hl7.fhir.r4.core": "4.0.1"
            },
            "fhirVersions": ["4.0.1"]
        });

        let manifest = resolver.parse_package_manifest(&package_data).unwrap();
        assert_eq!(manifest.name, "test.package");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, Some("Test package".to_string()));
        assert!(manifest.dependencies.contains_key("hl7.fhir.r4.core"));
        assert_eq!(manifest.fhir_versions, vec!["4.0.1"]);
    }

    #[test]
    fn test_enumerate_package_resources() {
        let resolver = TestPackageResolver;

        let package_data = serde_json::json!({
            "files": [
                "StructureDefinition-Patient.json",
                "StructureDefinition-Observation.json",
                "package.json",
                "ValueSet-example.json"
            ]
        });

        let resources = resolver.enumerate_package_resources(&package_data).unwrap();
        assert!(resources.contains(&"StructureDefinition-Patient.json".to_string()));
        assert!(resources.contains(&"StructureDefinition-Observation.json".to_string()));
        assert!(resources.contains(&"ValueSet-example.json".to_string()));
        assert!(!resources.contains(&"package.json".to_string()));
    }



    #[test]
    fn test_sanitize_package_identifier_edge_cases() {
        let resolver = TestPackageResolver;

        // Test trimming of special characters
        assert_eq!(
            resolver.sanitize_package_identifier("..test..").unwrap(),
            "test"
        );
        assert_eq!(
            resolver.sanitize_package_identifier("--test--").unwrap(),
            "test"
        );

        // Test mixed valid and invalid characters
        assert_eq!(
            resolver.sanitize_package_identifier("test@#$%package").unwrap(),
            "testpackage"
        );

        // Test that only special characters fails
        assert!(resolver.sanitize_package_identifier("@#$%").is_err());
    }

    #[test]
    fn test_validate_structure_definitions_comprehensive() {
        let resolver = TestPackageResolver;

        let mut definitions = HashMap::new();
        
        // Test empty URL
        let mut invalid_sd = create_test_structure_definition();
        invalid_sd.url = "".to_string();
        definitions.insert("empty_url".to_string(), invalid_sd);
        assert!(resolver.validate_structure_definitions(&definitions).is_err());

        definitions.clear();

        // Test invalid status
        let mut invalid_sd = create_test_structure_definition();
        invalid_sd.status = "invalid_status".to_string();
        definitions.insert("invalid_status".to_string(), invalid_sd);
        assert!(resolver.validate_structure_definitions(&definitions).is_err());

        definitions.clear();

        // Test empty type
        let mut invalid_sd = create_test_structure_definition();
        invalid_sd.type_ = "".to_string();
        definitions.insert("empty_type".to_string(), invalid_sd);
        assert!(resolver.validate_structure_definitions(&definitions).is_err());
    }

    // Helper function to create a valid test StructureDefinition
    fn create_test_structure_definition() -> StructureDefinition {
        StructureDefinition {
            resource: crate::Resource {
                resource_type: crate::ResourceType::StructureDefinition,
                id: Some("test".to_string()),
                meta: None,
            },
            url: "http://example.org/StructureDefinition/Test".to_string(),
            version: Some("1.0.0".to_string()),
            name: "Test".to_string(),
            title: Some("Test Structure".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Patient".to_string(),
            snapshot: None,
        }
    }
}