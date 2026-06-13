//! SearchParameter parsing and extraction from FHIR packages.
//!
//! This module provides types and functions for loading SearchParameter resources
//! from FHIR packages via the canonical manager, making them available for
//! code generation purposes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, CoreResult};

/// Information extracted from a FHIR SearchParameter resource.
///
/// This struct contains the essential information needed to generate
/// search parameter interfaces and utilities in various target languages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchParameterInfo {
    /// The search parameter code (e.g., "name", "patient", "identifier")
    pub code: String,

    /// Resource types this parameter applies to
    pub base: Vec<String>,

    /// The search parameter type (string, date, token, reference, number, etc.)
    #[serde(rename = "type")]
    pub param_type: String,

    /// Human-readable description of the parameter
    pub description: String,

    /// FHIRPath expression defining where to find the value (optional)
    pub expression: Option<String>,

    /// For reference-type parameters: the allowed target resource types
    #[serde(default)]
    pub target: Vec<String>,

    /// The canonical URL of the SearchParameter resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl SearchParameterInfo {
    /// Parse a SearchParameter resource from JSON.
    ///
    /// # Arguments
    ///
    /// * `content` - The JSON content of a SearchParameter resource
    ///
    /// # Returns
    ///
    /// Returns `Ok(SearchParameterInfo)` if parsing succeeds, or an error if
    /// required fields are missing or invalid.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let json = serde_json::json!({
    ///     "resourceType": "SearchParameter",
    ///     "code": "name",
    ///     "base": ["Patient"],
    ///     "type": "string",
    ///     "description": "A server defined search that may match any of the string fields..."
    /// });
    /// let info = SearchParameterInfo::from_json(&json)?;
    /// assert_eq!(info.code, "name");
    /// ```
    pub fn from_json(content: &Value) -> CoreResult<Self> {
        // Verify it's a SearchParameter resource
        let resource_type = content
            .get("resourceType")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::validation("Missing or invalid resourceType field"))?;

        if resource_type != "SearchParameter" {
            return Err(CoreError::validation(format!(
                "Expected SearchParameter, got {}",
                resource_type
            )));
        }

        // Extract required fields
        let code = content
            .get("code")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::validation("Missing 'code' field in SearchParameter"))?
            .to_string();

        let base = content
            .get("base")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .ok_or_else(|| {
                CoreError::validation("Missing or invalid 'base' field in SearchParameter")
            })?;

        let param_type = content
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::validation("Missing 'type' field in SearchParameter"))?
            .to_string();

        let description = content
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("No description available")
            .to_string();

        // Extract optional fields
        let expression = content
            .get("expression")
            .and_then(Value::as_str)
            .map(String::from);

        let target = content
            .get("target")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let url = content.get("url").and_then(Value::as_str).map(String::from);

        Ok(Self {
            code,
            base,
            param_type,
            description,
            expression,
            target,
            url,
        })
    }

    /// Get a display name for this search parameter.
    ///
    /// Returns the code with underscores replaced by spaces and title-cased.
    pub fn display_name(&self) -> String {
        self.code.replace(['-', '_'], " ")
    }

    /// Check if this parameter applies to a specific resource type.
    pub fn applies_to(&self, resource_type: &str) -> bool {
        self.base.iter().any(|b| b == resource_type)
    }

    /// Check if this is a reference-type parameter.
    pub fn is_reference(&self) -> bool {
        self.param_type == "reference"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_patient_name_search_param() {
        let json = json!({
            "resourceType": "SearchParameter",
            "id": "Patient-name",
            "url": "http://hl7.org/fhir/SearchParameter/Patient-name",
            "code": "name",
            "base": ["Patient"],
            "type": "string",
            "description": "A server defined search that may match any of the string fields in the HumanName",
            "expression": "Patient.name"
        });

        let info = SearchParameterInfo::from_json(&json).unwrap();
        assert_eq!(info.code, "name");
        assert_eq!(info.base, vec!["Patient"]);
        assert_eq!(info.param_type, "string");
        assert!(info.description.contains("HumanName"));
        assert_eq!(info.expression, Some("Patient.name".to_string()));
        assert_eq!(info.target, Vec::<String>::new());
    }

    #[test]
    fn test_parse_observation_subject_reference_param() {
        let json = json!({
            "resourceType": "SearchParameter",
            "code": "subject",
            "base": ["Observation"],
            "type": "reference",
            "description": "The subject that the observation is about",
            "target": ["Patient", "Group", "Device", "Location"]
        });

        let info = SearchParameterInfo::from_json(&json).unwrap();
        assert_eq!(info.code, "subject");
        assert_eq!(info.base, vec!["Observation"]);
        assert_eq!(info.param_type, "reference");
        assert!(info.is_reference());
        assert_eq!(info.target, vec!["Patient", "Group", "Device", "Location"]);
    }

    #[test]
    fn test_parse_multi_base_search_param() {
        let json = json!({
            "resourceType": "SearchParameter",
            "code": "identifier",
            "base": ["Patient", "Practitioner", "Organization"],
            "type": "token",
            "description": "A patient identifier"
        });

        let info = SearchParameterInfo::from_json(&json).unwrap();
        assert_eq!(info.code, "identifier");
        assert_eq!(info.base, vec!["Patient", "Practitioner", "Organization"]);
        assert_eq!(info.param_type, "token");
        assert!(info.applies_to("Patient"));
        assert!(info.applies_to("Practitioner"));
        assert!(info.applies_to("Organization"));
        assert!(!info.applies_to("Observation"));
    }

    #[test]
    fn test_missing_required_field() {
        let json = json!({
            "resourceType": "SearchParameter",
            "base": ["Patient"],
            "type": "string"
            // Missing 'code' field
        });

        let result = SearchParameterInfo::from_json(&json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Missing 'code' field")
        );
    }

    #[test]
    fn test_wrong_resource_type() {
        let json = json!({
            "resourceType": "Patient",
            "code": "name",
            "base": ["Patient"],
            "type": "string"
        });

        let result = SearchParameterInfo::from_json(&json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Expected SearchParameter")
        );
    }

    #[test]
    fn test_display_name() {
        let json = json!({
            "resourceType": "SearchParameter",
            "code": "address-city",
            "base": ["Patient"],
            "type": "string",
            "description": "City"
        });

        let info = SearchParameterInfo::from_json(&json).unwrap();
        assert_eq!(info.display_name(), "address city");
    }
}
