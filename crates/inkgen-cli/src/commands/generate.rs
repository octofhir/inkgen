use crate::config::schema::{InkgenConfig, TypeScriptConfig};
use crate::error::{CliError, CliResult};
use inkgen_core::{PackageResolver, ProfileService, CoreConfig, FhirResource, StructureDefinition};
use inkgen_typescript::{TypeScriptGenerator, LanguageGenerator};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn, instrument, debug};

/// Execute the TypeScript generation command
#[instrument(skip(config_path, output_dir))]
pub async fn execute_generate_typescript(
    config_path: PathBuf,
    output_dir: PathBuf,
    package_override: Option<String>,
) -> CliResult<()> {
    info!("Starting TypeScript code generation");
    
    // Parse configuration
    let config = if config_path.exists() {
        info!("Loading configuration from: {}", config_path.display());
        InkgenConfig::from_file(&config_path)?
    } else {
        warn!("Configuration file not found at {}, using defaults", config_path.display());
        InkgenConfig::default()
    };
    
    // Initialize core services
    info!("Initializing core services");
    let resolver = Arc::new(PackageResolver::new().await
        .map_err(|e| CliError::CoreError(e.into()))?);
    
    // Create core configuration based on CLI config
    let core_config = create_core_config(&config);
    let profile_service = ProfileService::with_config(resolver.clone(), core_config.profile_resolution);
    
    // Determine packages to process
    let packages = if let Some(pkg) = package_override {
        info!("Using package override: {}", pkg);
        vec![pkg]
    } else {
        let package_names: Vec<String> = config.packages.iter()
            .map(|p| p.normalize_name())
            .collect();
        info!("Processing {} packages from configuration", package_names.len());
        package_names
    };
    
    // Initialize TypeScript generator
    let ts_config = config.languages
        .as_ref()
        .and_then(|l| l.typescript.as_ref())
        .cloned()
        .unwrap_or_default();
    let generator = create_typescript_generator(&ts_config)?;
    
    // Create output directory
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| CliError::IoError {
            operation: format!("creating output directory '{}'", output_dir.display()),
            source: e,
        })?;
    
    // Process each package
    let mut total_files = 0;
    for package_name in packages {
        info!("Processing package: {}", package_name);
        
        // Resolve package
        let package = resolver.resolve_package(&package_name, None).await
            .map_err(|e| CliError::PackageFetch {
                package: package_name.clone(),
                source: Box::new(e),
            })?;
        
        debug!("Package {} contains {} resources", package_name, package.resources.len());
        
        // Convert HashMap to Vec for processing
        let resource_vec: Vec<&StructureDefinition> = package.resources.values().collect();
        
        // Apply tree-shaking if configured
        let filtered_resources = apply_tree_shaking(&resource_vec, &config, &package_name)?;
        info!("After tree-shaking: {} resources selected for generation", filtered_resources.len());
        
        // Generate TypeScript code for each resource
        let generated_files = generate_package_files(&generator, &filtered_resources, &profile_service, &package_name).await?;
        
        // Write generated files
        let file_count = generated_files.len();
        for file in generated_files {
            let output_path = output_dir.join(&file.path);
            
            // Create parent directories if needed
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CliError::IoError {
                        operation: format!("creating directory '{}'", parent.display()),
                        source: e,
                    })?;
            }
            
            // Write file content
            std::fs::write(&output_path, &file.content)
                .map_err(|e| CliError::IoError {
                    operation: format!("writing file '{}'", output_path.display()),
                    source: e,
                })?;
            
            debug!("Generated file: {}", output_path.display());
        }
        
        total_files += file_count;
        info!("Generated {} files for package {}", file_count, package_name);
    }
    
    info!("TypeScript generation completed successfully - {} total files generated", total_files);
    Ok(())
}

/// Create core configuration from CLI configuration
fn create_core_config(config: &InkgenConfig) -> CoreConfig {
    // Check if this looks like a US Core configuration
    let has_us_core = config.packages.iter().any(|p| {
        let normalized = p.normalize_name();
        normalized.contains("us.core") || normalized.contains("us-core")
    });
    
    if has_us_core {
        CoreConfig::us_core()
    } else {
        // Default to R4 core for most cases
        CoreConfig::fhir_r4_core()
    }
}

/// Create TypeScript generator with configuration
fn create_typescript_generator(_config: &TypeScriptConfig) -> CliResult<TypeScriptGenerator> {
    // For now, create a basic generator
    // TODO: Apply TypeScript-specific configuration options
    TypeScriptGenerator::new()
        .map_err(|e| CliError::generation_failed_with_source(
            "Failed to initialize TypeScript generator", e
        ))
}

/// Apply tree-shaking configuration to filter resources
fn apply_tree_shaking<'a>(
    resources: &'a [&'a StructureDefinition],
    config: &InkgenConfig,
    package_name: &str,
) -> CliResult<Vec<&'a StructureDefinition>> {
    let mut filtered = resources.to_vec();
    
    if let Some(tree_shaking) = &config.tree_shaking {
        debug!("Applying tree-shaking for package: {}", package_name);
        
        // Apply resource allowlist
        if let Some(allowed_resources) = &tree_shaking.allowed_resources {
            debug!("Filtering by allowed resources: {:?}", allowed_resources);
            filtered.retain(|resource| {
                is_resource_type_allowed(resource, allowed_resources)
            });
        }
        
        // Apply profile allowlist
        if let Some(allowed_profiles) = &tree_shaking.allowed_profiles {
            debug!("Filtering by allowed profiles: {:?}", allowed_profiles);
            filtered.retain(|resource| {
                is_profile_allowed(resource, allowed_profiles)
            });
        }
    }
    
    // Apply package-specific inclusion/exclusion rules
    if let Some(package_spec) = config.packages.iter().find(|p| p.normalize_name() == package_name) {
        if let Some(include_rules) = &package_spec.include {
            debug!("Applying package include rules: {:?}", include_rules);
            filtered.retain(|resource| {
                matches_include_rules(resource, include_rules)
            });
        }
        
        if let Some(exclude_rules) = &package_spec.exclude {
            debug!("Applying package exclude rules: {:?}", exclude_rules);
            filtered.retain(|resource| {
                !matches_exclude_rules(resource, exclude_rules)
            });
        }
    }
    
    Ok(filtered)
}



/// Check if a resource type is in the allowed resources list
fn is_resource_type_allowed(resource: &StructureDefinition, allowed_resources: &[String]) -> bool {
    let resource_type = &resource.type_;
    
    allowed_resources.iter().any(|allowed| {
        // Support exact matches and pattern matching
        resource_type == allowed || 
        resource_type.to_lowercase().contains(&allowed.to_lowercase()) ||
        allowed.contains('*') && matches_wildcard_pattern(resource_type, allowed)
    })
}

/// Check if a profile is in the allowed profiles list
fn is_profile_allowed(resource: &StructureDefinition, allowed_profiles: &[String]) -> bool {
    // Check against profile URL, name, and ID
    let profile_identifiers = [
        &resource.url,
        &resource.name,
        resource.resource.id.as_deref().unwrap_or(""),
    ];
    
    allowed_profiles.iter().any(|allowed| {
        profile_identifiers.iter().any(|identifier| {
            identifier == allowed ||
            identifier.to_lowercase().contains(&allowed.to_lowercase()) ||
            allowed.contains('*') && matches_wildcard_pattern(identifier, allowed)
        })
    })
}

/// Check if a resource matches package-specific include rules
fn matches_include_rules(resource: &StructureDefinition, include_rules: &[String]) -> bool {
    let searchable_fields = [
        &resource.name,
        &resource.type_,
        &resource.url,
        resource.resource.id.as_deref().unwrap_or(""),
        resource.title.as_deref().unwrap_or(""),
    ];
    
    include_rules.iter().any(|rule| {
        searchable_fields.iter().any(|field| {
            field.to_lowercase().contains(&rule.to_lowercase()) ||
            (rule.contains('*') && matches_wildcard_pattern(field, rule))
        })
    })
}

/// Check if a resource matches package-specific exclude rules
fn matches_exclude_rules(resource: &StructureDefinition, exclude_rules: &[String]) -> bool {
    let searchable_fields = [
        &resource.name,
        &resource.type_,
        &resource.url,
        resource.resource.id.as_deref().unwrap_or(""),
        resource.title.as_deref().unwrap_or(""),
    ];
    
    exclude_rules.iter().any(|rule| {
        searchable_fields.iter().any(|field| {
            field.to_lowercase().contains(&rule.to_lowercase()) ||
            (rule.contains('*') && matches_wildcard_pattern(field, rule))
        })
    })
}

/// Simple wildcard pattern matching (supports * as wildcard)
fn matches_wildcard_pattern(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return text == pattern;
    }
    
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }
    
    let mut text_pos = 0;
    
    // Check if text starts with the first part (if not empty)
    if !parts[0].is_empty() {
        if !text.starts_with(parts[0]) {
            return false;
        }
        text_pos = parts[0].len();
    }
    
    // Check middle parts
    for part in &parts[1..parts.len()-1] {
        if part.is_empty() {
            continue;
        }
        if let Some(pos) = text[text_pos..].find(part) {
            text_pos += pos + part.len();
        } else {
            return false;
        }
    }
    
    // Check if text ends with the last part (if not empty)
    if let Some(last_part) = parts.last() {
        if !last_part.is_empty() {
            return text[text_pos..].ends_with(last_part);
        }
    }
    
    true
}

/// Generated file structure
#[derive(Debug)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

/// Generate TypeScript files for a package
async fn generate_package_files(
    generator: &TypeScriptGenerator,
    resources: &[&StructureDefinition],
    _profile_service: &ProfileService,
    _package_name: &str,
) -> CliResult<Vec<GeneratedFile>> {
    let mut files = Vec::new();
    
    // Generate individual resource files
    for resource in resources {
        let content = generator.generate_profile(resource)
            .map_err(|e| CliError::generation_failed_with_source(
                format!("Failed to generate code for resource {}", resource.resource_type()), e
            ))?;
        
        let filename = format!("{}.ts", resource.name.to_lowercase().replace(' ', "_"));
        files.push(GeneratedFile {
            path: filename,
            content,
        });
    }
    
    // Generate package index file
    let resource_refs: Vec<&dyn FhirResource> = resources.iter().map(|r| *r as &dyn FhirResource).collect();
    let package_content = generator.generate_package(&resource_refs)
        .map_err(|e| CliError::generation_failed_with_source(
            "Failed to generate package index", e
        ))?;
    
    files.push(GeneratedFile {
        path: "index.ts".to_string(),
        content: package_content,
    });
    
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::PackageSpec;

    #[test]
    fn test_create_core_config_default() {
        let config = InkgenConfig::default();
        let core_config = create_core_config(&config);
        // Should use R4 core config based on default package
        assert_eq!(core_config.ir_options.version, "fhir-r4-4.0.1");
    }

    #[test]
    fn test_create_core_config_us_core() {
        let config = InkgenConfig {
            packages: vec![PackageSpec {
                name: "hl7.fhir.us.core".to_string(),
                version: None,
                include: None,
                exclude: None,
            }],
            tree_shaking: None,
            languages: None,
        };
        
        let core_config = create_core_config(&config);
        assert_eq!(core_config.ir_options.version, "us-core-6.1.0");
    }

    #[test]
    fn test_create_typescript_generator() {
        let config = TypeScriptConfig::default();
        let result = create_typescript_generator(&config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_package_files() {
        let generator = TypeScriptGenerator::new().unwrap();
        let resources: Vec<&StructureDefinition> = vec![]; // Empty for test
        let resolver = Arc::new(PackageResolver::new().await.unwrap());
        let profile_service = ProfileService::new(resolver);
        
        let result = generate_package_files(&generator, &resources, &profile_service, "test-package").await;
        assert!(result.is_ok());
        
        let files = result.unwrap();
        // Should at least have an index file
        assert!(files.iter().any(|f| f.path == "index.ts"));
    }



    #[test]
    fn test_is_resource_type_allowed() {
        use inkgen_core::{Resource, ResourceType};
        
        let patient_resource = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("test".to_string()),
                meta: None,
            },
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            version: None,
            name: "Patient".to_string(),
            title: None,
            status: "active".to_string(),
            experimental: None,
            type_: "Patient".to_string(),
            snapshot: None,
        };
        
        let allowed_resources = vec!["Patient".to_string()];
        assert!(is_resource_type_allowed(&patient_resource, &allowed_resources));
        
        let allowed_resources = vec!["Observation".to_string()];
        assert!(!is_resource_type_allowed(&patient_resource, &allowed_resources));
        
        // Test wildcard matching
        let allowed_resources = vec!["Pat*".to_string()];
        assert!(is_resource_type_allowed(&patient_resource, &allowed_resources));
    }

    #[test]
    fn test_matches_wildcard_pattern() {
        assert!(matches_wildcard_pattern("Patient", "Patient"));
        assert!(matches_wildcard_pattern("Patient", "Pat*"));
        assert!(matches_wildcard_pattern("Patient", "*ient"));
        assert!(matches_wildcard_pattern("Patient", "P*t"));
        assert!(matches_wildcard_pattern("Patient", "*"));
        
        assert!(!matches_wildcard_pattern("Patient", "Observation"));
        assert!(!matches_wildcard_pattern("Patient", "Obs*"));
        assert!(!matches_wildcard_pattern("Patient", "*Obs"));
    }

    #[test]
    fn test_apply_tree_shaking_allowed_resources() {
        use inkgen_core::{Resource, ResourceType};
        
        let patient_resource = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("test".to_string()),
                meta: None,
            },
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            version: None,
            name: "Patient".to_string(),
            title: None,
            status: "active".to_string(),
            experimental: None,
            type_: "Patient".to_string(),
            snapshot: None,
        };
        
        let observation_resource = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("test".to_string()),
                meta: None,
            },
            url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
            version: None,
            name: "Observation".to_string(),
            title: None,
            status: "active".to_string(),
            experimental: None,
            type_: "Observation".to_string(),
            snapshot: None,
        };
        
        let resources = vec![&patient_resource, &observation_resource];
        
        let config = InkgenConfig {
            packages: vec![],
            tree_shaking: Some(crate::config::schema::TreeShakingConfig {
                allowed_resources: Some(vec!["Patient".to_string()]),
                allowed_profiles: None,
            }),
            languages: None,
        };
        
        let result = apply_tree_shaking(&resources, &config, "test-package").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Patient");
    }
}