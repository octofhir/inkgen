//! Integration tests for Profile generation
//!
//! These tests verify the complete profile generation workflow including
//! class structure, extension accessors, serialization, and validation.

use indexmap::IndexMap;
use inkgen_core::config::{ExtensionAccessorStyle, ProfileMethodConfig};
use inkgen_core::ir::{
    Derivation, ElementCardinality, ElementDefinition, ElementMax, ProfileLineage,
    ResourceDefinition, ResourceKind,
};
use inkgen_typescript::extensions::RenderExtension;
use inkgen_typescript::profiles::{ConstrainedElement, FixedElement, ProfileInfo};
use insta::assert_snapshot;
use tera::Tera;

/// Helper function to create a Tera instance with the profile template
fn create_test_tera() -> Tera {
    let mut tera = Tera::default();
    tera.add_raw_template(
        "profile.ts.tera",
        include_str!("../src/templates/profile.ts.tera"),
    )
    .expect("Failed to add profile template");
    tera
}

/// Helper function to create a test profile with basic structure
fn create_test_profile() -> ProfileInfo {
    ProfileInfo {
        type_name: "USCorePatient".to_string(),
        canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
            .to_string(),
        base_type: "Patient".to_string(),
        title: Some("US Core Patient Profile".to_string()),
        description: Some("Defines constraints and extensions on the Patient resource".to_string()),
        must_support_elements: vec!["Patient.identifier".to_string(), "Patient.name".to_string()],
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
    }
}

/// Helper function to create a test extension
fn create_race_extension() -> RenderExtension {
    RenderExtension {
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
        type_name: "USCoreRaceExtension".to_string(),
        contexts: vec![],
        is_complex: false,
        value_type: Some("Coding".to_string()),
        value_type_code: None,
        nested_types: vec![],
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: Some("Race and ethnicity of the patient".to_string()),
    }
}

/// Helper function to create a complex extension
fn create_complex_extension() -> RenderExtension {
    RenderExtension {
        url: "http://example.org/StructureDefinition/complex-extension".to_string(),
        type_name: "ComplexExtension".to_string(),
        contexts: vec![],
        is_complex: true,
        value_type: None,
        value_type_code: None,
        nested_types: vec![],
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: Some("A complex extension with nested sub-extensions".to_string()),
    }
}

#[test]
fn test_profile_class_structure() {
    let tera = create_test_tera();
    let profile = create_test_profile();
    let method_config = ProfileMethodConfig::default();

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Check class declaration
    assert!(
        generated.contains("export class USCorePatient extends Patient"),
        "Should generate class extending base type"
    );

    // Check readonly __profile property
    assert!(
        generated.contains(
            "readonly __profile = 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient'"
        ),
        "Should include readonly __profile property"
    );

    // Check must-support element constraint
    assert!(
        generated.contains("declare identifier: NonNullable<Patient['identifier']>"),
        "Should declare must-support elements as required"
    );

    // Check JSDoc comments
    assert!(
        generated.contains("* US Core Patient Profile"),
        "Should include title in JSDoc"
    );
    assert!(
        generated.contains("* Defines constraints and extensions on the Patient resource"),
        "Should include description in JSDoc"
    );
    assert!(
        generated
            .contains("* @profile http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"),
        "Should include @profile tag"
    );
}

#[test]
fn test_extension_accessor_typed_style() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Typed,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should have typed accessors
    assert!(
        generated.contains("get uSCoreRace(): Coding | undefined"),
        "Should generate typed getter for extension"
    );
    assert!(
        generated.contains("set uSCoreRace(value: Coding | undefined)"),
        "Should generate typed setter for extension"
    );
    assert!(
        generated.contains("valueCoding"),
        "Should use correct value field for Coding type"
    );

    // Should NOT have raw Extension accessors
    assert!(
        !generated.contains("get uSCoreRaceExtension()"),
        "Should not generate raw Extension getter with 'typed' style"
    );
    assert!(
        !generated.contains("Extension | undefined"),
        "Should not reference Extension type with 'typed' style"
    );
}

#[test]
fn test_extension_accessor_raw_style() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Raw,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should have raw Extension accessors
    assert!(
        generated.contains("get uSCoreRaceExtension(): Extension | undefined"),
        "Should generate raw Extension getter"
    );
    assert!(
        generated.contains("set uSCoreRaceExtension(value: Extension | undefined)"),
        "Should generate raw Extension setter"
    );

    // Should NOT have typed accessors
    assert!(
        !generated.contains("get uSCoreRace(): Coding"),
        "Should not generate typed getter with 'raw' style"
    );
    assert!(
        !generated.contains("set uSCoreRace(value: Coding"),
        "Should not generate typed setter with 'raw' style"
    );
}

#[test]
fn test_extension_accessor_both_style() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should have both typed accessors
    assert!(
        generated.contains("get uSCoreRace(): Coding | undefined"),
        "Should generate typed getter with 'both' style"
    );
    assert!(
        generated.contains("set uSCoreRace(value: Coding | undefined)"),
        "Should generate typed setter with 'both' style"
    );

    // AND raw Extension accessors
    assert!(
        generated.contains("get uSCoreRaceExtension(): Extension | undefined"),
        "Should generate raw Extension getter with 'both' style"
    );
    assert!(
        generated.contains("set uSCoreRaceExtension(value: Extension | undefined)"),
        "Should generate raw Extension setter with 'both' style"
    );
}

#[test]
fn test_complex_extension_accessor() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_complex_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Complex extensions should generate Extension type accessors
    assert!(
        generated.contains("get complex(): Extension | undefined"),
        "Should generate getter for complex extension"
    );
    assert!(
        generated.contains("set complex(value: Extension | undefined)"),
        "Should generate setter for complex extension"
    );
    assert!(
		generated.contains(
			"this.extension?.find(e => e.url === 'http://example.org/StructureDefinition/complex-extension')"
		),
		"Should search for complex extension by URL"
	);
}

#[test]
fn test_fixed_value_constraints() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.fixed_elements = vec![
        FixedElement {
            path: "Patient.active".to_string(),
            field_name: "active".to_string(),
            fixed_value: "true".to_string(),
            value_type: "boolean".to_string(),
        },
        FixedElement {
            path: "Patient.gender".to_string(),
            field_name: "gender".to_string(),
            fixed_value: "\"female\"".to_string(),
            value_type: "string".to_string(),
        },
    ];

    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Check fixed value declarations
    assert!(
        generated.contains("/** Fixed value: true */"),
        "Should include JSDoc for fixed boolean value"
    );
    assert!(
        generated.contains("declare active: true"),
        "Should declare active as fixed boolean literal"
    );

    assert!(
        generated.contains("/** Fixed value: \"female\" */"),
        "Should include JSDoc for fixed string value"
    );
    assert!(
        generated.contains("declare gender: \"female\""),
        "Should declare gender as fixed string literal"
    );
}

#[test]
fn test_serialization_methods() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Check toJson method
    assert!(
        generated.contains("toJson(pretty?: boolean): string"),
        "Should generate toJson method signature"
    );
    assert!(
        generated.contains("return pretty ? JSON.stringify(this, null, 2) : JSON.stringify(this)"),
        "Should implement toJson with pretty-print option"
    );

    // Check toObject method
    assert!(
        generated.contains("toObject(): Patient"),
        "Should generate toObject method signature"
    );
    assert!(
        generated.contains("const { __profile, ...rest } = this"),
        "Should destructure __profile from this"
    );
    assert!(
        generated.contains("return rest as Patient"),
        "Should return object without __profile"
    );
}

#[test]
fn test_validation_methods_without_zod() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Check fromJson method
    assert!(
        generated.contains("static fromJson(json: string): USCorePatient"),
        "Should generate static fromJson method"
    );
    assert!(
        generated.contains("const parsed = JSON.parse(json)"),
        "Should parse JSON string"
    );
    assert!(
        generated.contains("return USCorePatient.fromObject(parsed)"),
        "Should delegate to fromObject"
    );

    // Check fromObject method
    assert!(
        generated.contains("static fromObject(obj: unknown): USCorePatient"),
        "Should generate static fromObject method"
    );
    assert!(
        generated.contains("return Object.assign(new USCorePatient(), obj)"),
        "Should create instance and assign properties without validation when zod is disabled"
    );
}

#[test]
fn test_validation_methods_with_zod() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .unwrap();

    // Check Zod schema generation
    assert!(
        generated.contains("export const USCorePatientSchema = PatientSchema.extend({"),
        "Should generate Zod schema extending base schema"
    );

    // Check fromObject uses schema
    assert!(
        generated.contains("const validated = USCorePatientSchema.parse(obj)"),
        "Should validate with Zod schema when enabled"
    );
    assert!(
        generated.contains("return Object.assign(new USCorePatient(), validated)"),
        "Should create instance with validated data"
    );

    // Check Zod imports
    assert!(
        generated.contains("import { z } from 'zod'"),
        "Should import zod when validation is enabled"
    );
    assert!(
        generated.contains("import { PatientSchema } from \"./Patient\""),
        "Should import base schema"
    );
}

#[test]
fn test_serialization_disabled() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: false,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should NOT have serialization methods
    assert!(
        !generated.contains("toJson("),
        "Should not generate toJson when serialization is disabled"
    );
    assert!(
        !generated.contains("toObject()"),
        "Should not generate toObject when serialization is disabled"
    );
}

#[test]
fn test_validation_disabled() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: false,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should NOT have validation methods
    assert!(
        !generated.contains("static fromJson("),
        "Should not generate fromJson when validation is disabled"
    );
    assert!(
        !generated.contains("static fromObject("),
        "Should not generate fromObject when validation is disabled"
    );
}

#[test]
fn test_extension_accessors_disabled() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: false,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should NOT have any extension accessors
    assert!(
        !generated.contains("get uSCoreRace"),
        "Should not generate typed getter when extension_accessors is disabled"
    );
    assert!(
        !generated.contains("set uSCoreRace"),
        "Should not generate typed setter when extension_accessors is disabled"
    );
    assert!(
        !generated.contains("get uSCoreRaceExtension"),
        "Should not generate raw getter when extension_accessors is disabled"
    );
    assert!(
        !generated.contains("set uSCoreRaceExtension"),
        "Should not generate raw setter when extension_accessors is disabled"
    );

    // Should still have class structure
    assert!(
        generated.contains("export class USCorePatient extends Patient"),
        "Should still generate class when accessors are disabled"
    );
}

#[test]
fn test_all_methods_disabled() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: false,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: false,
        validation: false,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should have minimal class structure
    assert!(
        generated.contains("export class USCorePatient extends Patient"),
        "Should generate class declaration"
    );
    assert!(
        generated.contains("readonly __profile ="),
        "Should include __profile property"
    );
    assert!(
        generated.contains("declare identifier: NonNullable<Patient['identifier']>"),
        "Should include must-support constraints"
    );

    // Should NOT have any methods
    assert!(!generated.contains("get "), "Should not have any getters");
    assert!(!generated.contains("set "), "Should not have any setters");
    assert!(
        !generated.contains("toJson("),
        "Should not have toJson method"
    );
    assert!(
        !generated.contains("toObject()"),
        "Should not have toObject method"
    );
    assert!(
        !generated.contains("static fromJson("),
        "Should not have fromJson method"
    );
    assert!(
        !generated.contains("static fromObject("),
        "Should not have fromObject method"
    );
}

#[test]
fn test_zod_schema_generation() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .unwrap();

    // Check Zod schema structure
    assert!(
        generated.contains("export const USCorePatientSchema = PatientSchema.extend({"),
        "Should generate Zod schema"
    );
    assert!(
        generated.contains("identifier: z.array(z.unknown()).min(1)"),
        "Should add cardinality constraint for must-support element"
    );
    assert!(generated.contains("});"), "Should close schema object");

    // Check Zod type export
    assert!(
        generated
            .contains("export type USCorePatientValidated = z.infer<typeof USCorePatientSchema>"),
        "Should export validated type"
    );
}

#[test]
fn test_type_guard_generation() {
    let tera = create_test_tera();
    let profile = create_test_profile();
    let method_config = ProfileMethodConfig::default();

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Check type guard function
    assert!(
        generated
            .contains("export function isUSCorePatient(value: Patient): value is USCorePatient"),
        "Should generate type guard function signature"
    );
    assert!(
        generated.contains("return '__profile' in value && value.__profile === 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient'"),
        "Should check __profile property"
    );
}

#[test]
fn test_multiple_extensions() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();

    let ethnicity_extension = RenderExtension {
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity".to_string(),
        type_name: "USCoreEthnicityExtension".to_string(),
        contexts: vec![],
        is_complex: false,
        value_type: Some("Coding".to_string()),
        value_type_code: None,
        nested_types: vec![],
        cardinality_min: 0,
        cardinality_max: Some(1),
        description: Some("Ethnicity of the patient".to_string()),
    };

    profile.extensions = vec![create_race_extension(), ethnicity_extension];

    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    // Should have accessors for both extensions
    assert!(
        generated.contains("get uSCoreRace()"),
        "Should generate race getter"
    );
    assert!(
        generated.contains("get uSCoreEthnicity()"),
        "Should generate ethnicity getter"
    );
    assert!(
        generated.contains("set uSCoreRace(value: Coding | undefined)"),
        "Should generate race setter"
    );
    assert!(
        generated.contains("set uSCoreEthnicity(value: Coding | undefined)"),
        "Should generate ethnicity setter"
    );
}

#[test]
fn test_profile_from_resource_definition() {
    // Create a ResourceDefinition representing a profile
    let definition = ResourceDefinition {
        id: "us-core-patient".to_string(),
        url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
        name: Some("USCorePatientProfile".to_string()),
        title: Some("US Core Patient Profile".to_string()),
        description: Some("Defines constraints on Patient resource for US Core".to_string()),
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
            create_test_element("Patient", 0, 1, false, None),
            create_test_element("Patient.identifier", 1, usize::MAX, true, None),
            // String fixed value: numeric/bool fixed values are intentionally
            // skipped (they conflict with branded primitive types).
            create_test_element(
                "Patient.gender",
                0,
                1,
                false,
                Some(serde_json::json!("female")),
            ),
        ],
        flat_elements: vec![],
        extensions: vec![],
        invariants: vec![],
    };

    let profile = ProfileInfo::from_resource_definition(&definition, &indexmap::IndexMap::new())
        .expect("Should create ProfileInfo from constraint profile");

    assert_eq!(profile.type_name, "USCorePatientProfile");
    assert_eq!(profile.base_type, "Patient");
    assert_eq!(profile.title, Some("US Core Patient Profile".to_string()));
    assert!(
        profile
            .must_support_elements
            .contains(&"Patient.identifier".to_string())
    );
    assert_eq!(profile.fixed_elements.len(), 1);
    assert_eq!(profile.fixed_elements[0].field_name, "gender");
    assert_eq!(profile.constrained_elements.len(), 1);
    assert!(profile.constrained_elements[0].makes_required);
}

#[test]
fn test_profile_from_non_constraint_definition() {
    // Create a ResourceDefinition that is NOT a profile (Specialization, not Constraint)
    let definition = ResourceDefinition {
        id: "Patient".to_string(),
        url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        name: Some("Patient".to_string()),
        title: None,
        description: None,
        version: None,
        status: None,
        kind: ResourceKind::Resource,
        fhir_type: Some("Patient".to_string()),
        date: None,
        lineage: ProfileLineage {
            base_definition: None,
            base_id: None,
            derivation: Some(Derivation::Specialization),
            type_name: None,
        },
        elements: vec![],
        flat_elements: vec![],
        extensions: vec![],
        invariants: vec![],
    };

    let profile = ProfileInfo::from_resource_definition(&definition, &indexmap::IndexMap::new());
    assert!(
        profile.is_none(),
        "Should return None for non-constraint definitions"
    );
}

/// Helper function to create a test ElementDefinition
fn create_test_element(
    path: &str,
    min: u32,
    max: usize,
    must_support: bool,
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
        types: vec![],
        content_reference: None,
        binding: None,
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

// Snapshot tests for deterministic output verification

#[test]
fn test_profile_snapshot_all_features() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];
    profile.fixed_elements = vec![FixedElement {
        path: "Patient.active".to_string(),
        field_name: "active".to_string(),
        fixed_value: "true".to_string(),
        value_type: "boolean".to_string(),
    }];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .unwrap();

    assert_snapshot!("profile_all_features", generated);
}

#[test]
fn test_profile_snapshot_typed_accessors() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Typed,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    assert_snapshot!("profile_typed_accessors", generated);
}

#[test]
fn test_profile_snapshot_raw_accessors() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_race_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Raw,
        serialization: true,
        validation: true,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    assert_snapshot!("profile_raw_accessors", generated);
}

#[test]
fn test_profile_snapshot_minimal_config() {
    let tera = create_test_tera();
    let profile = create_test_profile();

    let method_config = ProfileMethodConfig {
        extension_accessors: false,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: false,
        validation: false,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    assert_snapshot!("profile_minimal_config", generated);
}

#[test]
fn test_profile_snapshot_complex_extension() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();
    profile.extensions = vec![create_complex_extension()];

    let method_config = ProfileMethodConfig {
        extension_accessors: true,
        extension_style: ExtensionAccessorStyle::Both,
        serialization: false,
        validation: false,
    };

    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, false)
        .unwrap();

    assert_snapshot!("profile_complex_extension", generated);
}

#[test]
fn test_profile_snapshot_multiple_constraints() {
    let tera = create_test_tera();
    let mut profile = create_test_profile();

    // Add multiple constraint types
    profile.must_support_elements = vec![
        "Patient.identifier".to_string(),
        "Patient.name".to_string(),
        "Patient.gender".to_string(),
    ];
    profile.constrained_elements = vec![
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
    ];
    profile.fixed_elements = vec![
        FixedElement {
            path: "Patient.active".to_string(),
            field_name: "active".to_string(),
            fixed_value: "true".to_string(),
            value_type: "boolean".to_string(),
        },
        FixedElement {
            path: "Patient.gender".to_string(),
            field_name: "gender".to_string(),
            fixed_value: "\"female\"".to_string(),
            value_type: "string".to_string(),
        },
    ];

    let method_config = ProfileMethodConfig::default();
    let generated = profile
        .generate_typescript_with_template(&tera, &method_config, true)
        .unwrap();

    assert_snapshot!("profile_multiple_constraints", generated);
}
