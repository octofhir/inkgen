/// Core error types for FHIR processing with comprehensive error context
#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("Failed to parse FHIR resource: {message}")]
    ParseError { 
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Package not found: {package}")]
    PackageNotFound { 
        package: String,
        available_packages: Option<Vec<String>>,
    },
    
    #[error("Template error: {message}")]
    TemplateError { 
        message: String,
        template_name: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Package resolution failed for '{package}': {reason}")]
    PackageResolution { 
        package: String,
        version: Option<String>,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Profile merge conflict at path '{path}': {details}")]
    ProfileMergeConflict { 
        path: String, 
        details: String,
        base_profile: Option<String>,
        differential_profile: Option<String>,
    },
    
    #[error("IR serialization failed for element '{element}': {reason}")]
    SerializationError { 
        element: String, 
        reason: String,
        context: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Invalid FHIR structure: {message}")]
    InvalidStructure { 
        message: String,
        resource_type: Option<String>,
        element_path: Option<String>,
    },
    
    #[error("Cache operation '{operation}' failed: {reason}")]
    CacheError { 
        operation: String,
        reason: String,
        #[source]
        source: Option<std::io::Error>,
    },
    
    #[error("Network operation failed: {message}")]
    NetworkError {
        message: String,
        url: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Configuration error: {message}")]
    ConfigurationError {
        message: String,
        field: Option<String>,
        expected_type: Option<String>,
    },
    
    #[error("Validation failed: {message}")]
    ValidationError {
        message: String,
        violations: Vec<ValidationViolation>,
    },
    
    #[error("Resource dependency error: {message}")]
    DependencyError {
        message: String,
        missing_resource: String,
        required_by: Option<String>,
    },
}

/// Represents a specific validation violation
#[derive(Debug, Clone)]
pub struct ValidationViolation {
    pub path: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

/// Severity levels for validation violations
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Information,
}

/// Error severity classification for logging and handling
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Critical,  // System cannot continue
    High,      // Feature unavailable but system can continue
    Medium,    // Degraded functionality
    Low,       // Minor issues, full functionality available
}

/// Recovery options for different error types
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryOption {
    Retry,
    UseDefault,
    Skip,
    UserIntervention,
    None,
}

impl CoreError {
    /// Get the severity level of this error
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            CoreError::ParseError { .. } => ErrorSeverity::High,
            CoreError::PackageNotFound { .. } => ErrorSeverity::Critical,
            CoreError::TemplateError { .. } => ErrorSeverity::High,
            CoreError::PackageResolution { .. } => ErrorSeverity::Critical,
            CoreError::ProfileMergeConflict { .. } => ErrorSeverity::High,
            CoreError::SerializationError { .. } => ErrorSeverity::Medium,
            CoreError::InvalidStructure { .. } => ErrorSeverity::High,
            CoreError::CacheError { .. } => ErrorSeverity::Medium,
            CoreError::NetworkError { .. } => ErrorSeverity::Medium,
            CoreError::ConfigurationError { .. } => ErrorSeverity::Critical,
            CoreError::ValidationError { violations, .. } => {
                if violations.iter().any(|v| v.severity == ValidationSeverity::Error) {
                    ErrorSeverity::High
                } else {
                    ErrorSeverity::Medium
                }
            }
            CoreError::DependencyError { .. } => ErrorSeverity::High,
        }
    }
    
    /// Get suggested recovery options for this error
    pub fn recovery_options(&self) -> Vec<RecoveryOption> {
        match self {
            CoreError::ParseError { .. } => vec![RecoveryOption::UserIntervention],
            CoreError::PackageNotFound { .. } => vec![RecoveryOption::UserIntervention],
            CoreError::TemplateError { .. } => vec![RecoveryOption::UserIntervention],
            CoreError::PackageResolution { .. } => vec![RecoveryOption::Retry, RecoveryOption::UserIntervention],
            CoreError::ProfileMergeConflict { .. } => vec![RecoveryOption::UserIntervention],
            CoreError::SerializationError { .. } => vec![RecoveryOption::Skip, RecoveryOption::UserIntervention],
            CoreError::InvalidStructure { .. } => vec![RecoveryOption::Skip, RecoveryOption::UserIntervention],
            CoreError::CacheError { .. } => vec![RecoveryOption::Retry, RecoveryOption::UseDefault],
            CoreError::NetworkError { .. } => vec![RecoveryOption::Retry],
            CoreError::ConfigurationError { .. } => vec![RecoveryOption::UseDefault, RecoveryOption::UserIntervention],
            CoreError::ValidationError { .. } => vec![RecoveryOption::Skip, RecoveryOption::UserIntervention],
            CoreError::DependencyError { .. } => vec![RecoveryOption::UserIntervention],
        }
    }
    
    /// Get a user-friendly error message with context
    pub fn user_message(&self) -> String {
        match self {
            CoreError::ParseError { message, .. } => {
                format!("Unable to parse FHIR resource. {}", message)
            }
            CoreError::PackageNotFound { package, available_packages } => {
                let mut msg = format!("The FHIR package '{}' could not be found.", package);
                if let Some(packages) = available_packages {
                    if !packages.is_empty() {
                        msg.push_str(&format!(" Available packages: {}", packages.join(", ")));
                    }
                }
                msg
            }
            CoreError::TemplateError { message, template_name, .. } => {
                match template_name {
                    Some(name) => format!("Template '{}' error: {}", name, message),
                    None => format!("Template error: {}", message),
                }
            }
            CoreError::PackageResolution { package, version, reason, .. } => {
                match version {
                    Some(v) => format!("Failed to resolve package '{}' version '{}': {}", package, v, reason),
                    None => format!("Failed to resolve package '{}': {}", package, reason),
                }
            }
            CoreError::ProfileMergeConflict { path, details, base_profile, differential_profile } => {
                let mut msg = format!("Profile merge conflict at '{}': {}", path, details);
                if let (Some(base), Some(diff)) = (base_profile, differential_profile) {
                    msg.push_str(&format!(" (merging '{}' with '{}')", base, diff));
                }
                msg
            }
            CoreError::SerializationError { element, reason, context, .. } => {
                let mut msg = format!("Failed to serialize element '{}': {}", element, reason);
                if let Some(ctx) = context {
                    msg.push_str(&format!(" Context: {}", ctx));
                }
                msg
            }
            CoreError::InvalidStructure { message, resource_type, element_path } => {
                let mut msg = format!("Invalid FHIR structure: {}", message);
                if let Some(resource) = resource_type {
                    msg.push_str(&format!(" (resource: {})", resource));
                }
                if let Some(path) = element_path {
                    msg.push_str(&format!(" (path: {})", path));
                }
                msg
            }
            CoreError::CacheError { operation, reason, .. } => {
                format!("Cache operation '{}' failed: {}", operation, reason)
            }
            CoreError::NetworkError { message, url, .. } => {
                match url {
                    Some(u) => format!("Network error accessing '{}': {}", u, message),
                    None => format!("Network error: {}", message),
                }
            }
            CoreError::ConfigurationError { message, field, expected_type } => {
                let mut msg = format!("Configuration error: {}", message);
                if let Some(f) = field {
                    msg.push_str(&format!(" (field: {})", f));
                }
                if let Some(t) = expected_type {
                    msg.push_str(&format!(" (expected type: {})", t));
                }
                msg
            }
            CoreError::ValidationError { message, violations } => {
                let mut msg = format!("Validation failed: {}", message);
                if !violations.is_empty() {
                    msg.push_str(&format!(" ({} violations)", violations.len()));
                }
                msg
            }
            CoreError::DependencyError { message, missing_resource, required_by } => {
                let mut msg = format!("Dependency error: {} (missing: {})", message, missing_resource);
                if let Some(required) = required_by {
                    msg.push_str(&format!(" (required by: {})", required));
                }
                msg
            }
        }
    }
    
    /// Add context to an existing error
    pub fn with_context<S: Into<String>>(mut self, context: S) -> Self {
        let context_str = context.into();
        match &mut self {
            CoreError::SerializationError { context: ctx, .. } => {
                *ctx = Some(context_str);
            }
            _ => {
                // For other error types, we could extend them to include context
                // For now, we'll leave them as-is
            }
        }
        self
    }
}

impl From<tera::Error> for CoreError {
    fn from(err: tera::Error) -> Self {
        CoreError::TemplateError {
            message: err.to_string(),
            template_name: None,
            source: Some(Box::new(err)),
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::CacheError {
            operation: "file operation".to_string(),
            reason: err.to_string(),
            source: Some(err),
        }
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        CoreError::SerializationError {
            element: "unknown".to_string(),
            reason: err.to_string(),
            context: None,
            source: Some(Box::new(err)),
        }
    }
}

/// Convenience type alias for Results with CoreError
pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display() {
        let error = CoreError::ParseError {
            message: "Invalid FHIR format".to_string(),
            source: None,
        };
        assert_eq!(error.to_string(), "Failed to parse FHIR resource: Invalid FHIR format");
    }

    #[test]
    fn test_package_not_found_error_display() {
        let error = CoreError::PackageNotFound {
            package: "hl7.fhir.r4.core".to_string(),
            available_packages: None,
        };
        assert_eq!(error.to_string(), "Package not found: hl7.fhir.r4.core");
    }

    #[test]
    fn test_template_error_display() {
        let error = CoreError::TemplateError {
            message: "Template compilation failed".to_string(),
            template_name: None,
            source: None,
        };
        assert_eq!(error.to_string(), "Template error: Template compilation failed");
    }

    #[test]
    fn test_error_debug_format() {
        let error = CoreError::ParseError {
            message: "test".to_string(),
            source: None,
        };
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ParseError"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_tera_error_conversion() {
        let tera_error = tera::Error::msg("Template not found");
        let core_error: CoreError = tera_error.into();
        
        match core_error {
            CoreError::TemplateError { message, .. } => {
                assert!(message.contains("Template not found"));
            }
            _ => panic!("Expected TemplateError"),
        }
    }

    #[test]
    fn test_error_severity_classification() {
        let parse_error = CoreError::ParseError {
            message: "test".to_string(),
            source: None,
        };
        assert_eq!(parse_error.severity(), ErrorSeverity::High);

        let package_error = CoreError::PackageNotFound {
            package: "test".to_string(),
            available_packages: None,
        };
        assert_eq!(package_error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_recovery_options() {
        let network_error = CoreError::NetworkError {
            message: "Connection failed".to_string(),
            url: None,
            source: None,
        };
        let options = network_error.recovery_options();
        assert!(options.contains(&RecoveryOption::Retry));
    }

    #[test]
    fn test_user_friendly_messages() {
        let error = CoreError::PackageNotFound {
            package: "test-package".to_string(),
            available_packages: Some(vec!["pkg1".to_string(), "pkg2".to_string()]),
        };
        let message = error.user_message();
        assert!(message.contains("test-package"));
        assert!(message.contains("pkg1, pkg2"));
    }

    #[test]
    fn test_validation_error_severity() {
        let violations = vec![
            ValidationViolation {
                path: "test.path".to_string(),
                message: "Error violation".to_string(),
                severity: ValidationSeverity::Error,
            },
            ValidationViolation {
                path: "test.path2".to_string(),
                message: "Warning violation".to_string(),
                severity: ValidationSeverity::Warning,
            },
        ];
        
        let error = CoreError::ValidationError {
            message: "Multiple violations".to_string(),
            violations,
        };
        
        assert_eq!(error.severity(), ErrorSeverity::High);
    }

    #[test]
    fn test_error_context_addition() {
        let error = CoreError::SerializationError {
            element: "test".to_string(),
            reason: "failed".to_string(),
            context: None,
            source: None,
        };
        
        let error_with_context = error.with_context("additional context");
        match error_with_context {
            CoreError::SerializationError { context: Some(ctx), .. } => {
                assert_eq!(ctx, "additional context");
            }
            _ => panic!("Expected SerializationError with context"),
        }
    }
}