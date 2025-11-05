/// Core error types for FHIR processing
#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("Failed to parse FHIR resource: {0}")]
    ParseError(String),
    
    #[error("Package not found: {package}")]
    PackageNotFound { package: String },
    
    #[error("Template error: {0}")]
    TemplateError(String),
}

impl From<tera::Error> for CoreError {
    fn from(err: tera::Error) -> Self {
        CoreError::TemplateError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display() {
        let error = CoreError::ParseError("Invalid FHIR format".to_string());
        assert_eq!(error.to_string(), "Failed to parse FHIR resource: Invalid FHIR format");
    }

    #[test]
    fn test_package_not_found_error_display() {
        let error = CoreError::PackageNotFound {
            package: "hl7.fhir.r4.core".to_string(),
        };
        assert_eq!(error.to_string(), "Package not found: hl7.fhir.r4.core");
    }

    #[test]
    fn test_template_error_display() {
        let error = CoreError::TemplateError("Template compilation failed".to_string());
        assert_eq!(error.to_string(), "Template error: Template compilation failed");
    }

    #[test]
    fn test_error_debug_format() {
        let error = CoreError::ParseError("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ParseError"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_tera_error_conversion() {
        // Create a simple tera error scenario
        let tera_error = tera::Error::msg("Template not found");
        let core_error: CoreError = tera_error.into();
        
        match core_error {
            CoreError::TemplateError(msg) => {
                assert!(msg.contains("Template not found"));
            }
            _ => panic!("Expected TemplateError"),
        }
    }
}