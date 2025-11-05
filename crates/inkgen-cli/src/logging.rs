use crate::error::{CliError, CliResult};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use tracing_subscriber::{
    fmt::format::FmtSpan,
    EnvFilter,
};

/// Set up structured logging with tracing
pub fn setup_logging(verbose: bool, log_level: Option<String>) -> CliResult<()> {
    let env_filter = create_env_filter(verbose, log_level)?;
    
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(verbose) // Show targets in verbose mode
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(verbose) // Show file locations in verbose mode
        .with_line_number(verbose); // Show line numbers in verbose mode
    
    // Configure span events for structured logging
    if verbose {
        subscriber
            .with_span_events(FmtSpan::ENTER | FmtSpan::CLOSE)
            .try_init()
    } else {
        subscriber
            .with_span_events(FmtSpan::NONE)
            .try_init()
    }
    .map_err(|e| CliError::invalid_config(format!("Failed to initialize logging: {}", e)))?;
    
    Ok(())
}

/// Create environment filter for log level configuration
fn create_env_filter(verbose: bool, log_level: Option<String>) -> CliResult<EnvFilter> {
    let filter_string = if let Some(level) = log_level {
        validate_log_level(&level)?;
        format!("inkgen={}", level)
    } else if verbose {
        "inkgen=debug".to_string()
    } else {
        // Default filter: info for inkgen, warn for dependencies
        "inkgen=info,warn".to_string()
    };
    
    // Try to parse from environment first, then fall back to our filter
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&filter_string))
        .map_err(|e| CliError::invalid_config_with_source(
            format!("Invalid log filter: {}", filter_string),
            e
        ))?;
    
    Ok(env_filter)
}



/// Validate log level string
fn validate_log_level(level: &str) -> CliResult<()> {
    match level.to_lowercase().as_str() {
        "trace" | "debug" | "info" | "warn" | "error" | "off" => Ok(()),
        _ => Err(CliError::invalid_arguments(format!(
            "Invalid log level '{}'. Valid levels: trace, debug, info, warn, error, off",
            level
        ))),
    }
}

/// Progress bar utilities for long-running operations
pub struct ProgressReporter {
    bar: ProgressBar,
}

impl ProgressReporter {
    /// Create a new progress bar for indeterminate operations
    pub fn new_spinner(message: &str) -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );
        bar.set_message(message.to_string());
        bar.enable_steady_tick(Duration::from_millis(100));
        
        Self { bar }
    }
    
    /// Create a new progress bar for determinate operations
    pub fn new_progress(total: u64, message: &str) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        bar.set_message(message.to_string());
        
        Self { bar }
    }
    
    /// Update the progress bar message
    pub fn set_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }
    
    /// Increment progress (for determinate progress bars)
    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }
    
    /// Set progress position (for determinate progress bars)
    pub fn set_position(&self, pos: u64) {
        self.bar.set_position(pos);
    }
    
    /// Finish the progress bar with a success message
    pub fn finish_with_message(&self, message: &str) {
        self.bar.finish_with_message(message.to_string());
    }
    
    /// Finish the progress bar and clear it
    pub fn finish_and_clear(&self) {
        self.bar.finish_and_clear();
    }
    
    /// Abandon the progress bar (for error cases)
    pub fn abandon_with_message(&self, message: &str) {
        self.bar.abandon_with_message(message.to_string());
    }
}

impl Drop for ProgressReporter {
    fn drop(&mut self) {
        if !self.bar.is_finished() {
            self.bar.abandon();
        }
    }
}

/// Structured logging macros for command execution spans
#[macro_export]
macro_rules! command_span {
    ($command:expr) => {
        tracing::info_span!("command", command = $command)
    };
    ($command:expr, $($field:tt)*) => {
        tracing::info_span!("command", command = $command, $($field)*)
    };
}

#[macro_export]
macro_rules! operation_span {
    ($operation:expr) => {
        tracing::debug_span!("operation", operation = $operation)
    };
    ($operation:expr, $($field:tt)*) => {
        tracing::debug_span!("operation", operation = $operation, $($field)*)
    };
}

/// Log command start with structured information
pub fn log_command_start(command: &str, args: &[String]) {
    tracing::info!(
        command = command,
        args = ?args,
        "Starting command execution"
    );
}

/// Log command completion with timing
pub fn log_command_complete(command: &str, duration: Duration) {
    tracing::info!(
        command = command,
        duration_ms = duration.as_millis(),
        "Command completed successfully"
    );
}

/// Log command failure with error context
pub fn log_command_error(command: &str, error: &CliError, duration: Duration) {
    tracing::error!(
        command = command,
        error = %error,
        duration_ms = duration.as_millis(),
        "Command failed"
    );
}

/// Log operation progress for long-running tasks
pub fn log_operation_progress(operation: &str, current: u64, total: u64, message: &str) {
    tracing::debug!(
        operation = operation,
        current = current,
        total = total,
        progress_pct = (current as f64 / total as f64 * 100.0) as u32,
        message = message,
        "Operation progress"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_log_level() {
        // Valid levels
        assert!(validate_log_level("trace").is_ok());
        assert!(validate_log_level("debug").is_ok());
        assert!(validate_log_level("info").is_ok());
        assert!(validate_log_level("warn").is_ok());
        assert!(validate_log_level("error").is_ok());
        assert!(validate_log_level("off").is_ok());
        
        // Case insensitive
        assert!(validate_log_level("INFO").is_ok());
        assert!(validate_log_level("Debug").is_ok());
        
        // Invalid levels
        assert!(validate_log_level("invalid").is_err());
        assert!(validate_log_level("").is_err());
        assert!(validate_log_level("verbose").is_err());
    }
    
    #[test]
    fn test_create_env_filter() {
        // Default case
        let filter = create_env_filter(false, None).unwrap();
        let filter_str = format!("{:?}", filter);
        // The filter format may vary, so just check it was created successfully
        assert!(!filter_str.is_empty());
        
        // Verbose case
        let filter = create_env_filter(true, None).unwrap();
        let filter_str = format!("{:?}", filter);
        assert!(!filter_str.is_empty());
        
        // Explicit level
        let filter = create_env_filter(false, Some("warn".to_string())).unwrap();
        let filter_str = format!("{:?}", filter);
        assert!(!filter_str.is_empty());
        
        // Invalid level
        assert!(create_env_filter(false, Some("invalid".to_string())).is_err());
    }
    
    #[test]
    fn test_progress_reporter() {
        // Test spinner creation
        let spinner = ProgressReporter::new_spinner("Testing...");
        spinner.set_message("Updated message");
        spinner.finish_with_message("Done!");
        
        // Test progress bar creation
        let progress = ProgressReporter::new_progress(100, "Processing...");
        progress.inc(10);
        progress.set_position(50);
        progress.finish_and_clear();
    }
}