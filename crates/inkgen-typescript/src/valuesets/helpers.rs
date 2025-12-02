//! Helper function generation for Coding and CodeableConcept creation.
//!
//! This module generates TypeScript factory functions that simplify working
//! with FHIR Coding and CodeableConcept structures in ValueSets.

use serde::Serialize;

/// Configuration for Coding/CodeableConcept helper generation
#[derive(Debug, Clone)]
pub struct HelperConfig {
    /// Generate Coding factory functions
    pub coding_factories: bool,
    /// Generate CodeableConcept factory functions
    pub codeable_concept_factories: bool,
    /// Generate validation helpers
    pub validation_helpers: bool,
    /// Generate extraction helpers
    pub extraction_helpers: bool,
}

impl Default for HelperConfig {
    fn default() -> Self {
        Self {
            coding_factories: true,
            codeable_concept_factories: true,
            validation_helpers: true,
            extraction_helpers: true,
        }
    }
}

/// Information needed to generate helper functions for a ValueSet
#[derive(Debug, Clone, Serialize)]
pub struct ValueSetHelpers {
    /// ValueSet type name (e.g., "AdministrativeGender")
    pub type_name: String,
    /// CodeSystem URL
    pub system_url: String,
    /// Whether to generate Coding factories
    pub has_coding_factory: bool,
    /// Whether to generate CodeableConcept factories
    pub has_codeable_concept_factory: bool,
    /// Whether to generate validation helpers
    pub has_validation: bool,
    /// Whether to generate extraction helpers
    pub has_extraction: bool,
}

impl ValueSetHelpers {
    /// Creates helper information for a ValueSet
    ///
    /// # Arguments
    /// * `type_name` - The TypeScript type name for the ValueSet
    /// * `system_url` - The CodeSystem URL
    /// * `config` - Configuration for which helpers to generate
    pub fn new(type_name: String, system_url: String, config: &HelperConfig) -> Self {
        Self {
            type_name,
            system_url,
            has_coding_factory: config.coding_factories,
            has_codeable_concept_factory: config.codeable_concept_factories,
            has_validation: config.validation_helpers,
            has_extraction: config.extraction_helpers,
        }
    }

    /// Generate TypeScript code for Coding factory function
    ///
    /// Creates a function like:
    /// ```typescript
    /// export function createAdministrativeGenderCoding(
    ///   code: AdministrativeGender,
    ///   display?: string
    /// ): Coding {
    ///   const meta = AdministrativeGenderMetadata.codes[code];
    ///   return {
    ///     system: "http://hl7.org/fhir/administrative-gender",
    ///     code,
    ///     display: display ?? meta?.display,
    ///   };
    /// }
    /// ```
    pub fn coding_factory_code(&self) -> String {
        format!(
            r#"/**
 * Create a Coding for {} ValueSet
 * @param code - The code value
 * @param display - Optional display text (uses metadata if not provided)
 * @returns A Coding object
 */
export function create{}Coding(
  code: {},
  display?: string
): Coding {{
  const meta = {}Metadata.codes[code];
  return {{
    system: "{}",
    code,
    display: display ?? meta?.display,
  }};
}}"#,
            self.type_name,
            self.type_name,
            self.type_name,
            self.type_name,
            self.system_url
        )
    }

    /// Generate TypeScript code for CodeableConcept factory function
    ///
    /// Creates a function like:
    /// ```typescript
    /// export function createAdministrativeGenderCodeableConcept(
    ///   code: AdministrativeGender,
    ///   text?: string
    /// ): CodeableConcept {
    ///   return {
    ///     coding: [createAdministrativeGenderCoding(code)],
    ///     text,
    ///   };
    /// }
    /// ```
    pub fn codeable_concept_factory_code(&self) -> String {
        format!(
            r#"/**
 * Create a CodeableConcept for {} ValueSet
 * @param code - The code value
 * @param text - Optional text description
 * @returns A CodeableConcept object
 */
export function create{}CodeableConcept(
  code: {},
  text?: string
): CodeableConcept {{
  return {{
    coding: [create{}Coding(code)],
    text,
  }};
}}"#,
            self.type_name, self.type_name, self.type_name, self.type_name
        )
    }

    /// Generate TypeScript code for Coding validation helper
    ///
    /// Creates a function like:
    /// ```typescript
    /// export function isValidAdministrativeGenderCoding(coding: Coding): boolean {
    ///   return (
    ///     coding.system === "http://hl7.org/fhir/administrative-gender" &&
    ///     !!coding.code &&
    ///     isAdministrativeGender(coding.code)
    ///   );
    /// }
    /// ```
    pub fn validation_helper_code(&self) -> String {
        format!(
            r#"/**
 * Validate if a Coding belongs to the {} ValueSet
 * @param coding - The Coding to validate
 * @returns true if the Coding is valid for this ValueSet
 */
export function isValid{}Coding(coding: Coding): boolean {{
  return (
    coding.system === "{}" &&
    !!coding.code &&
    is{}(coding.code)
  );
}}"#,
            self.type_name, self.type_name, self.system_url, self.type_name
        )
    }

    /// Generate TypeScript code for code extraction helper
    ///
    /// Creates a function like:
    /// ```typescript
    /// export function extractAdministrativeGender(
    ///   concept: CodeableConcept
    /// ): AdministrativeGender | undefined {
    ///   const coding = concept.coding?.find(c =>
    ///     c.system === "http://hl7.org/fhir/administrative-gender"
    ///   );
    ///   return coding?.code && isAdministrativeGender(coding.code)
    ///     ? coding.code
    ///     : undefined;
    /// }
    /// ```
    pub fn extraction_helper_code(&self) -> String {
        format!(
            r#"/**
 * Extract {} code from a CodeableConcept
 * @param concept - The CodeableConcept to extract from
 * @returns The code if found and valid, undefined otherwise
 */
export function extract{}(
  concept: CodeableConcept
): {} | undefined {{
  const coding = concept.coding?.find(c =>
    c.system === "{}"
  );
  return coding?.code && is{}(coding.code)
    ? coding.code
    : undefined;
}}"#,
            self.type_name,
            self.type_name,
            self.type_name,
            self.system_url,
            self.type_name
        )
    }

    /// Generate all helper functions based on configuration
    pub fn generate_all_helpers(&self) -> String {
        let mut parts = Vec::new();

        if self.has_coding_factory {
            parts.push(self.coding_factory_code());
        }

        if self.has_codeable_concept_factory {
            parts.push(self.codeable_concept_factory_code());
        }

        if self.has_validation {
            parts.push(self.validation_helper_code());
        }

        if self.has_extraction {
            parts.push(self.extraction_helper_code());
        }

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helper_config_default() {
        let config = HelperConfig::default();

        assert!(config.coding_factories);
        assert!(config.codeable_concept_factories);
        assert!(config.validation_helpers);
        assert!(config.extraction_helpers);
    }

    #[test]
    fn test_valueset_helpers_creation() {
        let config = HelperConfig::default();
        let helpers = ValueSetHelpers::new(
            "AdministrativeGender".to_string(),
            "http://hl7.org/fhir/administrative-gender".to_string(),
            &config,
        );

        assert_eq!(helpers.type_name, "AdministrativeGender");
        assert_eq!(
            helpers.system_url,
            "http://hl7.org/fhir/administrative-gender"
        );
        assert!(helpers.has_coding_factory);
        assert!(helpers.has_codeable_concept_factory);
    }

    #[test]
    fn test_coding_factory_generation() {
        let config = HelperConfig::default();
        let helpers = ValueSetHelpers::new(
            "AdministrativeGender".to_string(),
            "http://hl7.org/fhir/administrative-gender".to_string(),
            &config,
        );

        let code = helpers.coding_factory_code();

        assert!(code.contains("createAdministrativeGenderCoding"));
        assert!(code.contains("code: AdministrativeGender"));
        assert!(code.contains("Coding"));
        assert!(code.contains("http://hl7.org/fhir/administrative-gender"));
    }

    #[test]
    fn test_codeable_concept_factory_generation() {
        let config = HelperConfig::default();
        let helpers = ValueSetHelpers::new(
            "AdministrativeGender".to_string(),
            "http://hl7.org/fhir/administrative-gender".to_string(),
            &config,
        );

        let code = helpers.codeable_concept_factory_code();

        assert!(code.contains("createAdministrativeGenderCodeableConcept"));
        assert!(code.contains("CodeableConcept"));
        assert!(code.contains("createAdministrativeGenderCoding"));
    }

    #[test]
    fn test_validation_helper_generation() {
        let config = HelperConfig::default();
        let helpers = ValueSetHelpers::new(
            "AdministrativeGender".to_string(),
            "http://hl7.org/fhir/administrative-gender".to_string(),
            &config,
        );

        let code = helpers.validation_helper_code();

        assert!(code.contains("isValidAdministrativeGenderCoding"));
        assert!(code.contains("coding.system"));
        assert!(code.contains("isAdministrativeGender"));
    }

    #[test]
    fn test_extraction_helper_generation() {
        let config = HelperConfig::default();
        let helpers = ValueSetHelpers::new(
            "AdministrativeGender".to_string(),
            "http://hl7.org/fhir/administrative-gender".to_string(),
            &config,
        );

        let code = helpers.extraction_helper_code();

        assert!(code.contains("extractAdministrativeGender"));
        assert!(code.contains("CodeableConcept"));
        assert!(code.contains("concept.coding?.find"));
    }

    #[test]
    fn test_generate_all_helpers() {
        let config = HelperConfig::default();
        let helpers = ValueSetHelpers::new(
            "TestValueSet".to_string(),
            "http://example.org/test".to_string(),
            &config,
        );

        let all_code = helpers.generate_all_helpers();

        // Should contain all helper types
        assert!(all_code.contains("createTestValueSetCoding"));
        assert!(all_code.contains("createTestValueSetCodeableConcept"));
        assert!(all_code.contains("isValidTestValueSetCoding"));
        assert!(all_code.contains("extractTestValueSet"));
    }

    #[test]
    fn test_selective_helper_generation() {
        let config = HelperConfig {
            coding_factories: true,
            codeable_concept_factories: false,
            validation_helpers: false,
            extraction_helpers: false,
        };

        let helpers = ValueSetHelpers::new(
            "TestValueSet".to_string(),
            "http://example.org/test".to_string(),
            &config,
        );

        let code = helpers.generate_all_helpers();

        // Should only contain Coding factory
        assert!(code.contains("createTestValueSetCoding"));
        assert!(!code.contains("createTestValueSetCodeableConcept"));
        assert!(!code.contains("isValidTestValueSetCoding"));
        assert!(!code.contains("extractTestValueSet"));
    }
}
