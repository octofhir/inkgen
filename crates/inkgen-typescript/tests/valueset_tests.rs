//! Integration tests for ValueSet generation
//!
//! These tests verify the complete ValueSet generation workflow including
//! code extraction, TypeScript generation, and binding strength handling.

use inkgen_typescript::valuesets::{BindingStrength, CodeInfo, ValueSetInfo};
use insta::assert_snapshot;
use serde_json::json;

/// Helper function to create a test ValueSet with specified codes
fn create_test_valueset(codes: Vec<(&str, Option<&str>)>) -> ValueSetInfo {
    ValueSetInfo {
        type_name: "TestStatus".to_string(),
        canonical_url: "http://example.org/ValueSet/test-status".to_string(),
        code_info: codes
            .into_iter()
            .map(|(code, display)| CodeInfo {
                code: code.to_string(),
                display: display.map(|s| s.to_string()),
                definition: None,
            })
            .collect(),
        title: Some("Test Status".to_string()),
        description: Some("A test value set for integration testing".to_string()),
        binding_strength: Some(BindingStrength::Required),
    }
}

#[test]
fn test_valueset_code_generation() {
    let vs = create_test_valueset(vec![
        ("active", Some("Active")),
        ("inactive", Some("Inactive")),
        ("pending", Some("Pending")),
    ]);

    let generated = vs.generate_typescript();

    // Check const array declaration
    assert!(
        generated.contains("export const TestStatusValues = ["),
        "Should generate const array"
    );
    assert!(
        generated.contains(r#""active", "inactive", "pending""#),
        "Should include all codes"
    );
    assert!(generated.contains("as const"), "Should use 'as const'");

    // Check type alias
    assert!(
        generated.contains("export type TestStatus = typeof TestStatusValues[number]"),
        "Should generate type alias"
    );

    // Check type guard function
    assert!(
        generated.contains("export function isTestStatus(value: string): value is TestStatus"),
        "Should generate type guard function"
    );
    assert!(
        generated.contains("return (TestStatusValues as readonly string[]).includes(value)"),
        "Type guard should check array membership"
    );

    // Check code definitions object (only generated when display/definition exists)
    assert!(
        generated.contains("export const TestStatusDefinitions = {"),
        "Should generate definitions object when displays are present"
    );
    assert!(
        generated.contains(r#""active": {"#),
        "Should include active definition"
    );
    assert!(
        generated.contains(r#"code: "active""#),
        "Should include code field"
    );
    assert!(
        generated.contains(r#"display: "Active""#),
        "Should include display field"
    );

    // Should be closed (Required binding) - no string & {}
    assert!(
        !generated.contains("string & {}"),
        "Required binding should generate closed type"
    );
}

#[test]
fn test_required_binding_generates_closed_type() {
    let vs = ValueSetInfo {
        binding_strength: Some(BindingStrength::Required),
        ..create_test_valueset(vec![("code1", None), ("code2", None)])
    };

    let generated = vs.generate_typescript();

    // Should generate closed type
    assert!(
        generated.contains("export type TestStatus = typeof TestStatusValues[number];"),
        "Required binding should generate closed type without string extension"
    );
    assert!(
        !generated.contains("| (string & {})"),
        "Required binding should not allow custom codes"
    );
    assert!(vs.is_closed(), "Required binding should be closed");
}

#[test]
fn test_extensible_binding_generates_closed_type() {
    let vs = ValueSetInfo {
        binding_strength: Some(BindingStrength::Extensible),
        ..create_test_valueset(vec![("code1", None), ("code2", None)])
    };

    let generated = vs.generate_typescript();

    // Extensible is also closed (only the specified codes)
    assert!(
        generated.contains("export type TestStatus = typeof TestStatusValues[number];"),
        "Extensible binding should generate closed type"
    );
    assert!(
        !generated.contains("| (string & {})"),
        "Extensible binding should not allow custom codes in TypeScript"
    );
    assert!(vs.is_closed(), "Extensible binding should be closed");
}

#[test]
fn test_preferred_binding_allows_custom_codes() {
    let vs = ValueSetInfo {
        binding_strength: Some(BindingStrength::Preferred),
        ..create_test_valueset(vec![("code1", None), ("code2", None)])
    };

    let generated = vs.generate_typescript();

    // Should allow custom codes
    assert!(
        generated.contains("| (string & {})"),
        "Preferred binding should allow custom codes"
    );
    assert!(
        generated.contains("// Open valueset - allows custom codes"),
        "Should include comment about open valueset"
    );
    assert!(!vs.is_closed(), "Preferred binding should be open");
}

#[test]
fn test_example_binding_allows_custom_codes() {
    let vs = ValueSetInfo {
        binding_strength: Some(BindingStrength::Example),
        ..create_test_valueset(vec![("code1", None), ("code2", None)])
    };

    let generated = vs.generate_typescript();

    // Should allow custom codes
    assert!(
        generated.contains("| (string & {})"),
        "Example binding should allow custom codes"
    );
    assert!(!vs.is_closed(), "Example binding should be open");
}

#[test]
fn test_no_binding_strength_generates_open_type() {
    let vs = ValueSetInfo {
        binding_strength: None,
        ..create_test_valueset(vec![("code1", None), ("code2", None)])
    };

    let generated = vs.generate_typescript();

    // Without binding strength, generates open type (allows custom codes)
    assert!(
        generated.contains("| (string & {})"),
        "No binding strength should generate open type (allows custom codes)"
    );
    assert!(
        generated.contains("// Open valueset - allows custom codes"),
        "Should include comment about open valueset"
    );
    assert!(
        !vs.is_closed(),
        "No binding strength means is_closed() returns false"
    );
}

#[test]
fn test_large_valueset_handling() {
    // Create a large valueset with 100 codes
    let large_codes: Vec<_> = (0..100)
        .map(|i| (format!("code{}", i), Some(format!("Code {}", i))))
        .collect();

    let valueset_json = json!({
        "resourceType": "ValueSet",
        "url": "http://example.org/ValueSet/large",
        "title": "Large ValueSet",
        "expansion": {
            "contains": large_codes.iter().map(|(code, display)| {
                json!({"code": code, "display": display})
            }).collect::<Vec<_>>()
        }
    });

    // With max_codes=50, should return None (skipped)
    let result =
        ValueSetInfo::from_valueset(&valueset_json, "Large".to_string(), Some(50)).unwrap();
    assert!(
        result.is_none(),
        "ValueSet with 100 codes should be skipped when max is 50"
    );

    // With max_codes=150, should return Some
    let result =
        ValueSetInfo::from_valueset(&valueset_json, "Large".to_string(), Some(150)).unwrap();
    assert!(
        result.is_some(),
        "ValueSet with 100 codes should be included when max is 150"
    );
    assert_eq!(result.unwrap().code_info.len(), 100);

    // With no max, should return Some
    let result = ValueSetInfo::from_valueset(&valueset_json, "Large".to_string(), None).unwrap();
    assert!(
        result.is_some(),
        "ValueSet should be included when no max is specified"
    );
}

#[test]
fn test_valueset_from_json_extraction() {
    let valueset = json!({
        "resourceType": "ValueSet",
        "url": "http://hl7.org/fhir/ValueSet/administrative-gender",
        "title": "Administrative Gender",
        "description": "The gender of a person used for administrative purposes",
        "expansion": {
            "contains": [
                {"code": "male", "display": "Male"},
                {"code": "female", "display": "Female"},
                {"code": "other", "display": "Other"},
                {"code": "unknown", "display": "Unknown"}
            ]
        }
    });

    let info = ValueSetInfo::from_valueset(&valueset, "AdministrativeGender".to_string(), None)
        .unwrap()
        .unwrap();

    assert_eq!(info.type_name, "AdministrativeGender");
    assert_eq!(
        info.canonical_url,
        "http://hl7.org/fhir/ValueSet/administrative-gender"
    );
    assert_eq!(info.title, Some("Administrative Gender".to_string()));
    assert_eq!(
        info.description,
        Some("The gender of a person used for administrative purposes".to_string())
    );
    assert_eq!(info.code_info.len(), 4);

    // Check codes are extracted correctly
    let codes: Vec<&str> = info.code_info.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(codes, vec!["male", "female", "other", "unknown"]);

    // Check displays are extracted correctly
    assert_eq!(info.code_info[0].display, Some("Male".to_string()));
    assert_eq!(info.code_info[1].display, Some("Female".to_string()));
}

#[test]
fn test_valueset_with_special_characters() {
    let vs = create_test_valueset(vec![
        ("code\"with\"quotes", Some("Display with \"quotes\"")),
        ("code\\with\\backslash", Some("Display\\Backslash")),
        ("code\nwith\nnewline", Some("Display\nNewline")),
    ]);

    let generated = vs.generate_typescript();

    // Check proper escaping in code array
    assert!(
        generated.contains(r#""code\"with\"quotes""#),
        "Should escape quotes in codes"
    );
    assert!(
        generated.contains(r#""code\\with\\backslash""#),
        "Should escape backslashes in codes"
    );
    assert!(
        generated.contains(r#""code\nwith\nnewline""#),
        "Should escape newlines in codes"
    );

    // Check proper escaping in definitions
    assert!(
        generated.contains(r#"display: "Display with \"quotes\"""#),
        "Should escape quotes in displays"
    );
}

#[test]
fn test_valueset_jsdoc_generation() {
    let vs = ValueSetInfo {
        type_name: "TestStatus".to_string(),
        canonical_url: "http://example.org/ValueSet/test".to_string(),
        code_info: vec![],
        title: Some("Test Status ValueSet".to_string()),
        description: Some("A comprehensive description of the test status value set".to_string()),
        binding_strength: None,
    };

    let generated = vs.generate_typescript();

    // Check JSDoc comment is generated
    assert!(generated.contains("/**"), "Should start JSDoc comment");
    assert!(
        generated.contains(" * Test Status ValueSet"),
        "Should include title in JSDoc"
    );
    assert!(
        generated.contains(" * A comprehensive description of the test status value set"),
        "Should include description in JSDoc"
    );
    assert!(
        generated.contains(" * @see http://example.org/ValueSet/test"),
        "Should include @see tag with URL"
    );
    assert!(generated.contains(" */"), "Should close JSDoc comment");
}

#[test]
fn test_empty_valueset() {
    let valueset = json!({
        "resourceType": "ValueSet",
        "url": "http://example.org/ValueSet/empty",
        "expansion": {
            "contains": []
        }
    });

    let result = ValueSetInfo::from_valueset(&valueset, "Empty".to_string(), None).unwrap();
    assert!(result.is_none(), "Empty ValueSet should return None");
}

#[test]
fn test_valueset_with_only_codes_no_displays() {
    let valueset = json!({
        "resourceType": "ValueSet",
        "url": "http://example.org/ValueSet/codes-only",
        "expansion": {
            "contains": [
                {"code": "code1"},
                {"code": "code2"},
                {"code": "code3"}
            ]
        }
    });

    let info = ValueSetInfo::from_valueset(&valueset, "CodesOnly".to_string(), None)
        .unwrap()
        .unwrap();

    assert_eq!(info.code_info.len(), 3);
    assert_eq!(info.code_info[0].code, "code1");
    assert_eq!(info.code_info[0].display, None);

    // Generation should still work without displays
    let generated = info.generate_typescript();
    assert!(generated.contains(r#""code1", "code2", "code3""#));
}

#[test]
fn test_type_guard_function_generation() {
    let vs = create_test_valueset(vec![("active", None), ("inactive", None)]);
    let generated = vs.generate_typescript();

    // Check type guard function structure
    assert!(
        generated.contains("export function isTestStatus(value: string): value is TestStatus {"),
        "Should generate type guard function signature"
    );
    assert!(
        generated.contains("return (TestStatusValues as readonly string[]).includes(value);"),
        "Should use includes() for type checking"
    );
    assert!(generated.contains("}"), "Should close function");
}

#[test]
fn test_code_definitions_object_generation() {
    let vs = create_test_valueset(vec![
        ("active", Some("Active Status")),
        ("inactive", Some("Inactive Status")),
    ]);
    let generated = vs.generate_typescript();

    // Check definitions object
    assert!(
        generated.contains("export const TestStatusDefinitions = {"),
        "Should start definitions object"
    );
    assert!(
        generated.contains(r#""active": {"#),
        "Should include active key"
    );
    assert!(
        generated.contains(r#"code: "active""#),
        "Should include active code"
    );
    assert!(
        generated.contains(r#"display: "Active Status""#),
        "Should include active display"
    );
    assert!(
        generated.contains(r#""inactive": {"#),
        "Should include inactive key"
    );
    assert!(
        generated.contains(r#"code: "inactive""#),
        "Should include inactive code"
    );
    assert!(
        generated.contains(r#"display: "Inactive Status""#),
        "Should include inactive display"
    );
    assert!(
        generated.contains("} as const;"),
        "Should close with 'as const'"
    );
}

// Snapshot tests for deterministic output verification

#[test]
fn test_valueset_snapshot_required_binding() {
    let vs = ValueSetInfo {
        type_name: "AccountStatus".to_string(),
        canonical_url: "http://hl7.org/fhir/ValueSet/account-status".to_string(),
        code_info: vec![
            CodeInfo {
                code: "active".to_string(),
                display: Some("Active".to_string()),
                definition: Some("The account is active and in use".to_string()),
            },
            CodeInfo {
                code: "inactive".to_string(),
                display: Some("Inactive".to_string()),
                definition: Some("The account is not active".to_string()),
            },
            CodeInfo {
                code: "entered-in-error".to_string(),
                display: Some("Entered in Error".to_string()),
                definition: None,
            },
        ],
        title: Some("Account Status".to_string()),
        description: Some("Indicates whether the account is available to be used".to_string()),
        binding_strength: Some(BindingStrength::Required),
    };

    let generated = vs.generate_typescript();
    assert_snapshot!("valueset_required_binding", generated);
}

#[test]
fn test_valueset_snapshot_preferred_binding() {
    let vs = ValueSetInfo {
        type_name: "ConditionSeverity".to_string(),
        canonical_url: "http://hl7.org/fhir/ValueSet/condition-severity".to_string(),
        code_info: vec![
            CodeInfo {
                code: "mild".to_string(),
                display: Some("Mild".to_string()),
                definition: None,
            },
            CodeInfo {
                code: "moderate".to_string(),
                display: Some("Moderate".to_string()),
                definition: None,
            },
            CodeInfo {
                code: "severe".to_string(),
                display: Some("Severe".to_string()),
                definition: None,
            },
        ],
        title: Some("Condition Severity".to_string()),
        description: Some("Preferred value set for condition severity".to_string()),
        binding_strength: Some(BindingStrength::Preferred),
    };

    let generated = vs.generate_typescript();
    assert_snapshot!("valueset_preferred_binding", generated);
}

#[test]
fn test_valueset_snapshot_no_displays() {
    let vs = ValueSetInfo {
        type_name: "SimpleCodeSystem".to_string(),
        canonical_url: "http://example.org/ValueSet/simple".to_string(),
        code_info: vec![
            CodeInfo {
                code: "code1".to_string(),
                display: None,
                definition: None,
            },
            CodeInfo {
                code: "code2".to_string(),
                display: None,
                definition: None,
            },
        ],
        title: None,
        description: None,
        binding_strength: Some(BindingStrength::Required),
    };

    let generated = vs.generate_typescript();
    assert_snapshot!("valueset_no_displays", generated);
}
