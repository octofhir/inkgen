//! FHIR date/time parsing and formatting utilities.
//!
//! This module generates TypeScript helpers for working with FHIR date, dateTime,
//! instant, and time primitives, handling the various formats and parsing rules.

use serde::Serialize;

/// Configuration for date utility generation
#[derive(Debug, Clone, Serialize)]
pub struct DateConfig {
    /// Generate parsing functions
    pub parsers: bool,
    /// Generate formatting functions
    pub formatters: bool,
    /// Generate validation functions
    pub validators: bool,
}

impl Default for DateConfig {
    fn default() -> Self {
        Self {
            parsers: true,
            formatters: true,
            validators: true,
        }
    }
}

/// Date utility generation helper
#[derive(Debug, Clone, Serialize)]
pub struct DateHelpers {
    /// Whether to generate parsers
    pub has_parsers: bool,
    /// Whether to generate formatters
    pub has_formatters: bool,
    /// Whether to generate validators
    pub has_validators: bool,
}

impl DateHelpers {
    /// Creates date helpers with configuration
    pub fn new(config: &DateConfig) -> Self {
        Self {
            has_parsers: config.parsers,
            has_formatters: config.formatters,
            has_validators: config.validators,
        }
    }

    /// Generate FHIR date parser
    ///
    /// Handles formats: YYYY, YYYY-MM, YYYY-MM-DD
    pub fn generate_date_parser() -> String {
        r#"/**
 * Parse a FHIR date string (YYYY, YYYY-MM, or YYYY-MM-DD) to a Date object
 * @param date - The FHIR date string
 * @returns A Date object (uses first day of month/year for partial dates)
 * @throws Error if the date format is invalid
 */
export function parseFHIRDate(date: string): Date {
  const match = date.match(/^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/);
  if (!match) {
    throw new Error(`Invalid FHIR date format: ${date}`);
  }

  const [, year, month = '01', day = '01'] = match;
  return new Date(`${year}-${month}-${day}T00:00:00.000Z`);
}"#
        .to_string()
    }

    /// Generate FHIR dateTime parser
    pub fn generate_datetime_parser() -> String {
        r#"/**
 * Parse a FHIR dateTime string to a Date object
 * Handles partial precision (YYYY, YYYY-MM, YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, etc.)
 * @param dateTime - The FHIR dateTime string
 * @returns A Date object
 * @throws Error if the dateTime format is invalid
 */
export function parseFHIRDateTime(dateTime: string): Date {
  // FHIR dateTime format: YYYY-MM-DDTHH:MM:SS[.SSS](Z|+/-HH:MM)
  const date = new Date(dateTime);
  if (isNaN(date.getTime())) {
    throw new Error(`Invalid FHIR dateTime format: ${dateTime}`);
  }
  return date;
}"#
        .to_string()
    }

    /// Generate FHIR instant parser
    pub fn generate_instant_parser() -> String {
        r#"/**
 * Parse a FHIR instant string to a Date object
 * instant requires full precision with timezone
 * @param instant - The FHIR instant string
 * @returns A Date object
 * @throws Error if the instant format is invalid
 */
export function parseFHIRInstant(instant: string): Date {
  const date = new Date(instant);
  if (isNaN(date.getTime())) {
    throw new Error(`Invalid FHIR instant format: ${instant}`);
  }
  return date;
}"#
        .to_string()
    }

    /// Generate FHIR time parser
    pub fn generate_time_parser() -> String {
        r#"/**
 * Parse a FHIR time string (HH:MM:SS or HH:MM:SS.SSS)
 * @param time - The FHIR time string
 * @returns An object with hours, minutes, seconds, and milliseconds
 * @throws Error if the time format is invalid
 */
export function parseFHIRTime(time: string): {
  hours: number;
  minutes: number;
  seconds: number;
  milliseconds: number;
} {
  const match = time.match(/^(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?$/);
  if (!match) {
    throw new Error(`Invalid FHIR time format: ${time}`);
  }

  const [, hours, minutes, seconds = '0', ms = '0'] = match;
  return {
    hours: parseInt(hours, 10),
    minutes: parseInt(minutes, 10),
    seconds: parseInt(seconds, 10),
    milliseconds: parseInt(ms.padEnd(3, '0').slice(0, 3), 10),
  };
}"#
        .to_string()
    }

    /// Generate FHIR date formatter
    pub fn generate_date_formatter() -> String {
        r#"/**
 * Format a Date object to a FHIR date string (YYYY-MM-DD)
 * @param date - The Date object
 * @returns A FHIR date string
 */
export function toFHIRDate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}"#
        .to_string()
    }

    /// Generate FHIR dateTime formatter
    pub fn generate_datetime_formatter() -> String {
        r#"/**
 * Format a Date object to a FHIR dateTime string with timezone
 * @param date - The Date object
 * @param includeMilliseconds - Whether to include milliseconds (default: true)
 * @returns A FHIR dateTime string
 */
export function toFHIRDateTime(date: Date, includeMilliseconds = true): string {
  const iso = date.toISOString();
  if (!includeMilliseconds) {
    return iso.replace(/\.\d{3}Z$/, 'Z');
  }
  return iso;
}"#
        .to_string()
    }

    /// Generate FHIR instant formatter
    pub fn generate_instant_formatter() -> String {
        r#"/**
 * Format a Date object to a FHIR instant string
 * instant always includes full precision and timezone
 * @param date - The Date object
 * @returns A FHIR instant string
 */
export function toFHIRInstant(date: Date): string {
  return date.toISOString();
}"#
        .to_string()
    }

    /// Generate FHIR time formatter
    pub fn generate_time_formatter() -> String {
        r#"/**
 * Format time components to a FHIR time string
 * @param hours - Hours (0-23)
 * @param minutes - Minutes (0-59)
 * @param seconds - Optional seconds (0-59)
 * @param milliseconds - Optional milliseconds (0-999)
 * @returns A FHIR time string
 */
export function toFHIRTime(
  hours: number,
  minutes: number,
  seconds?: number,
  milliseconds?: number
): string {
  const h = String(hours).padStart(2, '0');
  const m = String(minutes).padStart(2, '0');

  if (seconds === undefined) {
    return `${h}:${m}`;
  }

  const s = String(seconds).padStart(2, '0');

  if (milliseconds === undefined || milliseconds === 0) {
    return `${h}:${m}:${s}`;
  }

  const ms = String(milliseconds).padStart(3, '0');
  return `${h}:${m}:${s}.${ms}`;
}"#
        .to_string()
    }

    /// Generate FHIR date validator
    pub fn generate_date_validator() -> String {
        r#"/**
 * Validate if a string is a valid FHIR date
 * @param value - The string to validate
 * @returns true if valid FHIR date format
 */
export function isValidFHIRDate(value: string): boolean {
  return /^\d{4}(-\d{2}(-\d{2})?)?$/.test(value);
}"#
        .to_string()
    }

    /// Generate FHIR dateTime validator
    pub fn generate_datetime_validator() -> String {
        r#"/**
 * Validate if a string is a valid FHIR dateTime
 * @param value - The string to validate
 * @returns true if valid FHIR dateTime format
 */
export function isValidFHIRDateTime(value: string): boolean {
  const pattern = /^\d{4}(-\d{2}(-\d{2}(T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:\d{2})?)?)?)?$/;
  return pattern.test(value);
}"#
        .to_string()
    }

    /// Generate FHIR instant validator
    pub fn generate_instant_validator() -> String {
        r#"/**
 * Validate if a string is a valid FHIR instant
 * instant requires full precision with timezone
 * @param value - The string to validate
 * @returns true if valid FHIR instant format
 */
export function isValidFHIRInstant(value: string): boolean {
  const pattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;
  return pattern.test(value) && !isNaN(Date.parse(value));
}"#
        .to_string()
    }

    /// Generate FHIR time validator
    pub fn generate_time_validator() -> String {
        r#"/**
 * Validate if a string is a valid FHIR time
 * @param value - The string to validate
 * @returns true if valid FHIR time format
 */
export function isValidFHIRTime(value: string): boolean {
  return /^\d{2}:\d{2}(:\d{2}(\.\d+)?)?$/.test(value);
}"#
        .to_string()
    }

    /// Generate all date helper code
    pub fn generate_all(&self) -> String {
        let mut parts = Vec::new();

        if self.has_parsers {
            parts.push("// Parsing functions".to_string());
            parts.push(Self::generate_date_parser());
            parts.push(Self::generate_datetime_parser());
            parts.push(Self::generate_instant_parser());
            parts.push(Self::generate_time_parser());
        }

        if self.has_formatters {
            parts.push("\n// Formatting functions".to_string());
            parts.push(Self::generate_date_formatter());
            parts.push(Self::generate_datetime_formatter());
            parts.push(Self::generate_instant_formatter());
            parts.push(Self::generate_time_formatter());
        }

        if self.has_validators {
            parts.push("\n// Validation functions".to_string());
            parts.push(Self::generate_date_validator());
            parts.push(Self::generate_datetime_validator());
            parts.push(Self::generate_instant_validator());
            parts.push(Self::generate_time_validator());
        }

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_config_default() {
        let config = DateConfig::default();
        assert!(config.parsers);
        assert!(config.formatters);
        assert!(config.validators);
    }

    #[test]
    fn test_generate_date_parser() {
        let code = DateHelpers::generate_date_parser();
        assert!(code.contains("parseFHIRDate"));
        assert!(code.contains("YYYY-MM-DD"));
    }

    #[test]
    fn test_generate_all() {
        let config = DateConfig::default();
        let helpers = DateHelpers::new(&config);

        let code = helpers.generate_all();

        // Should contain all parser functions
        assert!(code.contains("parseFHIRDate"));
        assert!(code.contains("parseFHIRDateTime"));
        assert!(code.contains("parseFHIRInstant"));
        assert!(code.contains("parseFHIRTime"));

        // Should contain all formatter functions
        assert!(code.contains("toFHIRDate"));
        assert!(code.contains("toFHIRDateTime"));
        assert!(code.contains("toFHIRInstant"));
        assert!(code.contains("toFHIRTime"));

        // Should contain all validator functions
        assert!(code.contains("isValidFHIRDate"));
        assert!(code.contains("isValidFHIRDateTime"));
        assert!(code.contains("isValidFHIRInstant"));
        assert!(code.contains("isValidFHIRTime"));
    }

    #[test]
    fn test_selective_generation() {
        let config = DateConfig {
            parsers: true,
            formatters: false,
            validators: false,
        };
        let helpers = DateHelpers::new(&config);

        let code = helpers.generate_all();

        // Should only contain parsers
        assert!(code.contains("parseFHIRDate"));
        assert!(!code.contains("toFHIRDate"));
        assert!(!code.contains("isValidFHIRDate"));
    }
}
