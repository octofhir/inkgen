use crate::error::{CliError, CliResult};
use crate::logging::ProgressReporter;
use inkgen_core::package::{PackageResolver, Package};
use tracing::{info, warn, debug, instrument};

/// Execute the fetch command to download and cache FHIR packages
#[instrument(skip_all, fields(package = %package, version = ?version, force = %force))]
pub async fn execute_fetch(
    package: String,
    version: Option<String>,
    force: bool,
) -> CliResult<()> {
    info!("Starting package fetch operation");
    
    // Normalize package name (will be implemented in task 3.2)
    let normalized_package = normalize_package_name(&package)?;
    debug!("Normalized package name: {} -> {}", package, normalized_package);
    
    if force {
        warn!("Force flag specified - will re-download even if cached");
    }
    
    // Create progress bar for user feedback
    let progress = ProgressReporter::new_spinner("Initializing package resolver...");
    
    // Initialize package resolver
    let resolver = match PackageResolver::new().await {
        Ok(resolver) => {
            progress.set_message("Package resolver initialized");
            resolver
        }
        Err(e) => {
            progress.abandon_with_message("Failed to initialize package resolver");
            return Err(CliError::PackageFetch {
                package: normalized_package,
                source: Box::new(e),
            });
        }
    };
    
    progress.set_message("Resolving package...");
    
    // Resolve version specification
    let resolved_version = resolve_version_specification(version.as_deref())?;
    
    // Resolve the package
    let resolved_package = match resolver.resolve_package(&normalized_package, resolved_version.as_deref()).await {
        Ok(package) => {
            progress.finish_with_message("Package resolved successfully");
            package
        }
        Err(e) => {
            progress.abandon_with_message("Package resolution failed");
            return Err(CliError::PackageFetch {
                package: normalized_package,
                source: Box::new(e),
            });
        }
    };
    
    // Display success information
    display_package_info(&resolved_package);
    
    info!(
        "Successfully fetched {} v{} ({} resources)",
        resolved_package.name,
        resolved_package.version,
        resolved_package.resources.len()
    );
    
    Ok(())
}

/// Normalize package names to support shortened forms
/// Supports automatic expansion of shortened package names:
/// - "r4.core" -> "hl7.fhir.r4.core"
/// - "us.core" -> "hl7.fhir.us.core"
/// - Full names are used as-is
fn normalize_package_name(package: &str) -> CliResult<String> {
    // Validate input
    if package.is_empty() {
        return Err(CliError::invalid_config("Package name cannot be empty"));
    }
    
    // Trim whitespace
    let package = package.trim();
    
    // Validate package identifier format
    validate_package_identifier(package)?;
    
    // If already a full package name, return as-is
    if package.starts_with("hl7.fhir.") {
        return Ok(package.to_string());
    }
    
    // Handle shortened package names
    let normalized = expand_shortened_package_name(package)?;
    
    debug!("Package name normalization: {} -> {}", package, normalized);
    Ok(normalized)
}

/// Validate package identifier format
fn validate_package_identifier(package: &str) -> CliResult<()> {
    // Check for invalid characters
    let valid_chars = package.chars().all(|c| {
        c.is_alphanumeric() || c == '.' || c == '-' || c == '_'
    });
    
    if !valid_chars {
        return Err(CliError::invalid_config(format!(
            "Invalid package identifier '{}'. Only alphanumeric characters, dots, hyphens, and underscores are allowed.",
            package
        )));
    }
    
    // Check for empty segments
    if package.contains("..") {
        return Err(CliError::invalid_config(format!(
            "Invalid package identifier '{}'. Empty segments (consecutive dots) are not allowed.",
            package
        )));
    }
    
    // Check for leading/trailing dots
    if package.starts_with('.') || package.ends_with('.') {
        return Err(CliError::invalid_config(format!(
            "Invalid package identifier '{}'. Cannot start or end with a dot.",
            package
        )));
    }
    
    // Minimum length check
    if package.len() < 2 {
        return Err(CliError::invalid_config(format!(
            "Invalid package identifier '{}'. Must be at least 2 characters long.",
            package
        )));
    }
    
    Ok(())
}

/// Expand shortened package names to full HL7 FHIR package names
fn expand_shortened_package_name(package: &str) -> CliResult<String> {
    // Common FHIR package expansions
    let expanded = match package {
        // Core FHIR packages
        "r4.core" => "hl7.fhir.r4.core",
        "r5.core" => "hl7.fhir.r5.core",
        "r6.core" => "hl7.fhir.r6.core",
        
        // US Core packages
        "us.core" => "hl7.fhir.us.core",
        
        // Other common packages
        "uv.ips" => "hl7.fhir.uv.ips",
        "uv.sdc" => "hl7.fhir.uv.sdc",
        "uv.cpg" => "hl7.fhir.uv.cpg",
        
        // If it contains dots but doesn't start with hl7.fhir, assume it's a partial name
        _ if package.contains('.') => {
            // Check if it looks like a partial HL7 FHIR package name
            if is_likely_fhir_package(package) {
                return Ok(format!("hl7.fhir.{}", package));
            } else {
                // Return as-is for non-FHIR packages or already full names
                return Ok(package.to_string());
            }
        }
        
        // Single word packages - could be ambiguous
        _ => {
            return Err(CliError::invalid_config(format!(
                "Ambiguous package name '{}'. Please use a more specific name like 'r4.core' or the full name 'hl7.fhir.r4.core'.",
                package
            )));
        }
    };
    
    Ok(expanded.to_string())
}

/// Check if a package name looks like a FHIR package
fn is_likely_fhir_package(package: &str) -> bool {
    let parts: Vec<&str> = package.split('.').collect();
    
    // Must have at least 2 parts
    if parts.len() < 2 {
        return false;
    }
    
    // Check for common FHIR patterns
    let first_part = parts[0];
    let second_part = parts[1];
    
    // Common FHIR version patterns
    if matches!(first_part, "r4" | "r5" | "r6") {
        return true;
    }
    
    // Common realm patterns
    if matches!(first_part, "us" | "au" | "ca" | "uk" | "de" | "fr" | "nl" | "uv") {
        return true;
    }
    
    // Common second-level patterns
    if matches!(second_part, "core" | "base" | "extensions" | "terminology") {
        return true;
    }
    
    false
}

/// Resolve version specification, handling "latest" and validation
fn resolve_version_specification(version: Option<&str>) -> CliResult<Option<String>> {
    match version {
        None => Ok(None), // Use latest
        Some("latest") => Ok(None), // Explicitly request latest
        Some(v) => {
            // Validate version format (basic semver-like validation)
            if validate_version_format(v) {
                Ok(Some(v.to_string()))
            } else {
                Err(CliError::invalid_config(format!(
                    "Invalid version format '{}'. Expected format: x.y.z or x.y.z-suffix",
                    v
                )))
            }
        }
    }
}

/// Validate version format (basic semver validation)
fn validate_version_format(version: &str) -> bool {
    // Allow basic semver patterns: x.y.z, x.y.z-suffix, x.y, x
    let version_regex = regex::Regex::new(r"^\d+(\.\d+)?(\.\d+)?(-[a-zA-Z0-9\-\.]+)?$");
    
    match version_regex {
        Ok(regex) => regex.is_match(version),
        Err(_) => {
            // Fallback to basic validation if regex fails
            !version.is_empty() && version.chars().any(|c| c.is_ascii_digit())
        }
    }
}



/// Display information about the fetched package
fn display_package_info(package: &Package) {
    println!("\n✅ Package fetched successfully!");
    println!("📦 Name: {}", package.name);
    println!("🏷️  Version: {}", package.version);
    
    if let Some(description) = &package.manifest.description {
        println!("📝 Description: {}", description);
    }
    
    println!("🔢 Resources: {}", package.resources.len());
    
    if !package.manifest.fhir_versions.is_empty() {
        println!("🩺 FHIR Versions: {}", package.manifest.fhir_versions.join(", "));
    }
    
    if !package.manifest.dependencies.is_empty() {
        println!("📋 Dependencies:");
        for (dep_name, dep_version) in &package.manifest.dependencies {
            println!("  • {} ({})", dep_name, dep_version);
        }
    }
    
    println!("💾 Cached for future use");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_package_name_empty() {
        assert!(normalize_package_name("").is_err());
        assert!(normalize_package_name("   ").is_err());
    }

    #[test]
    fn test_normalize_package_name_full_names() {
        // Full package names should be returned as-is
        assert_eq!(
            normalize_package_name("hl7.fhir.r4.core").unwrap(),
            "hl7.fhir.r4.core"
        );
        
        assert_eq!(
            normalize_package_name("hl7.fhir.us.core").unwrap(),
            "hl7.fhir.us.core"
        );
    }

    #[test]
    fn test_normalize_package_name_shortened() {
        // Test common shortened names
        assert_eq!(
            normalize_package_name("r4.core").unwrap(),
            "hl7.fhir.r4.core"
        );
        
        assert_eq!(
            normalize_package_name("r5.core").unwrap(),
            "hl7.fhir.r5.core"
        );
        
        assert_eq!(
            normalize_package_name("us.core").unwrap(),
            "hl7.fhir.us.core"
        );
        
        assert_eq!(
            normalize_package_name("uv.ips").unwrap(),
            "hl7.fhir.uv.ips"
        );
    }

    #[test]
    fn test_normalize_package_name_partial() {
        // Test partial names that should be expanded
        assert_eq!(
            normalize_package_name("au.base").unwrap(),
            "hl7.fhir.au.base"
        );
        
        assert_eq!(
            normalize_package_name("ca.baseline").unwrap(),
            "hl7.fhir.ca.baseline"
        );
    }

    #[test]
    fn test_normalize_package_name_ambiguous() {
        // Single word packages should be rejected as ambiguous
        assert!(normalize_package_name("core").is_err());
        assert!(normalize_package_name("base").is_err());
        assert!(normalize_package_name("r4").is_err());
    }

    #[test]
    fn test_validate_package_identifier() {
        // Valid identifiers
        assert!(validate_package_identifier("hl7.fhir.r4.core").is_ok());
        assert!(validate_package_identifier("my-package_v1").is_ok());
        assert!(validate_package_identifier("test.package").is_ok());

        // Invalid characters
        assert!(validate_package_identifier("test@package").is_err());
        assert!(validate_package_identifier("test#package").is_err());
        assert!(validate_package_identifier("test package").is_err());

        // Empty segments
        assert!(validate_package_identifier("test..package").is_err());
        assert!(validate_package_identifier("test...package").is_err());

        // Leading/trailing dots
        assert!(validate_package_identifier(".test.package").is_err());
        assert!(validate_package_identifier("test.package.").is_err());

        // Too short
        assert!(validate_package_identifier("a").is_err());
        assert!(validate_package_identifier("").is_err());
    }

    #[test]
    fn test_expand_shortened_package_name() {
        // Known expansions
        assert_eq!(
            expand_shortened_package_name("r4.core").unwrap(),
            "hl7.fhir.r4.core"
        );
        
        assert_eq!(
            expand_shortened_package_name("us.core").unwrap(),
            "hl7.fhir.us.core"
        );

        // Ambiguous single words should fail
        assert!(expand_shortened_package_name("core").is_err());
        assert!(expand_shortened_package_name("base").is_err());
    }

    #[test]
    fn test_is_likely_fhir_package() {
        // FHIR version patterns
        assert!(is_likely_fhir_package("r4.core"));
        assert!(is_likely_fhir_package("r5.extensions"));
        assert!(is_likely_fhir_package("r6.terminology"));

        // Realm patterns
        assert!(is_likely_fhir_package("us.core"));
        assert!(is_likely_fhir_package("au.base"));
        assert!(is_likely_fhir_package("uv.ips"));

        // Common second-level patterns
        assert!(is_likely_fhir_package("something.core"));
        assert!(is_likely_fhir_package("something.base"));

        // Non-FHIR patterns
        assert!(!is_likely_fhir_package("random.package"));
        assert!(!is_likely_fhir_package("single"));
        assert!(!is_likely_fhir_package(""));
    }

    #[test]
    fn test_resolve_version_specification() {
        // None should return None (latest)
        assert_eq!(resolve_version_specification(None).unwrap(), None);
        
        // "latest" should return None
        assert_eq!(resolve_version_specification(Some("latest")).unwrap(), None);
        
        // Valid versions
        assert_eq!(
            resolve_version_specification(Some("4.0.1")).unwrap(),
            Some("4.0.1".to_string())
        );
        
        assert_eq!(
            resolve_version_specification(Some("1.2.3-beta")).unwrap(),
            Some("1.2.3-beta".to_string())
        );
        
        // Invalid versions should fail
        assert!(resolve_version_specification(Some("invalid")).is_err());
        assert!(resolve_version_specification(Some("")).is_err());
    }

    #[test]
    fn test_validate_version_format() {
        // Valid versions
        assert!(validate_version_format("1.0.0"));
        assert!(validate_version_format("4.0.1"));
        assert!(validate_version_format("1.2.3-beta"));
        assert!(validate_version_format("2.1"));
        assert!(validate_version_format("5"));
        assert!(validate_version_format("1.0.0-rc.1"));

        // Invalid versions
        assert!(!validate_version_format(""));
        assert!(!validate_version_format("abc"));
        assert!(!validate_version_format("1.2.3.4.5"));
        assert!(!validate_version_format("v1.0.0")); // No 'v' prefix
    }
    

}