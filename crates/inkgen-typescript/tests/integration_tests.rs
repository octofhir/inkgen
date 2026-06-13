//! Integration tests for TypeScript generation
//!
//! These tests verify that different components of the TypeScript generator
//! work correctly together, including profiles, valuesets, extensions,
//! configuration, and templates.

use indexmap::IndexMap;
use inkgen_core::config::{ExtensionAccessorStyle, ProfileMethodConfig};
use inkgen_core::ir::{
    BindingDefinition, BindingStrength as IrBindingStrength, Derivation, ElementCardinality,
    ElementDefinition, ElementMax, ElementType, ProfileLineage, ResourceDefinition, ResourceKind,
};
use inkgen_typescript::extensions::RenderExtension;
use inkgen_typescript::profiles::{ConstrainedElement, FixedElement, ProfileInfo};
use inkgen_typescript::valuesets::{BindingStrength, CodeInfo, ValueSetInfo};
use insta::assert_snapshot;
use serde_json::json;
use tera::Tera;

/// Helper to create a test Tera instance with all templates
fn create_test_tera() -> Tera {
    let mut tera = Tera::default();
    tera.add_raw_template(
        "profile.ts.tera",
        include_str!("../src/templates/profile.ts.tera"),
    )
    .expect("Failed to add profile template");
    tera
}

/// Helper to create a test ResourceDefinition representing a profile
fn create_test_profile_definition() -> ResourceDefinition {
    ResourceDefinition {
        id: "us-core-patient".to_string(),
        flat_elements: vec![],
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
        name: Some("USCorePatientProfile".to_string()),
        title: Some("US Core Patient Profile".to_string()),
        description: Some(
            "The US Core Patient Profile meets the U.S. Core Data for Interoperability (USCDI) requirements"
                .to_string(),
        ),
        version: None,
        status: None,
        kind: ResourceKind::Resource,
        fhir_type: Some("Patient".to_string()),
        date: None,
        lineage: ProfileLineage {
            base_definition: Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
            base_id: Some("Patient".to_string()),
            derivation: Some(Derivation::Constraint),
            type_name: Some("Patient".to_string()),
        },
        elements: vec![
            create_element("Patient", 0, 1, false, None, None),
            create_element(
                "Patient.identifier",
                1,
                usize::MAX,
                true,
                None,
                None,
            ),
            create_element(
                "Patient.name",
                1,
                usize::MAX,
                true,
                None,
                None,
            ),
            create_element(
                "Patient.gender",
                1,
                1,
                true,
                Some(BindingDefinition {
                    strength: IrBindingStrength::Required,
                    value_set: Some(
                        "http://hl7.org/fhir/ValueSet/administrative-gender".to_string(),
                    ),
                    description: None,
                    additional: IndexMap::new(),
                }),
                None,
            ),
            create_element(
                "Patient.maritalStatus",
                0,
                1,
                false,
                None,
                // String fixed value: numeric/bool fixed values are intentionally
                // skipped (they conflict with branded primitive types).
                Some(json!("M")),
            ),
        ],
        extensions: vec![],
        invariants: vec![],
    }
}

/// Helper to create a test ElementDefinition
fn create_element(
    path: &str,
    min: u32,
    max: usize,
    must_support: bool,
    binding: Option<BindingDefinition>,
    fixed: Option<serde_json::Value>,
) -> ElementDefinition {
    ElementDefinition {
        id: path.to_string(),
        path: path.to_string(),
        slice_name: None,
        short: None,
        definition: None,
        comment: None,
        requirements: None,
        cardinality: ElementCardinality {
            min,
            max: if max == usize::MAX {
                ElementMax::Unbounded
            } else {
                ElementMax::Finite(max as u32)
            },
        },
        types: if path.contains("gender") {
            vec![ElementType {
                code: "code".to_string(),
                profiles: vec![],
                target_profiles: vec![],
                versioning: None,
                aggregation: vec![],
            }]
        } else {
            vec![]
        },
        content_reference: None,
        binding,
        invariants: vec![],
        fixed,
        pattern: None,
        default_value: None,
        example_values: vec![],
        must_support,
        is_summary: false,
        slicing: None,
        extension: vec![],
        additional_fields: IndexMap::new(),
        children: vec![],
        parent_path: None,
        depth: 0,
        is_backbone: false,
    }
}

#[test]
fn test_profile_with_extensions_integration() {
    let tera = create_test_tera();

    // Create a profile with an extension
    let race_extension = RenderExtension {
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
        type_name: "USCoreRaceExtension".to_string(),
        contexts: vec![],
        is_complex: false,
        value_type: Some("Coding".to_string()),
        value_type_code: None,
        value_member: None,
        nested_types: vec![],
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: Some("Race of the patient".to_string()),
    };

    let profile = ProfileInfo {
        type_name: "USCorePatient".to_string(),
        canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
            .to_string(),
        base_type: "Patient".to_string(),
        title: Some("US Core Patient Profile".to_string()),
        description: Some("Defines constraints on Patient resource".to_string()),
        must_support_elements: vec!["Patient.identifier".to_string()],
        fixed_elements: vec![FixedElement {
            path: "Patient.active".to_string(),
            field_name: "active".to_string(),
            fixed_value: "true".to_string(),
            value_type: "boolean".to_string(),
        }],
        constrained_elements: vec![ConstrainedElement {
            path: "Patient.identifier".to_string(),
            field_name: "identifier".to_string(),
            min: 1,
            max: "*".to_string(),
            makes_required: true,
            choice_members: Vec::new(),
        }],
        extensions: vec![race_extension],
    };

    // Generate with all features enabled
    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .expect("Profile generation should succeed");

    // Verify all components are present
    assert!(
        generated.contains("export class USCorePatient extends Patient"),
        "Should generate class declaration"
    );
    assert!(
        generated.contains("readonly __profile ="),
        "Should include __profile property"
    );
    assert!(
        generated.contains("declare active: true"),
        "Should include fixed value constraint"
    );
    assert!(
        generated.contains("declare identifier: NonNullable<Patient['identifier']>"),
        "Should include must-support constraint"
    );
    assert!(
        generated.contains("get uSCoreRace()"),
        "Should include typed extension accessor"
    );
    assert!(
        generated.contains("get uSCoreRaceExtension()"),
        "Should include raw extension accessor"
    );
    assert!(
        generated.contains("toJson(pretty?: boolean)"),
        "Should include serialization method"
    );
    assert!(
        generated.contains("static fromJson(json: string)"),
        "Should include validation method"
    );
    assert!(
        generated.contains("export const USCorePatientSchema"),
        "Should include Zod schema"
    );
    assert!(
        generated.contains("import { z } from 'zod'"),
        "Should import zod"
    );
}

#[test]
fn test_valueset_generation_integration() {
    // Create a valueset from FHIR JSON
    let valueset_json = json!({
        "resourceType": "ValueSet",
        "url": "http://hl7.org/fhir/ValueSet/administrative-gender",
        "name": "AdministrativeGender",
        "title": "Administrative Gender",
        "description": "The gender of a person used for administrative purposes",
        "status": "active",
        "expansion": {
            "contains": [
                {"code": "male", "display": "Male"},
                {"code": "female", "display": "Female"},
                {"code": "other", "display": "Other"},
                {"code": "unknown", "display": "Unknown"}
            ]
        }
    });

    // Extract valueset
    let valueset =
        ValueSetInfo::from_valueset(&valueset_json, "AdministrativeGender".to_string(), Some(50))
            .expect("Extraction should succeed")
            .expect("ValueSet should not be skipped");

    // Verify extraction
    assert_eq!(valueset.type_name, "AdministrativeGender");
    assert_eq!(
        valueset.canonical_url,
        "http://hl7.org/fhir/ValueSet/administrative-gender"
    );
    assert_eq!(valueset.code_info.len(), 4);
    assert_eq!(valueset.code_info[0].code, "male");
    assert_eq!(valueset.code_info[0].display, Some("Male".to_string()));

    // Generate TypeScript
    let generated = valueset.generate_typescript();

    // Verify generation
    assert!(
        generated.contains("export const AdministrativeGenderValues = ["),
        "Should generate const array"
    );
    assert!(
        generated.contains(r#""male", "female", "other", "unknown""#),
        "Should include all codes"
    );
    assert!(
        generated.contains("export type AdministrativeGender ="),
        "Should generate type alias"
    );
    assert!(
        generated.contains("export function isAdministrativeGender("),
        "Should generate type guard"
    );
    assert!(
        generated.contains("export const AdministrativeGenderDefinitions = {"),
        "Should generate definitions object"
    );
    assert!(
        generated.contains(r#"display: "Male""#),
        "Should include display values"
    );
}

#[test]
fn test_profile_from_resource_definition_integration() {
    let definition = create_test_profile_definition();

    // Extract profile info
    let profile = ProfileInfo::from_resource_definition(&definition, &indexmap::IndexMap::new())
        .expect("Should create ProfileInfo from constraint profile");

    // Verify extraction — profiles are named by canonical-URL segment
    // (`us-core-patient`), not the (non-unique) SD `name`.
    assert_eq!(profile.type_name, "UsCorePatient");
    assert_eq!(profile.base_type, "Patient");
    assert_eq!(profile.title, Some("US Core Patient Profile".to_string()));

    // Check must-support elements
    assert!(
        profile
            .must_support_elements
            .contains(&"Patient.identifier".to_string()),
        "Should extract identifier as must-support"
    );
    assert!(
        profile
            .must_support_elements
            .contains(&"Patient.name".to_string()),
        "Should extract name as must-support"
    );
    assert!(
        profile
            .must_support_elements
            .contains(&"Patient.gender".to_string()),
        "Should extract gender as must-support"
    );

    // Check fixed value
    assert_eq!(profile.fixed_elements.len(), 1);
    assert_eq!(profile.fixed_elements[0].field_name, "maritalStatus");
    assert_eq!(profile.fixed_elements[0].fixed_value, "\"M\"");

    // Check constrained elements (min > 0)
    assert!(
        profile.constrained_elements.len() >= 2,
        "Should have constrained elements for identifier and name"
    );

    // Generate TypeScript
    let tera = create_test_tera();
    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .expect("Generation should succeed");

    // Verify generated code
    assert!(
        generated.contains("export class UsCorePatient extends Patient"),
        "Should generate class"
    );
    assert!(
        generated.contains("declare maritalStatus: \"M\""),
        "Should include fixed value"
    );
    assert!(
        generated.contains("declare identifier:"),
        "Should include must-support identifier"
    );
    assert!(
        generated.contains("declare name:"),
        "Should include must-support name"
    );
}

#[test]
fn test_configuration_combinations_integration() {
    let tera = create_test_tera();
    let profile = ProfileInfo {
        type_name: "TestProfile".to_string(),
        canonical_url: "http://example.org/StructureDefinition/test".to_string(),
        base_type: "Patient".to_string(),
        title: Some("Test Profile".to_string()),
        description: Some("A test profile".to_string()),
        must_support_elements: vec![],
        fixed_elements: vec![],
        constrained_elements: vec![],
        extensions: vec![RenderExtension {
            url: "http://example.org/Extension/test".to_string(),
            type_name: "TestExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("string".to_string()),
            value_type_code: None,
            value_member: None,
            nested_types: vec![],
            cardinality_min: 0,
            cardinality_max: Some(1),
            description: None,
        }],
    };

    // Test 1: All features enabled with "both" style
    let config1 = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };
    let gen1 = profile
        .generate_typescript_with_template(&tera, &config1, true)
        .unwrap();
    assert!(gen1.contains("get test()"), "Should have typed accessor");
    assert!(
        gen1.contains("get testExtension()"),
        "Should have raw accessor"
    );
    assert!(gen1.contains("toJson("), "Should have serialization");
    assert!(gen1.contains("static fromJson("), "Should have validation");
    assert!(gen1.contains("Schema"), "Should have Zod schema");

    // Test 2: Only typed accessors, no serialization/validation
    let config2 = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Typed,
        serialization: false,
        validation: false,
    };
    let gen2 = profile
        .generate_typescript_with_template(&tera, &config2, false)
        .unwrap();
    assert!(gen2.contains("get test()"), "Should have typed accessor");
    assert!(
        !gen2.contains("get testExtension()"),
        "Should not have raw accessor"
    );
    assert!(!gen2.contains("toJson("), "Should not have serialization");
    assert!(
        !gen2.contains("static fromJson("),
        "Should not have validation"
    );

    // Test 3: Only raw accessors with Zod validation
    let config3 = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Raw,
        serialization: false,
        validation: true,
    };
    let gen3 = profile
        .generate_typescript_with_template(&tera, &config3, true)
        .unwrap();
    assert!(
        !gen3.contains("get test(): string"),
        "Should not have typed accessor"
    );
    assert!(
        gen3.contains("get testExtension()"),
        "Should have raw accessor"
    );
    assert!(!gen3.contains("toJson("), "Should not have serialization");
    assert!(gen3.contains("static fromJson("), "Should have validation");
    assert!(gen3.contains("Schema"), "Should have Zod schema");

    // Test 4: Minimal configuration (no methods)
    let config4 = ProfileMethodConfig {
        extension_accessors: false,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: false,
        validation: false,
    };
    let gen4 = profile
        .generate_typescript_with_template(&tera, &config4, false)
        .unwrap();
    assert!(!gen4.contains("get "), "Should not have any getters");
    assert!(!gen4.contains("set "), "Should not have any setters");
    assert!(!gen4.contains("toJson("), "Should not have serialization");
    assert!(
        !gen4.contains("static fromJson("),
        "Should not have validation"
    );
    assert!(
        gen4.contains("export class TestProfile extends Patient"),
        "Should still generate class"
    );
}

#[test]
fn test_valueset_with_binding_strength_integration() {
    // Test Required binding (closed type)
    let required_vs = ValueSetInfo {
        type_name: "RequiredStatus".to_string(),
        canonical_url: "http://example.org/ValueSet/required-status".to_string(),
        code_info: vec![
            CodeInfo {
                code: "active".to_string(),
                display: Some("Active".to_string()),
                definition: None,
            },
            CodeInfo {
                code: "inactive".to_string(),
                display: Some("Inactive".to_string()),
                definition: None,
            },
        ],
        title: None,
        description: None,
        binding_strength: Some(BindingStrength::Required),
    };

    let required_gen = required_vs.generate_typescript();
    assert!(
        !required_gen.contains("| (string & {})"),
        "Required binding should not allow custom codes"
    );
    assert!(
        required_gen.contains("export type RequiredStatus = typeof RequiredStatusValues[number];"),
        "Should generate closed type"
    );

    // Test Preferred binding (open type)
    let preferred_vs = ValueSetInfo {
        type_name: "PreferredStatus".to_string(),
        canonical_url: "http://example.org/ValueSet/preferred-status".to_string(),
        code_info: vec![CodeInfo {
            code: "active".to_string(),
            display: Some("Active".to_string()),
            definition: None,
        }],
        title: None,
        description: None,
        binding_strength: Some(BindingStrength::Preferred),
    };

    let preferred_gen = preferred_vs.generate_typescript();
    assert!(
        preferred_gen.contains("| (string & {})"),
        "Preferred binding should allow custom codes"
    );
    assert!(
        preferred_gen.contains("// Open valueset - allows custom codes"),
        "Should include comment about open type"
    );
}

#[test]
fn test_large_valueset_size_limit_integration() {
    // Create a large valueset
    let large_codes: Vec<_> = (0..150)
        .map(|i| CodeInfo {
            code: format!("code-{}", i),
            display: Some(format!("Code {}", i)),
            definition: None,
        })
        .collect();

    let valueset_json = json!({
        "resourceType": "ValueSet",
        "url": "http://example.org/ValueSet/large",
        "name": "LargeValueSet",
        "expansion": {
            "contains": large_codes.iter().map(|c| {
                json!({"code": c.code, "display": c.display})
            }).collect::<Vec<_>>()
        }
    });

    // Test with max_codes = 100 (should skip)
    let result1 = ValueSetInfo::from_valueset(&valueset_json, "Large".to_string(), Some(100))
        .expect("Extraction should succeed");
    assert!(
        result1.is_none(),
        "Should skip valueset exceeding max_codes"
    );

    // Test with max_codes = 200 (should include)
    let result2 = ValueSetInfo::from_valueset(&valueset_json, "Large".to_string(), Some(200))
        .expect("Extraction should succeed");
    assert!(
        result2.is_some(),
        "Should include valueset within max_codes"
    );
    let vs = result2.unwrap();
    assert_eq!(vs.code_info.len(), 150);

    // Test with no max (should include)
    let result3 = ValueSetInfo::from_valueset(&valueset_json, "Large".to_string(), None)
        .expect("Extraction should succeed");
    assert!(
        result3.is_some(),
        "Should include valueset when no max specified"
    );
}

#[test]
fn test_profile_type_guard_integration() {
    let tera = create_test_tera();
    let profile = ProfileInfo {
        type_name: "MyProfile".to_string(),
        canonical_url: "http://example.org/StructureDefinition/my-profile".to_string(),
        base_type: "Patient".to_string(),
        title: Some("My Profile".to_string()),
        description: None,
        must_support_elements: vec![],
        fixed_elements: vec![],
        constrained_elements: vec![],
        extensions: vec![],
    };

    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Verify type guard function
    assert!(
        generated.contains("export function isMyProfile(value: Patient): value is MyProfile"),
        "Should generate type guard function"
    );
    assert!(
        generated.contains("return '__profile' in value && value.__profile === 'http://example.org/StructureDefinition/my-profile'"),
        "Type guard should check __profile property"
    );
}

#[test]
fn test_zod_schema_with_constraints_integration() {
    let tera = create_test_tera();
    let profile = ProfileInfo {
        type_name: "ConstrainedProfile".to_string(),
        canonical_url: "http://example.org/StructureDefinition/constrained".to_string(),
        base_type: "Patient".to_string(),
        title: Some("Constrained Profile".to_string()),
        description: None,
        must_support_elements: vec![],
        fixed_elements: vec![],
        constrained_elements: vec![
            ConstrainedElement {
                path: "Patient.identifier".to_string(),
                field_name: "identifier".to_string(),
                min: 1,
                max: "*".to_string(),
                makes_required: true,
                choice_members: Vec::new(),
            },
            ConstrainedElement {
                path: "Patient.name".to_string(),
                field_name: "name".to_string(),
                min: 2,
                max: "*".to_string(),
                makes_required: true,
                choice_members: Vec::new(),
            },
        ],
        extensions: vec![],
    };

    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .unwrap();

    // Verify Zod schema with constraints
    assert!(
        generated.contains("export const ConstrainedProfileSchema = PatientSchema.extend({"),
        "Should generate Zod schema"
    );
    assert!(
        generated.contains("identifier: z.array(z.unknown()).min(1)"),
        "Should include min(1) constraint for identifier"
    );
    assert!(
        generated.contains("name: z.array(z.unknown()).min(2)"),
        "Should include min(2) constraint for name"
    );
    assert!(generated.contains("});"), "Should close schema object");

    // Verify type inference
    assert!(
        generated.contains(
            "export type ConstrainedProfileValidated = z.infer<typeof ConstrainedProfileSchema>"
        ),
        "Should export validated type"
    );
}

// Snapshot tests for deterministic output verification

#[test]
fn test_integration_snapshot_complete_profile() {
    let tera = create_test_tera();

    // Create a comprehensive profile with all features
    let race_extension = RenderExtension {
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
        type_name: "USCoreRaceExtension".to_string(),
        contexts: vec![],
        is_complex: false,
        value_type: Some("Coding".to_string()),
        value_type_code: None,
        value_member: None,
        nested_types: vec![],
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: Some("Race of the patient".to_string()),
    };

    let ethnicity_extension = RenderExtension {
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity".to_string(),
        type_name: "USCoreEthnicityExtension".to_string(),
        contexts: vec![],
        is_complex: false,
        value_type: Some("Coding".to_string()),
        value_type_code: None,
        value_member: None,
        nested_types: vec![],
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: Some("Ethnicity of the patient".to_string()),
    };

    let profile = ProfileInfo {
        type_name: "USCorePatient".to_string(),
        canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
            .to_string(),
        base_type: "Patient".to_string(),
        title: Some("US Core Patient Profile".to_string()),
        description: Some(
            "Defines constraints and extensions on the Patient resource for US Core".to_string(),
        ),
        must_support_elements: vec![
            "Patient.identifier".to_string(),
            "Patient.name".to_string(),
            "Patient.gender".to_string(),
        ],
        fixed_elements: vec![FixedElement {
            path: "Patient.active".to_string(),
            field_name: "active".to_string(),
            fixed_value: "true".to_string(),
            value_type: "boolean".to_string(),
        }],
        constrained_elements: vec![
            ConstrainedElement {
                path: "Patient.identifier".to_string(),
                field_name: "identifier".to_string(),
                min: 1,
                max: "*".to_string(),
                makes_required: true,
                choice_members: Vec::new(),
            },
            ConstrainedElement {
                path: "Patient.name".to_string(),
                field_name: "name".to_string(),
                min: 1,
                max: "*".to_string(),
                makes_required: true,
                choice_members: Vec::new(),
            },
        ],
        extensions: vec![race_extension, ethnicity_extension],
    };

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .unwrap();

    assert_snapshot!("integration_complete_profile", generated);
}

#[test]
fn test_integration_snapshot_valueset_administrative_gender() {
    let valueset_json = json!({
        "resourceType": "ValueSet",
        "url": "http://hl7.org/fhir/ValueSet/administrative-gender",
        "name": "AdministrativeGender",
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

    let valueset =
        ValueSetInfo::from_valueset(&valueset_json, "AdministrativeGender".to_string(), None)
            .unwrap()
            .unwrap();

    let generated = valueset.generate_typescript();
    assert_snapshot!("integration_valueset_administrative_gender", generated);
}

#[test]
fn test_integration_snapshot_profile_from_resource_definition() {
    let definition = create_test_profile_definition();
    let profile =
        ProfileInfo::from_resource_definition(&definition, &indexmap::IndexMap::new()).unwrap();

    let tera = create_test_tera();
    let method_config = ProfileMethodConfig {
        extension_accessors: false,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    assert_snapshot!("integration_profile_from_definition", generated);
}

#[test]
fn test_integration_snapshot_minimal_profile() {
    let tera = create_test_tera();
    let profile = ProfileInfo {
        type_name: "MinimalProfile".to_string(),
        canonical_url: "http://example.org/StructureDefinition/minimal".to_string(),
        base_type: "Patient".to_string(),
        title: Some("Minimal Profile".to_string()),
        description: Some("A minimal profile for testing".to_string()),
        must_support_elements: vec!["Patient.identifier".to_string()],
        fixed_elements: vec![],
        constrained_elements: vec![ConstrainedElement {
            path: "Patient.identifier".to_string(),
            field_name: "identifier".to_string(),
            min: 1,
            max: "*".to_string(),
            makes_required: true,
            choice_members: Vec::new(),
        }],
        extensions: vec![],
    };

    let method_config = ProfileMethodConfig {
        extension_accessors: false,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: false,
        validation: false,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    assert_snapshot!("integration_minimal_profile", generated);
}
