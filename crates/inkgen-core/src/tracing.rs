//! Tracing and logging utilities for structured diagnostics

use crate::{CoreError, ErrorSeverity};
use std::time::Instant;
use tracing::{debug, error, info, warn, Span};

/// Performance monitoring utilities
pub struct PerformanceMonitor {
    start_time: Instant,
    operation: String,
}

impl PerformanceMonitor {
    /// Start monitoring a new operation
    pub fn start<S: Into<String>>(operation: S) -> Self {
        let operation = operation.into();
        debug!("Starting operation: {}", operation);
        Self {
            start_time: Instant::now(),
            operation,
        }
    }
    
    /// Record a checkpoint with elapsed time
    pub fn checkpoint<S: AsRef<str>>(&self, checkpoint: S) {
        let elapsed = self.start_time.elapsed();
        debug!(
            operation = %self.operation,
            checkpoint = %checkpoint.as_ref(),
            elapsed_ms = elapsed.as_millis(),
            "Operation checkpoint"
        );
    }
    
    /// Finish monitoring and log final duration
    pub fn finish(self) {
        let elapsed = self.start_time.elapsed();
        info!(
            operation = %self.operation,
            total_duration_ms = elapsed.as_millis(),
            "Operation completed"
        );
    }
    
    /// Finish with error and log failure
    pub fn finish_with_error(self, error: &CoreError) {
        let elapsed = self.start_time.elapsed();
        let severity = error.severity();
        
        match severity {
            ErrorSeverity::Critical => error!(
                operation = %self.operation,
                total_duration_ms = elapsed.as_millis(),
                error = %error,
                severity = ?severity,
                "Operation failed critically"
            ),
            ErrorSeverity::High => error!(
                operation = %self.operation,
                total_duration_ms = elapsed.as_millis(),
                error = %error,
                severity = ?severity,
                "Operation failed with high severity"
            ),
            ErrorSeverity::Medium => warn!(
                operation = %self.operation,
                total_duration_ms = elapsed.as_millis(),
                error = %error,
                severity = ?severity,
                "Operation failed with medium severity"
            ),
            ErrorSeverity::Low => info!(
                operation = %self.operation,
                total_duration_ms = elapsed.as_millis(),
                error = %error,
                severity = ?severity,
                "Operation completed with minor issues"
            ),
        }
    }
}

/// Diagnostic context for package resolution operations
pub struct PackageResolutionContext {
    pub package_name: String,
    pub version: Option<String>,
    pub operation_id: String,
}

impl PackageResolutionContext {
    pub fn new<S: Into<String>>(package_name: S, version: Option<String>) -> Self {
        Self {
            package_name: package_name.into(),
            version,
            operation_id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    /// Create a tracing span for this context
    pub fn span(&self) -> Span {
        tracing::info_span!(
            "package_resolution",
            package = %self.package_name,
            version = ?self.version,
            operation_id = %self.operation_id
        )
    }
    
    /// Log package resolution start
    pub fn log_start(&self) {
        info!(
            package = %self.package_name,
            version = ?self.version,
            operation_id = %self.operation_id,
            "Starting package resolution"
        );
    }
    
    /// Log package resolution success
    pub fn log_success(&self, resource_count: usize, cache_hit: bool) {
        info!(
            package = %self.package_name,
            version = ?self.version,
            operation_id = %self.operation_id,
            resource_count = resource_count,
            cache_hit = cache_hit,
            "Package resolution completed successfully"
        );
    }
    
    /// Log package resolution failure
    pub fn log_failure(&self, error: &CoreError) {
        error!(
            package = %self.package_name,
            version = ?self.version,
            operation_id = %self.operation_id,
            error = %error,
            error_severity = ?error.severity(),
            "Package resolution failed"
        );
    }
}

/// Diagnostic context for profile processing operations
pub struct ProfileProcessingContext {
    pub profile_url: String,
    pub base_profile: Option<String>,
    pub operation_id: String,
}

impl ProfileProcessingContext {
    pub fn new<S: Into<String>>(profile_url: S, base_profile: Option<String>) -> Self {
        Self {
            profile_url: profile_url.into(),
            base_profile,
            operation_id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    /// Create a tracing span for this context
    pub fn span(&self) -> Span {
        tracing::info_span!(
            "profile_processing",
            profile_url = %self.profile_url,
            base_profile = ?self.base_profile,
            operation_id = %self.operation_id
        )
    }
    
    /// Log profile processing start
    pub fn log_start(&self) {
        info!(
            profile_url = %self.profile_url,
            base_profile = ?self.base_profile,
            operation_id = %self.operation_id,
            "Starting profile processing"
        );
    }
    
    /// Log profile merge operation
    pub fn log_merge(&self, element_count: usize, conflicts: usize) {
        if conflicts > 0 {
            warn!(
                profile_url = %self.profile_url,
                operation_id = %self.operation_id,
                element_count = element_count,
                conflicts = conflicts,
                "Profile merge completed with conflicts"
            );
        } else {
            debug!(
                profile_url = %self.profile_url,
                operation_id = %self.operation_id,
                element_count = element_count,
                "Profile merge completed successfully"
            );
        }
    }
    
    /// Log profile processing success
    pub fn log_success(&self, element_count: usize, must_support_count: usize) {
        info!(
            profile_url = %self.profile_url,
            base_profile = ?self.base_profile,
            operation_id = %self.operation_id,
            element_count = element_count,
            must_support_count = must_support_count,
            "Profile processing completed successfully"
        );
    }
    
    /// Log profile processing failure
    pub fn log_failure(&self, error: &CoreError) {
        error!(
            profile_url = %self.profile_url,
            base_profile = ?self.base_profile,
            operation_id = %self.operation_id,
            error = %error,
            error_severity = ?error.severity(),
            "Profile processing failed"
        );
    }
}

/// Utility functions for structured logging
pub mod logging {
    use super::*;
    use tracing::{debug, info, warn, error};
    
    /// Log cache operations with structured data
    pub fn log_cache_operation<S: AsRef<str>>(
        operation: S,
        key: &str,
        hit: bool,
        size_bytes: Option<usize>,
    ) {
        if hit {
            debug!(
                operation = %operation.as_ref(),
                cache_key = %key,
                cache_hit = true,
                size_bytes = ?size_bytes,
                "Cache hit"
            );
        } else {
            debug!(
                operation = %operation.as_ref(),
                cache_key = %key,
                cache_hit = false,
                "Cache miss"
            );
        }
    }
    
    /// Log network operations with timing
    pub fn log_network_request<S: AsRef<str>>(
        url: S,
        method: &str,
        status_code: Option<u16>,
        duration_ms: u64,
    ) {
        match status_code {
            Some(code) if code >= 200 && code < 300 => {
                debug!(
                    url = %url.as_ref(),
                    method = %method,
                    status_code = code,
                    duration_ms = duration_ms,
                    "Network request successful"
                );
            }
            Some(code) if code >= 400 => {
                warn!(
                    url = %url.as_ref(),
                    method = %method,
                    status_code = code,
                    duration_ms = duration_ms,
                    "Network request failed"
                );
            }
            Some(code) => {
                info!(
                    url = %url.as_ref(),
                    method = %method,
                    status_code = code,
                    duration_ms = duration_ms,
                    "Network request completed"
                );
            }
            None => {
                error!(
                    url = %url.as_ref(),
                    method = %method,
                    duration_ms = duration_ms,
                    "Network request failed without response"
                );
            }
        }
    }
    
    /// Log validation results with detailed information
    pub fn log_validation_results(
        resource_type: &str,
        element_path: Option<&str>,
        violations: &[crate::ValidationViolation],
    ) {
        let error_count = violations.iter()
            .filter(|v| v.severity == crate::ValidationSeverity::Error)
            .count();
        let warning_count = violations.iter()
            .filter(|v| v.severity == crate::ValidationSeverity::Warning)
            .count();
        let info_count = violations.iter()
            .filter(|v| v.severity == crate::ValidationSeverity::Information)
            .count();
        
        if error_count > 0 {
            error!(
                resource_type = %resource_type,
                element_path = ?element_path,
                error_count = error_count,
                warning_count = warning_count,
                info_count = info_count,
                total_violations = violations.len(),
                "Validation failed with errors"
            );
        } else if warning_count > 0 {
            warn!(
                resource_type = %resource_type,
                element_path = ?element_path,
                warning_count = warning_count,
                info_count = info_count,
                total_violations = violations.len(),
                "Validation completed with warnings"
            );
        } else {
            debug!(
                resource_type = %resource_type,
                element_path = ?element_path,
                info_count = info_count,
                "Validation completed successfully"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_performance_monitor_creation() {
        let monitor = PerformanceMonitor::start("test_operation");
        assert_eq!(monitor.operation, "test_operation");
    }
    
    #[test]
    fn test_package_resolution_context() {
        let context = PackageResolutionContext::new("test.package", Some("1.0.0".to_string()));
        assert_eq!(context.package_name, "test.package");
        assert_eq!(context.version, Some("1.0.0".to_string()));
        assert!(!context.operation_id.is_empty());
    }
    
    #[test]
    fn test_profile_processing_context() {
        let context = ProfileProcessingContext::new(
            "http://example.com/profile",
            Some("base_profile".to_string())
        );
        assert_eq!(context.profile_url, "http://example.com/profile");
        assert_eq!(context.base_profile, Some("base_profile".to_string()));
        assert!(!context.operation_id.is_empty());
    }
}