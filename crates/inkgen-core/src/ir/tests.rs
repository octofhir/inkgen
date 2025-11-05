//! Snapshot tests for IR serialization

#[cfg(test)]
mod snapshot_tests {
    use super::super::*;
    use crate::ir::{
        ElementDefinition, ElementNode, ElementTree, ElementType, ResourceMetadata, ResourceKind,
        DerivationType, TerminologyBinding, BindingStrength, Invariant, InvariantSeverity,
        SlicingInfo, Discriminator, DiscriminatorType, SlicingRules,
    };

    fn create_patient_ir() -> ResourceIR {
        let metadata = ResourceMetadata {
            name: "Patient".to_string(),
            description: Some("Demographics and other administrative information about an individual".to_string()),
            kind: ResourceKind::Resource,
            base_definition: Some("http://hl7.org/fhir/StructureDefinition/DomainResource".to_string()),
            derivation: DerivationType::Specialization,
        };

        // Create root element
        let patient_def = ElementDefinition {
            min: 0,
            max: "1".to_string(),
            types: vec![ElementType {
                code: "Patient".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: false,
            short: Some("Information about an individual or animal receiving health care services".to_string()),
            definition: Some("Demographics and other administrative information about an individual or animal receiving care or other health-related services.".to_string()),
            comment: None,
        };

        let root_element = ElementNode::new("Patient".to_string(), patient_def);
        let mut elements = ElementTree::new(root_element);

        // Add id element
        let id_def = ElementDefinition {
            min: 0,
            max: "1".to_string(),
            types: vec![ElementType {
                code: "id".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: false,
            short: Some("Logical id of this artifact".to_string()),
            definition: Some("The logical id of the resource, as used in the URL for the resource.".to_string()),
            comment: None,
        };

        let id_element = ElementNode::new("Patient.id".to_string(), id_def);
        elements.add_element(id_element);

        // Add name element with slicing
        let name_def = ElementDefinition {
            min: 0,
            max: "*".to_string(),
            types: vec![ElementType {
                code: "HumanName".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: true,
            short: Some("A name associated with the individual".to_string()),
            definition: Some("A name associated with the patient.".to_string()),
            comment: Some("A patient may have multiple names with different uses or applicable periods.".to_string()),
        };

        let mut name_element = ElementNode::new("Patient.name".to_string(), name_def);
        name_element.slicing = Some(SlicingInfo {
            discriminator: vec![Discriminator {
                type_: DiscriminatorType::Value,
                path: "use".to_string(),
            }],
            description: Some("Slice based on the name use".to_string()),
            ordered: false,
            rules: SlicingRules::Open,
        });
        elements.add_element(name_element);

        // Add gender element
        let gender_def = ElementDefinition {
            min: 0,
            max: "1".to_string(),
            types: vec![ElementType {
                code: "code".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: false,
            short: Some("male | female | other | unknown".to_string()),
            definition: Some("Administrative Gender - the gender that the patient is considered to have for administration and record keeping purposes.".to_string()),
            comment: None,
        };

        let gender_element = ElementNode::new("Patient.gender".to_string(), gender_def);
        elements.add_element(gender_element);

        // Update root element children
        if let Some(root) = elements.elements.get_mut("Patient") {
            root.add_child("Patient.id".to_string());
            root.add_child("Patient.name".to_string());
            root.add_child("Patient.gender".to_string());
        }

        let mut ir = ResourceIR::new(metadata, elements);

        // Add terminology binding for gender
        let gender_binding = TerminologyBinding::new(
            "Patient.gender".to_string(),
            Some("http://hl7.org/fhir/ValueSet/administrative-gender".to_string()),
            BindingStrength::Required,
        );
        ir.add_binding(gender_binding);

        // Add invariant
        let invariant = Invariant::new(
            "pat-1".to_string(),
            InvariantSeverity::Error,
            "SHALL have a contact's details or a reference to an organization".to_string(),
            "contact.name.exists() or contact.telecom.exists() or contact.address.exists() or contact.organization.exists()".to_string(),
        );
        ir.add_invariant(invariant);

        ir
    }

    fn create_observation_ir() -> ResourceIR {
        let metadata = ResourceMetadata {
            name: "Observation".to_string(),
            description: Some("Measurements and simple assertions made about a patient, device or other subject".to_string()),
            kind: ResourceKind::Resource,
            base_definition: Some("http://hl7.org/fhir/StructureDefinition/DomainResource".to_string()),
            derivation: DerivationType::Specialization,
        };

        // Create root element
        let obs_def = ElementDefinition {
            min: 0,
            max: "1".to_string(),
            types: vec![ElementType {
                code: "Observation".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: false,
            short: Some("Measurements and simple assertions".to_string()),
            definition: Some("Measurements and simple assertions made about a patient, device or other subject.".to_string()),
            comment: None,
        };

        let root_element = ElementNode::new("Observation".to_string(), obs_def);
        let mut elements = ElementTree::new(root_element);

        // Add status element
        let status_def = ElementDefinition {
            min: 1,
            max: "1".to_string(),
            types: vec![ElementType {
                code: "code".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: true,
            short: Some("registered | preliminary | final | amended +".to_string()),
            definition: Some("The status of the result value.".to_string()),
            comment: None,
        };

        let status_element = ElementNode::new("Observation.status".to_string(), status_def);
        elements.add_element(status_element);

        // Add code element
        let code_def = ElementDefinition {
            min: 1,
            max: "1".to_string(),
            types: vec![ElementType {
                code: "CodeableConcept".to_string(),
                profile: None,
                target_profile: None,
            }],
            must_support: true,
            short: Some("Type of observation (code / type)".to_string()),
            definition: Some("Describes what was observed. Sometimes this is called the observation \"name\".".to_string()),
            comment: None,
        };

        let code_element = ElementNode::new("Observation.code".to_string(), code_def);
        elements.add_element(code_element);

        // Update root element children
        if let Some(root) = elements.elements.get_mut("Observation") {
            root.add_child("Observation.status".to_string());
            root.add_child("Observation.code".to_string());
        }

        let mut ir = ResourceIR::new(metadata, elements);

        // Add terminology bindings
        let status_binding = TerminologyBinding::new(
            "Observation.status".to_string(),
            Some("http://hl7.org/fhir/ValueSet/observation-status".to_string()),
            BindingStrength::Required,
        );
        ir.add_binding(status_binding);

        let code_binding = TerminologyBinding::new(
            "Observation.code".to_string(),
            Some("http://hl7.org/fhir/ValueSet/observation-codes".to_string()),
            BindingStrength::Example,
        );
        ir.add_binding(code_binding);

        // Add invariant
        let invariant = Invariant::new(
            "obs-6".to_string(),
            InvariantSeverity::Error,
            "dataAbsentReason SHALL only be present if Observation.value[x] is not present".to_string(),
            "dataAbsentReason.empty() or value.empty()".to_string(),
        );
        ir.add_invariant(invariant);

        ir
    }

    #[test]
    fn test_patient_ir_snapshot() {
        let serializer = IRSerializer::default();
        let patient_ir = create_patient_ir();
        let json = serializer.serialize(&patient_ir).unwrap();
        
        insta::assert_snapshot!("patient_ir_serialization", json);
    }

    #[test]
    fn test_observation_ir_snapshot() {
        let serializer = IRSerializer::default();
        let observation_ir = create_observation_ir();
        let json = serializer.serialize(&observation_ir).unwrap();
        
        insta::assert_snapshot!("observation_ir_serialization", json);
    }

    #[test]
    fn test_multiple_serializations_consistent() {
        let serializer = IRSerializer::default();
        let patient_ir = create_patient_ir();
        
        // Serialize multiple times
        let json1 = serializer.serialize(&patient_ir).unwrap();
        let json2 = serializer.serialize(&patient_ir).unwrap();
        let json3 = serializer.serialize(&patient_ir).unwrap();
        
        // All should be identical
        assert_eq!(json1, json2);
        assert_eq!(json2, json3);
        
        insta::assert_snapshot!("consistent_serialization", json1);
    }

    #[test]
    fn test_round_trip_snapshot() {
        let serializer = IRSerializer::default();
        let original_ir = create_patient_ir();
        
        // Serialize, deserialize, then serialize again
        let json1 = serializer.serialize(&original_ir).unwrap();
        let deserialized_ir = serializer.deserialize(&json1).unwrap();
        let json2 = serializer.serialize(&deserialized_ir).unwrap();
        
        // Should be identical
        assert_eq!(json1, json2);
        
        insta::assert_snapshot!("round_trip_serialization", json2);
    }
}