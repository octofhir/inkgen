//! Terminology and value set code extraction.
//!
//! This module provides functionality for:
//! - Extracting concept codes from FHIR ValueSet resources
//! - Caching resolved value sets by canonical URL
//! - Supporting configurable size limits for code extraction

use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{CoreError, CoreResult};

/// Represents a resolved value set with extracted codes.
#[derive(Debug, Clone)]
pub struct ResolvedValueSet {
    /// Canonical URL of the value set
    pub url: String,
    /// Extracted concept codes
    pub codes: Vec<String>,
    /// Display names for codes (if available)
    pub displays: IndexMap<String, String>,
    /// Title of the value set
    pub title: Option<String>,
    /// Description of the value set
    pub description: Option<String>,
}

/// Cache for resolved value sets to avoid repeated parsing.
#[derive(Debug, Clone, Default)]
pub struct ValueSetCache {
    cache: Arc<Mutex<HashMap<String, ResolvedValueSet>>>,
}

impl ValueSetCache {
    /// Creates a new empty value set cache.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Gets a cached value set by URL, if present.
    pub fn get(&self, url: &str) -> Option<ResolvedValueSet> {
        self.cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(url).cloned())
    }

    /// Inserts a resolved value set into the cache.
    pub fn insert(&self, url: String, value_set: ResolvedValueSet) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(url, value_set);
        }
    }

    /// Clears all cached value sets.
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Returns the number of cached value sets.
    pub fn len(&self) -> usize {
        self.cache.lock().map(|cache| cache.len()).unwrap_or(0)
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Extracts concept codes from a FHIR ValueSet resource.
///
/// This function supports multiple ValueSet structures:
/// - expansion.contains (preferred)
/// - compose.include with concept lists
/// - compose.include with complete system references
///
/// # Arguments
///
/// * `value_set_json` - The ValueSet resource as JSON
/// * `max_codes` - Optional maximum number of codes to extract
///
/// # Returns
///
/// Result containing the resolved value set with extracted codes
pub fn extract_codes_from_valueset(
    value_set_json: &Value,
    max_codes: Option<usize>,
) -> CoreResult<ResolvedValueSet> {
    let url = value_set_json
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| CoreError::validation("ValueSet missing url"))?
        .to_string();

    let title = value_set_json
        .get("title")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let description = value_set_json
        .get("description")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let mut codes = Vec::new();
    let mut displays = IndexMap::new();

    // Try expansion first (most reliable)
    if let Some(expansion) = value_set_json.get("expansion") {
        extract_from_expansion(expansion, &mut codes, &mut displays, max_codes)?;
    }

    // If no codes from expansion, try compose
    if codes.is_empty() {
        if let Some(compose) = value_set_json.get("compose") {
            extract_from_compose(compose, &mut codes, &mut displays, max_codes)?;
        }
    }

    Ok(ResolvedValueSet {
        url,
        codes,
        displays,
        title,
        description,
    })
}

/// Extracts codes from ValueSet expansion.contains array.
fn extract_from_expansion(
    expansion: &Value,
    codes: &mut Vec<String>,
    displays: &mut IndexMap<String, String>,
    max_codes: Option<usize>,
) -> CoreResult<()> {
    if let Some(contains) = expansion.get("contains").and_then(Value::as_array) {
        for item in contains {
            if let Some(max) = max_codes {
                if codes.len() >= max {
                    break;
                }
            }

            if let Some(code) = item.get("code").and_then(Value::as_str) {
                let code_str = code.to_string();
                codes.push(code_str.clone());

                if let Some(display) = item.get("display").and_then(Value::as_str) {
                    displays.insert(code_str, display.to_string());
                }
            }
        }
    }

    Ok(())
}

/// Extracts codes from ValueSet compose.include array.
fn extract_from_compose(
    compose: &Value,
    codes: &mut Vec<String>,
    displays: &mut IndexMap<String, String>,
    max_codes: Option<usize>,
) -> CoreResult<()> {
    if let Some(include) = compose.get("include").and_then(Value::as_array) {
        for include_item in include {
            if let Some(max) = max_codes {
                if codes.len() >= max {
                    break;
                }
            }

            // Try to get concepts from include item
            if let Some(concepts) = include_item.get("concept").and_then(Value::as_array) {
                for concept in concepts {
                    if let Some(max) = max_codes {
                        if codes.len() >= max {
                            break;
                        }
                    }

                    if let Some(code) = concept.get("code").and_then(Value::as_str) {
                        let code_str = code.to_string();
                        codes.push(code_str.clone());

                        if let Some(display) = concept.get("display").and_then(Value::as_str) {
                            displays.insert(code_str, display.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Determines if a value set should be generated based on size and configuration.
///
/// # Arguments
///
/// * `code_count` - Number of codes in the value set
/// * `max_size` - Maximum allowed size for generation
///
/// # Returns
///
/// true if the value set should be generated as a type union, false otherwise
pub fn should_generate_valueset(code_count: usize, max_size: usize) -> bool {
    code_count > 0 && code_count <= max_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_from_expansion() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://example.org/fhir/ValueSet/test",
            "title": "Test Value Set",
            "expansion": {
                "contains": [
                    {
                        "code": "active",
                        "display": "Active"
                    },
                    {
                        "code": "inactive",
                        "display": "Inactive"
                    },
                    {
                        "code": "pending"
                    }
                ]
            }
        });

        let result = extract_codes_from_valueset(&valueset, None).unwrap();

        assert_eq!(result.url, "http://example.org/fhir/ValueSet/test");
        assert_eq!(result.title, Some("Test Value Set".to_string()));
        assert_eq!(result.codes.len(), 3);
        assert!(result.codes.contains(&"active".to_string()));
        assert!(result.codes.contains(&"inactive".to_string()));
        assert!(result.codes.contains(&"pending".to_string()));
        assert_eq!(result.displays.get("active"), Some(&"Active".to_string()));
        assert_eq!(
            result.displays.get("inactive"),
            Some(&"Inactive".to_string())
        );
    }

    #[test]
    fn test_extract_from_compose() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://example.org/fhir/ValueSet/test",
            "compose": {
                "include": [
                    {
                        "system": "http://example.org/codes",
                        "concept": [
                            {
                                "code": "code1",
                                "display": "Code 1"
                            },
                            {
                                "code": "code2",
                                "display": "Code 2"
                            }
                        ]
                    }
                ]
            }
        });

        let result = extract_codes_from_valueset(&valueset, None).unwrap();

        assert_eq!(result.codes.len(), 2);
        assert!(result.codes.contains(&"code1".to_string()));
        assert!(result.codes.contains(&"code2".to_string()));
        assert_eq!(result.displays.get("code1"), Some(&"Code 1".to_string()));
    }

    #[test]
    fn test_max_codes_limit() {
        let valueset = json!({
            "resourceType": "ValueSet",
            "url": "http://example.org/fhir/ValueSet/test",
            "expansion": {
                "contains": [
                    {"code": "code1"},
                    {"code": "code2"},
                    {"code": "code3"},
                    {"code": "code4"},
                    {"code": "code5"}
                ]
            }
        });

        let result = extract_codes_from_valueset(&valueset, Some(3)).unwrap();

        assert_eq!(result.codes.len(), 3);
        assert!(result.codes.contains(&"code1".to_string()));
        assert!(result.codes.contains(&"code2".to_string()));
        assert!(result.codes.contains(&"code3".to_string()));
    }

    #[test]
    fn test_should_generate_valueset() {
        assert!(should_generate_valueset(5, 10));
        assert!(should_generate_valueset(10, 10));
        assert!(!should_generate_valueset(11, 10));
        assert!(!should_generate_valueset(0, 10));
        assert!(should_generate_valueset(1, 50));
        assert!(!should_generate_valueset(51, 50));
    }

    #[test]
    fn test_valueset_cache() {
        let cache = ValueSetCache::new();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        let vs = ResolvedValueSet {
            url: "http://example.org/test".to_string(),
            codes: vec!["a".to_string(), "b".to_string()],
            displays: IndexMap::new(),
            title: Some("Test".to_string()),
            description: None,
        };

        cache.insert("http://example.org/test".to_string(), vs.clone());

        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        let retrieved = cache.get("http://example.org/test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().codes.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }
}
