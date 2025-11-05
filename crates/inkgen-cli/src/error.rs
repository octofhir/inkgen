use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Configuration file already exists: {path}. Use --force to overwrite")]
    ConfigExists { path: String },
    
    #[error("Invalid configuration: {message}")]
    InvalidConfig { 
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Package fetch failed: {package}")]
    PackageFetch { 
        package: String, 
        #[source] 
        source: Box<dyn std::error::Error + Send + Sync> 
    },
    
    #[error("Code generation failed: {reason}")]
    GenerationFailed { 
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("IO error during {operation}")]
    IoError { 
        operation: String, 
        #[source] 
        source: std::io::Error 
    },
    
    #[error("Network error: {operation}")]
    NetworkError {
        operation: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    
    #[error("File not found: {path}")]
    FileNotFound { 
        path: String,
        context: Option<String>,
    },
    
    #[error("Permission denied: {operation}")]
    PermissionDenied {
        operation: String,
        path: Option<String>,
    },
    
    #[error("Invalid command arguments: {message}")]
    InvalidArguments { message: String },
    
    #[error("Core engine error")]
    CoreError(#[from] anyhow::Error),
    
    #[error("TOML parsing error")]
    TomlError(#[from] toml::de::Error),
    
    #[error("Serialization error")]
    SerializationError(#[from] toml::ser::Error),
}

pub type CliResult<T> = Result<T, CliError>;

impl CliError {
    /// Provide user-friendly error messages with actionable suggestions
    pub fn user_message(&self) -> String {
        match self {
            CliError::ConfigExists { path } => {
                format!(
                    "Configuration file already exists at '{}'.\n\nSuggestions:\n• Use --force to overwrite the existing file\n• Choose a different output path with --output\n• Remove the existing file manually",
                    path
                )
            }
            CliError::PackageFetch { package, source } => {
                let mut message = format!("Failed to fetch package '{}'.", package);
                
                // Add specific suggestions based on the error type
                if let Some(io_err) = source.downcast_ref::<std::io::Error>() {
                    match io_err.kind() {
                        std::io::ErrorKind::NotFound => {
                            message.push_str("\n\nSuggestions:\n• Verify the package name is correct\n• Check if the package exists in the FHIR registry\n• Try using the full package name (e.g., 'hl7.fhir.r4.core')");
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            message.push_str("\n\nSuggestions:\n• Check file permissions in the cache directory\n• Run with appropriate permissions\n• Clear the cache directory if corrupted");
                        }
                        _ => {
                            message.push_str("\n\nSuggestions:\n• Check your internet connection\n• Verify the package name and version\n• Try again later if the registry is temporarily unavailable");
                        }
                    }
                } else {
                    message.push_str("\n\nSuggestions:\n• Check your internet connection\n• Verify the package name and version\n• Try again later if the registry is temporarily unavailable");
                }
                
                message
            }
            CliError::InvalidConfig { message, .. } => {
                format!(
                    "Configuration error: {}\n\nSuggestions:\n• Check your inkgen.toml file syntax\n• Validate TOML format using an online validator\n• Run 'inkgen config init --force' to regenerate the config file\n• Refer to the documentation for valid configuration options",
                    message
                )
            }
            CliError::GenerationFailed { reason, .. } => {
                format!(
                    "Code generation failed: {}\n\nSuggestions:\n• Verify your configuration file is valid\n• Ensure all required packages are fetched\n• Check that the output directory is writable\n• Try with a simpler configuration first",
                    reason
                )
            }
            CliError::IoError { operation, source } => {
                let mut message = format!("File system error during {}", operation);
                
                match source.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        message.push_str("\n\nSuggestions:\n• Check file and directory permissions\n• Run with appropriate user privileges\n• Ensure the target directory is writable");
                    }
                    std::io::ErrorKind::NotFound => {
                        message.push_str("\n\nSuggestions:\n• Verify the file or directory path exists\n• Create parent directories if needed\n• Check for typos in the path");
                    }
                    std::io::ErrorKind::AlreadyExists => {
                        message.push_str("\n\nSuggestions:\n• Use --force to overwrite existing files\n• Choose a different output location\n• Remove conflicting files manually");
                    }
                    _ => {
                        message.push_str("\n\nSuggestions:\n• Check available disk space\n• Verify file system integrity\n• Try a different location if possible");
                    }
                }
                
                message
            }
            CliError::NetworkError { operation, .. } => {
                format!(
                    "Network error during {}\n\nSuggestions:\n• Check your internet connection\n• Verify proxy settings if applicable\n• Try again later if the service is temporarily unavailable\n• Check firewall settings",
                    operation
                )
            }
            CliError::FileNotFound { path, context } => {
                let mut message = format!("File not found: {}", path);
                if let Some(ctx) = context {
                    message.push_str(&format!(" ({})", ctx));
                }
                message.push_str("\n\nSuggestions:\n• Verify the file path is correct\n• Check for typos in the filename\n• Ensure the file exists and is accessible\n• Use absolute paths if relative paths are causing issues");
                message
            }
            CliError::PermissionDenied { operation, path } => {
                let mut message = format!("Permission denied: {}", operation);
                if let Some(p) = path {
                    message.push_str(&format!(" ({})", p));
                }
                message.push_str("\n\nSuggestions:\n• Run with appropriate user privileges\n• Check file and directory permissions\n• Ensure you have write access to the target location\n• Use sudo if necessary (with caution)");
                message
            }
            CliError::InvalidArguments { message } => {
                format!(
                    "Invalid command arguments: {}\n\nSuggestions:\n• Use --help to see available options\n• Check the command syntax in the documentation\n• Verify argument values are in the correct format",
                    message
                )
            }
            CliError::CoreError(err) => {
                format!(
                    "Internal error: {}\n\nThis appears to be an internal issue. Please report this bug with the following details:\n• Command that caused the error\n• Configuration file contents\n• Error details: {}",
                    err, err
                )
            }
            CliError::TomlError(err) => {
                format!(
                    "TOML parsing error: {}\n\nSuggestions:\n• Check your inkgen.toml file syntax\n• Validate TOML format using an online validator\n• Look for missing quotes, brackets, or commas\n• Run 'inkgen config init --force' to regenerate the config file",
                    err
                )
            }
            CliError::SerializationError(err) => {
                format!(
                    "Serialization error: {}\n\nThis is likely an internal issue. Please report this bug with:\n• The configuration that caused the error\n• Steps to reproduce\n• Error details: {}",
                    err, err
                )
            }
        }
    }
    
    /// Create a new InvalidConfig error with optional source
    pub fn invalid_config<S: Into<String>>(message: S) -> Self {
        Self::InvalidConfig {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new InvalidConfig error with source
    pub fn invalid_config_with_source<S: Into<String>, E: std::error::Error + Send + Sync + 'static>(
        message: S,
        source: E,
    ) -> Self {
        Self::InvalidConfig {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new GenerationFailed error with optional source
    pub fn generation_failed<S: Into<String>>(reason: S) -> Self {
        Self::GenerationFailed {
            reason: reason.into(),
            source: None,
        }
    }
    
    /// Create a new GenerationFailed error with source
    pub fn generation_failed_with_source<S: Into<String>, E: std::error::Error + Send + Sync + 'static>(
        reason: S,
        source: E,
    ) -> Self {
        Self::GenerationFailed {
            reason: reason.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new NetworkError
    pub fn network_error<S: Into<String>, E: std::error::Error + Send + Sync + 'static>(
        operation: S,
        source: E,
    ) -> Self {
        Self::NetworkError {
            operation: operation.into(),
            source: Box::new(source),
        }
    }
    
    /// Create a new FileNotFound error
    pub fn file_not_found<S: Into<String>>(path: S) -> Self {
        Self::FileNotFound {
            path: path.into(),
            context: None,
        }
    }
    
    /// Create a new FileNotFound error with context
    pub fn file_not_found_with_context<S: Into<String>, C: Into<String>>(path: S, context: C) -> Self {
        Self::FileNotFound {
            path: path.into(),
            context: Some(context.into()),
        }
    }
    
    /// Create a new PermissionDenied error
    pub fn permission_denied<S: Into<String>>(operation: S) -> Self {
        Self::PermissionDenied {
            operation: operation.into(),
            path: None,
        }
    }
    
    /// Create a new PermissionDenied error with path
    pub fn permission_denied_with_path<S: Into<String>, P: Into<String>>(operation: S, path: P) -> Self {
        Self::PermissionDenied {
            operation: operation.into(),
            path: Some(path.into()),
        }
    }
    
    /// Create a new InvalidArguments error
    pub fn invalid_arguments<S: Into<String>>(message: S) -> Self {
        Self::InvalidArguments {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    
    #[test]
    fn test_invalid_config_constructor() {
        let error = CliError::invalid_config("Test message");
        match error {
            CliError::InvalidConfig { message, source } => {
                assert_eq!(message, "Test message");
                assert!(source.is_none());
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }
    
    #[test]
    fn test_invalid_config_with_source() {
        let source_error = io::Error::new(io::ErrorKind::NotFound, "Source error");
        let error = CliError::invalid_config_with_source("Test message", source_error);
        
        match error {
            CliError::InvalidConfig { message, source } => {
                assert_eq!(message, "Test message");
                assert!(source.is_some());
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }
    
    #[test]
    fn test_generation_failed_constructor() {
        let error = CliError::generation_failed("Generation failed");
        match error {
            CliError::GenerationFailed { reason, source } => {
                assert_eq!(reason, "Generation failed");
                assert!(source.is_none());
            }
            _ => panic!("Expected GenerationFailed error"),
        }
    }
    
    #[test]
    fn test_generation_failed_with_source() {
        let source_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let error = CliError::generation_failed_with_source("Generation failed", source_error);
        
        match error {
            CliError::GenerationFailed { reason, source } => {
                assert_eq!(reason, "Generation failed");
                assert!(source.is_some());
            }
            _ => panic!("Expected GenerationFailed error"),
        }
    }
    
    #[test]
    fn test_network_error_constructor() {
        let source_error = io::Error::new(io::ErrorKind::TimedOut, "Connection timeout");
        let error = CliError::network_error("downloading package", source_error);
        
        match error {
            CliError::NetworkError { operation, source: _ } => {
                assert_eq!(operation, "downloading package");
            }
            _ => panic!("Expected NetworkError error"),
        }
    }
    
    #[test]
    fn test_file_not_found_constructor() {
        let error = CliError::file_not_found("/path/to/file.txt");
        match error {
            CliError::FileNotFound { path, context } => {
                assert_eq!(path, "/path/to/file.txt");
                assert!(context.is_none());
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }
    
    #[test]
    fn test_file_not_found_with_context() {
        let error = CliError::file_not_found_with_context("/path/to/file.txt", "configuration file");
        match error {
            CliError::FileNotFound { path, context } => {
                assert_eq!(path, "/path/to/file.txt");
                assert_eq!(context, Some("configuration file".to_string()));
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }
    
    #[test]
    fn test_permission_denied_constructor() {
        let error = CliError::permission_denied("writing file");
        match error {
            CliError::PermissionDenied { operation, path } => {
                assert_eq!(operation, "writing file");
                assert!(path.is_none());
            }
            _ => panic!("Expected PermissionDenied error"),
        }
    }
    
    #[test]
    fn test_permission_denied_with_path() {
        let error = CliError::permission_denied_with_path("writing file", "/protected/path");
        match error {
            CliError::PermissionDenied { operation, path } => {
                assert_eq!(operation, "writing file");
                assert_eq!(path, Some("/protected/path".to_string()));
            }
            _ => panic!("Expected PermissionDenied error"),
        }
    }
    
    #[test]
    fn test_invalid_arguments_constructor() {
        let error = CliError::invalid_arguments("Missing required argument");
        match error {
            CliError::InvalidArguments { message } => {
                assert_eq!(message, "Missing required argument");
            }
            _ => panic!("Expected InvalidArguments error"),
        }
    }
    
    #[test]
    fn test_user_message_config_exists() {
        let error = CliError::ConfigExists {
            path: "/path/to/config.toml".to_string(),
        };
        
        let message = error.user_message();
        assert!(message.contains("/path/to/config.toml"));
        assert!(message.contains("--force"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_invalid_config() {
        let error = CliError::InvalidConfig {
            message: "Syntax error in TOML".to_string(),
            source: None,
        };
        
        let message = error.user_message();
        assert!(message.contains("Syntax error in TOML"));
        assert!(message.contains("inkgen.toml"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_package_fetch_not_found() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "Package not found");
        let error = CliError::PackageFetch {
            package: "hl7.fhir.r4.core".to_string(),
            source: Box::new(io_error),
        };
        
        let message = error.user_message();
        assert!(message.contains("hl7.fhir.r4.core"));
        assert!(message.contains("package name is correct"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_package_fetch_permission_denied() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let error = CliError::PackageFetch {
            package: "hl7.fhir.r4.core".to_string(),
            source: Box::new(io_error),
        };
        
        let message = error.user_message();
        assert!(message.contains("hl7.fhir.r4.core"));
        assert!(message.contains("file permissions"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_generation_failed() {
        let error = CliError::GenerationFailed {
            reason: "Invalid resource structure".to_string(),
            source: None,
        };
        
        let message = error.user_message();
        assert!(message.contains("Invalid resource structure"));
        assert!(message.contains("configuration file"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_io_error_permission_denied() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let error = CliError::IoError {
            operation: "creating directory".to_string(),
            source: io_error,
        };
        
        let message = error.user_message();
        assert!(message.contains("creating directory"));
        assert!(message.contains("permissions"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_io_error_not_found() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let error = CliError::IoError {
            operation: "reading file".to_string(),
            source: io_error,
        };
        
        let message = error.user_message();
        assert!(message.contains("reading file"));
        assert!(message.contains("path exists"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_network_error() {
        let io_error = io::Error::new(io::ErrorKind::TimedOut, "Connection timeout");
        let error = CliError::NetworkError {
            operation: "downloading package".to_string(),
            source: Box::new(io_error),
        };
        
        let message = error.user_message();
        assert!(message.contains("downloading package"));
        assert!(message.contains("internet connection"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_file_not_found() {
        let error = CliError::FileNotFound {
            path: "/missing/file.txt".to_string(),
            context: Some("configuration file".to_string()),
        };
        
        let message = error.user_message();
        assert!(message.contains("/missing/file.txt"));
        assert!(message.contains("configuration file"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_permission_denied() {
        let error = CliError::PermissionDenied {
            operation: "writing file".to_string(),
            path: Some("/protected/path".to_string()),
        };
        
        let message = error.user_message();
        assert!(message.contains("writing file"));
        assert!(message.contains("/protected/path"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_user_message_invalid_arguments() {
        let error = CliError::InvalidArguments {
            message: "Missing required argument".to_string(),
        };
        
        let message = error.user_message();
        assert!(message.contains("Missing required argument"));
        assert!(message.contains("--help"));
        assert!(message.contains("Suggestions:"));
    }
    
    #[test]
    fn test_error_display() {
        let error = CliError::InvalidConfig {
            message: "Test error".to_string(),
            source: None,
        };
        
        let display_message = format!("{}", error);
        assert!(display_message.contains("Invalid configuration"));
        assert!(display_message.contains("Test error"));
    }
    
    #[test]
    fn test_error_debug() {
        let error = CliError::InvalidArguments {
            message: "Debug test".to_string(),
        };
        
        let debug_message = format!("{:?}", error);
        assert!(debug_message.contains("InvalidArguments"));
        assert!(debug_message.contains("Debug test"));
    }
    
    #[test]
    fn test_toml_error_conversion() {
        let toml_content = "invalid toml [";
        let toml_error = toml::from_str::<toml::Value>(toml_content).unwrap_err();
        let cli_error: CliError = toml_error.into();
        
        match cli_error {
            CliError::TomlError(_) => {}, // Expected
            _ => panic!("Expected TomlError conversion"),
        }
    }
    
    #[test]
    fn test_anyhow_error_conversion() {
        let anyhow_error = anyhow::anyhow!("Test anyhow error");
        let cli_error: CliError = anyhow_error.into();
        
        match cli_error {
            CliError::CoreError(_) => {}, // Expected
            _ => panic!("Expected CoreError conversion"),
        }
    }
}