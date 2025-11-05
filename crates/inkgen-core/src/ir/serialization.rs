//! Deterministic serialization for IR

use crate::ir::ResourceIR;
use crate::{CoreError, Result};
use serde_json;

/// Handles deterministic serialization of IR structures
pub struct IRSerializer {
    version: String,
}

/// Serialized IR with version metadata
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SerializedIR {
    pub version: String,
    pub resource: ResourceIR,
}

impl IRSerializer {
    /// Create a new IRSerializer with version
    pub fn new(version: String) -> Self {
        Self { version }
    }

    /// Serialize ResourceIR to deterministic JSON
    pub fn serialize(&self, ir: &ResourceIR) -> Result<String> {
        let serialized = SerializedIR {
            version: self.version.clone(),
            resource: ir.clone(),
        };

        serde_json::to_string_pretty(&serialized).map_err(|e| CoreError::SerializationError {
            element: "ResourceIR".to_string(),
            reason: e.to_string(),
            context: None,
            source: Some(Box::new(e)),
        })
    }

    /// Deserialize JSON to ResourceIR
    pub fn deserialize(&self, json: &str) -> Result<ResourceIR> {
        let serialized: SerializedIR =
            serde_json::from_str(json).map_err(|e| CoreError::SerializationError {
                element: "JSON".to_string(),
                reason: e.to_string(),
                context: None,
                source: Some(Box::new(e)),
            })?;

        // Version compatibility check could be added here
        Ok(serialized.resource)
    }

    /// Create a human-readable debug representation
    pub fn debug_representation(&self, ir: &ResourceIR) -> String {
        format!(
            "ResourceIR(name: {}, elements: {}, bindings: {}, invariants: {})",
            ir.metadata.name,
            ir.elements.elements.len(),
            ir.bindings.len(),
            ir.invariants.len()
        )
    }

    /// Validate serialization round-trip
    pub fn validate_round_trip(&self, ir: &ResourceIR) -> Result<bool> {
        let serialized = self.serialize(ir)?;
        let deserialized = self.deserialize(&serialized)?;
        let re_serialized = self.serialize(&deserialized)?;

        Ok(serialized == re_serialized)
    }
}

impl Default for IRSerializer {
    fn default() -> Self {
        Self::new("0.1.0".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        ElementDefinition, ElementNode, ElementTree, ElementType, ResourceMetadata, ResourceKind,
        DerivationType, TerminologyBinding, BindingStrength, Invariant, InvariantSeverity,
    };

    fn create_test_resource_ir() -> ResourceIR {
        let metadata = ResourceMetadata {
            name: "Patient".to_string(),
            description: Some("Demographics and other administrative information about an individual".to_string()),
            kind: ResourceKind::Resource,
            base_definition: Some("http://hl7.org/fhir/StructureDefinition/DomainResource".to_string()),
            derivation: DerivationType::Specialization,
        };

        let element_def = ElementDefinition {
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

        let root_element = ElementNode::new("Patient".to_string(), element_def);
        let mut elements = ElementTree::new(root_element);

        // Add a child element
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
        elements.add_element(id_element.clone());

        // Add child to root
        if let Some(root) = elements.elements.get_mut("Patient") {
            root.add_child("Patient.id".to_string());
        }

        let mut ir = ResourceIR::new(metadata, elements);

        // Add a terminology binding
        let binding = TerminologyBinding::new(
            "Patient.gender".to_string(),
            Some("http://hl7.org/fhir/ValueSet/administrative-gender".to_string()),
            BindingStrength::Required,
        );
        ir.add_binding(binding);

        // Add an invariant
        let invariant = Invariant::new(
            "pat-1".to_string(),
            InvariantSeverity::Error,
            "SHALL have a contact's details or a reference to an organization".to_string(),
            "contact.name.exists() or contact.telecom.exists() or contact.address.exists() or contact.organization.exists()".to_string(),
        );
        ir.add_invariant(invariant);

        ir
    }

    #[test]
    fn test_serialize_deterministic_json() {
        let serializer = IRSerializer::default();
        let ir = create_test_resource_ir();

        let json1 = serializer.serialize(&ir).unwrap();
        let json2 = serializer.serialize(&ir).unwrap();

        // Should produce identical JSON output
        assert_eq!(json1, json2);

        // Should contain version metadata
        assert!(json1.contains("\"version\": \"0.1.0\""));
        
        // Should contain resource data
        assert!(json1.contains("\"name\": \"Patient\""));
        assert!(json1.contains("\"elements\""));
        assert!(json1.contains("\"bindings\""));
        assert!(json1.contains("\"invariants\""));
    }

    #[test]
    fn test_round_trip_serialization() {
        let serializer = IRSerializer::default();
        let original_ir = create_test_resource_ir();

        // Serialize and deserialize
        let json = serializer.serialize(&original_ir).unwrap();
        let deserialized_ir = serializer.deserialize(&json).unwrap();

        // Should be identical after round-trip
        let original_json = serializer.serialize(&original_ir).unwrap();
        let deserialized_json = serializer.serialize(&deserialized_ir).unwrap();
        
        assert_eq!(original_json, deserialized_json);
    }

    #[test]
    fn test_validate_round_trip() {
        let serializer = IRSerializer::default();
        let ir = create_test_resource_ir();

        let is_valid = serializer.validate_round_trip(&ir).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_consistent_ordering() {
        let serializer = IRSerializer::default();
        
        // Create multiple instances with same data but potentially different insertion order
        let ir1 = create_test_resource_ir();
        let ir2 = create_test_resource_ir();

        let json1 = serializer.serialize(&ir1).unwrap();
        let json2 = serializer.serialize(&ir2).unwrap();

        // Should produce identical JSON despite potentially different creation order
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_debug_representation() {
        let serializer = IRSerializer::default();
        let ir = create_test_resource_ir();

        let debug_str = serializer.debug_representation(&ir);
        
        assert!(debug_str.contains("ResourceIR(name: Patient"));
        assert!(debug_str.contains("elements: 2")); // Patient + Patient.id
        assert!(debug_str.contains("bindings: 1"));
        assert!(debug_str.contains("invariants: 1"));
    }

    #[test]
    fn test_version_metadata() {
        let custom_version = "1.2.3";
        let serializer = IRSerializer::new(custom_version.to_string());
        let ir = create_test_resource_ir();

        let json = serializer.serialize(&ir).unwrap();
        assert!(json.contains(&format!("\"version\": \"{}\"", custom_version)));
    }

    #[test]
    fn test_serialization_error_handling() {
        let serializer = IRSerializer::default();
        
        // Test deserialization with invalid JSON
        let invalid_json = "{ invalid json }";
        let result = serializer.deserialize(invalid_json);
        
        assert!(result.is_err());
        if let Err(CoreError::SerializationError { element, reason: _, .. }) = result {
            assert_eq!(element, "JSON");
        } else {
            panic!("Expected SerializationError");
        }
    }
}