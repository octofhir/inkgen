//! Profile helper function generation.
//!
//! This module handles:
//! - Generating attach/extract/validate helper functions for profiles
//! - Creating type-safe profile attachment and extraction patterns
//! - Generating profile type guard functions
//! - Profile constraint validation function metadata

use inkgen_core::ir::ResourceDefinition;

/// Metadata for a profile attachment helper function.
///
/// Example: `attachUSCorePatientTo(patient, usCore)`
#[derive(Debug, Clone)]
pub struct ProfileAttachFunction {
    /// Function name (e.g., "attachUSCorePatientTo")
    pub function_name: String,
    /// Profile name in PascalCase (e.g., "USCorePatient")
    pub profile_name: String,
    /// Profile canonical URL
    pub profile_url: String,
    /// Base resource type (e.g., "Patient")
    pub resource_type: String,
    /// Parameter name for the profile data (e.g., "usCore")
    pub parameter_name: String,
    /// Profile description
    pub description: Option<String>,
}

/// Metadata for a profile extraction helper function.
///
/// Example: `extractUSCorePatientFrom(patient)`
#[derive(Debug, Clone)]
pub struct ProfileExtractFunction {
    /// Function name (e.g., "extractUSCorePatientFrom")
    pub function_name: String,
    /// Profile name in PascalCase (e.g., "USCorePatient")
    pub profile_name: String,
    /// Profile canonical URL
    pub profile_url: String,
    /// Base resource type (e.g., "Patient")
    pub resource_type: String,
    /// Return type name (often same as profile_name)
    pub return_type: String,
    /// Profile description
    pub description: Option<String>,
}

/// Metadata for a profile type guard function.
///
/// Example: `isUSCorePatient(resource)`
#[derive(Debug, Clone)]
pub struct ProfileGuardFunction {
    /// Function name (e.g., "isUSCorePatient")
    pub function_name: String,
    /// Profile name in PascalCase
    pub profile_name: String,
    /// Profile canonical URL
    pub profile_url: String,
    /// Input parameter type (e.g., "Patient")
    pub input_type: String,
    /// Narrowed return type (e.g., "Patient & { meta?: {profile?: string[]} }")
    pub narrowed_type: String,
    /// Profile description
    pub description: Option<String>,
}

/// Metadata for a profile validation helper function.
#[derive(Debug, Clone)]
pub struct ProfileValidationFunction {
    /// Function name (e.g., "validateUSCorePatient")
    pub function_name: String,
    /// Profile name in PascalCase
    pub profile_name: String,
    /// Profile canonical URL
    pub profile_url: String,
    /// Parameter type to validate (e.g., "Patient")
    pub parameter_type: String,
    /// Returns array of validation errors
    pub return_type: String,
    /// Constraints to validate
    pub constraints: Vec<ProfileConstraint>,
    /// Profile description
    pub description: Option<String>,
}

/// A single constraint to validate in a profile.
#[derive(Debug, Clone)]
pub struct ProfileConstraint {
    /// Constraint key (e.g., "must-support")
    pub key: String,
    /// Constraint description
    pub description: Option<String>,
    /// Element path being constrained (e.g., "Patient.name")
    pub element_path: String,
    /// Whether this is a required constraint
    pub is_required: bool,
    /// Whether this is a must-support constraint
    pub is_must_support: bool,
}

/// Complete collection of helper functions for a profile.
#[derive(Debug, Clone)]
pub struct ProfileHelpers {
    /// Attach function metadata
    pub attach_function: ProfileAttachFunction,
    /// Extract function metadata
    pub extract_function: ProfileExtractFunction,
    /// Type guard function metadata
    pub guard_function: ProfileGuardFunction,
    /// Validation function metadata
    pub validation_function: ProfileValidationFunction,
}

/// Generate profile helper functions for a profile.
///
/// The profile should be a ResourceDefinition with lineage.derivation == Constraint.
/// The base resource type is derived from lineage.base_definition.
pub fn generate_profile_helpers(profile: &ResourceDefinition) -> Option<ProfileHelpers> {
    // Extract base resource type from lineage
    let base_definition = profile.lineage.base_definition.as_ref()?;
    let base_resource_type = base_definition
        .split('/')
        .next_back()
        .unwrap_or("Resource")
        .to_string();

    let profile_name = profile_url_to_name(&profile.url);

    // Generate constraint list from profile elements
    let constraints = collect_constraints(profile);

    let attach_function = ProfileAttachFunction {
        function_name: format!("attach{}To", profile_name),
        profile_name: profile_name.clone(),
        profile_url: profile.url.clone(),
        resource_type: base_resource_type.clone(),
        parameter_name: to_camel_case(&profile_name),
        description: profile.description.clone(),
    };

    let extract_function = ProfileExtractFunction {
        function_name: format!("extract{}From", profile_name),
        profile_name: profile_name.clone(),
        profile_url: profile.url.clone(),
        resource_type: base_resource_type.clone(),
        return_type: profile_name.clone(),
        description: profile.description.clone(),
    };

    let guard_function = ProfileGuardFunction {
        function_name: format!("is{}", profile_name),
        profile_name: profile_name.clone(),
        profile_url: profile.url.clone(),
        input_type: base_resource_type.clone(),
        narrowed_type: format!(
            "{} & {{ meta?: {{ profile?: string[] }} }}",
            base_resource_type
        ),
        description: profile.description.clone(),
    };

    let validation_function = ProfileValidationFunction {
        function_name: format!("validate{}", profile_name),
        profile_name: profile_name.clone(),
        profile_url: profile.url.clone(),
        parameter_type: base_resource_type.clone(),
        return_type: "ProfileValidationError[]".to_string(),
        constraints,
        description: profile.description.clone(),
    };

    Some(ProfileHelpers {
        attach_function,
        extract_function,
        guard_function,
        validation_function,
    })
}

/// Collect constraint information from profile elements.
fn collect_constraints(profile: &ResourceDefinition) -> Vec<ProfileConstraint> {
    let mut constraints = Vec::new();

    for element in &profile.elements {
        // Must-support constraints
        if element.must_support {
            constraints.push(ProfileConstraint {
                key: format!("must-support-{}", element.path),
                description: Some(format!("Element {} is must-support", element.path)),
                element_path: element.path.clone(),
                is_required: false,
                is_must_support: true,
            });
        }

        // Cardinality constraints (required elements)
        if element.cardinality.min > 0 {
            constraints.push(ProfileConstraint {
                key: format!("cardinality-{}", element.path),
                description: Some(format!(
                    "Element {} has minimum cardinality {}",
                    element.path, element.cardinality.min
                )),
                element_path: element.path.clone(),
                is_required: true,
                is_must_support: false,
            });
        }
    }

    constraints
}

/// Convert a profile canonical URL to a profile name in PascalCase.
///
/// Examples:
/// - `http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient`
///   -> `USCorePatient`
/// - `http://example.org/fhir/profiles/custom-profile`
///   -> `CustomProfile`
pub fn profile_url_to_name(url: &str) -> String {
    // Extract the last component (after the last /)
    let last_component = url.split('/').next_back().unwrap_or("Profile");

    // Convert kebab-case or snake_case to PascalCase
    let pascal_parts: Vec<String> = last_component
        .split(['-', '_'])
        .map(|word| {
            if word.is_empty() {
                String::new()
            } else if is_known_acronym(word) {
                // Known acronyms (like "us") should be ALL_CAPS
                word.to_uppercase()
            } else {
                // Regular words: capitalize first letter, keep rest as-is
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        })
        .collect();

    pascal_parts.join("")
}

/// Check if a word is a known acronym that should be all uppercase.
fn is_known_acronym(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "us" | "ips" | "hl7" | "fhir" | "cda" | "ccda" | "uu" | "uuid" | "id"
    )
}

/// Convert PascalCase to camelCase.
///
/// Examples:
/// - `USCorePatient` -> `uSCorePatient`
/// - `Patient` -> `patient`
fn to_camel_case(pascal_case: &str) -> String {
    if pascal_case.is_empty() {
        return String::new();
    }

    let mut chars = pascal_case.chars();
    let first = chars.next().unwrap();
    first.to_lowercase().collect::<String>() + chars.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_url_to_name_us_core() {
        assert_eq!(
            profile_url_to_name("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"),
            "USCorePatient"
        );
    }

    #[test]
    fn test_profile_url_to_name_simple() {
        assert_eq!(
            profile_url_to_name("http://example.org/fhir/profiles/custom-profile"),
            "CustomProfile"
        );
    }

    #[test]
    fn test_profile_url_to_name_snake_case() {
        assert_eq!(
            profile_url_to_name("http://example.org/fhir/profiles/my_custom_profile"),
            "MyCustomProfile"
        );
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("USCorePatient"), "uSCorePatient");
        assert_eq!(to_camel_case("Patient"), "patient");
        assert_eq!(to_camel_case("ProfileHelper"), "profileHelper");
    }

    #[test]
    fn test_attach_function_name_generation() {
        let profile_name =
            profile_url_to_name("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient");
        let function_name = format!("attach{}To", profile_name);
        assert_eq!(function_name, "attachUSCorePatientTo");
    }

    #[test]
    fn test_extract_function_name_generation() {
        let profile_name =
            profile_url_to_name("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient");
        let function_name = format!("extract{}From", profile_name);
        assert_eq!(function_name, "extractUSCorePatientFrom");
    }

    #[test]
    fn test_guard_function_name_generation() {
        let profile_name =
            profile_url_to_name("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient");
        let function_name = format!("is{}", profile_name);
        assert_eq!(function_name, "isUSCorePatient");
    }

    #[test]
    fn test_validation_function_name_generation() {
        let profile_name =
            profile_url_to_name("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient");
        let function_name = format!("validate{}", profile_name);
        assert_eq!(function_name, "validateUSCorePatient");
    }

    #[test]
    fn test_parameter_name_generation() {
        let profile_name =
            profile_url_to_name("http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient");
        let parameter_name = to_camel_case(&profile_name);
        assert_eq!(parameter_name, "uSCorePatient");
    }
}
