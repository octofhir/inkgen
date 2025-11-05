//! FHIR data structures and resource definitions

use serde::{Deserialize, Serialize};

/// Base FHIR resource trait
pub trait FhirResource {
    /// Get the resource type
    fn resource_type(&self) -> &'static str;
    
    /// Get the resource ID if present
    fn id(&self) -> Option<&str>;
}

/// FHIR resource types enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Patient,
    Observation,
    Practitioner,
    Organization,
    StructureDefinition,
    ValueSet,
    CodeSystem,
}

/// Base FHIR resource structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource type
    #[serde(rename = "resourceType")]
    pub resource_type: ResourceType,
    
    /// Logical id of this artifact
    pub id: Option<String>,
    
    /// Metadata about the resource
    pub meta: Option<Meta>,
}

/// Metadata about a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// Version specific identifier
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
    
    /// When the resource version last changed
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    
    /// Profiles this resource claims to conform to
    pub profile: Option<Vec<String>>,
}

/// FHIR Patient resource placeholder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    /// Base resource fields
    #[serde(flatten)]
    pub resource: Resource,
    
    /// Whether this patient record is in active use
    pub active: Option<bool>,
    
    /// A name associated with the patient
    pub name: Option<Vec<HumanName>>,
}

/// Human name structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanName {
    /// usual | official | temp | nickname | anonymous | old | maiden
    #[serde(rename = "use")]
    pub use_: Option<String>,
    
    /// Family name (often called 'Surname')
    pub family: Option<String>,
    
    /// Given names (not always 'first'). Includes middle names
    pub given: Option<Vec<String>>,
}

/// FHIR Observation resource placeholder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Base resource fields
    #[serde(flatten)]
    pub resource: Resource,
    
    /// registered | preliminary | final | amended +
    pub status: String,
    
    /// Type of observation (code / type)
    pub code: CodeableConcept,
    
    /// Who and/or what the observation is about
    pub subject: Option<Reference>,
}

/// A reference to another resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// Literal reference, Relative, internal or absolute URL
    pub reference: Option<String>,
    
    /// Type the reference refers to (e.g. "Patient")
    #[serde(rename = "type")]
    pub type_: Option<String>,
    
    /// Text alternative for the resource
    pub display: Option<String>,
}

/// Concept - reference to a terminology or just text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeableConcept {
    /// Code defined by a terminology system
    pub coding: Option<Vec<Coding>>,
    
    /// Plain text representation of the concept
    pub text: Option<String>,
}

/// A reference to a code defined by a terminology system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coding {
    /// Identity of the terminology system
    pub system: Option<String>,
    
    /// Version of the system - if relevant
    pub version: Option<String>,
    
    /// Symbol in syntax defined by the system
    pub code: Option<String>,
    
    /// Representation defined by the system
    pub display: Option<String>,
}

/// FHIR StructureDefinition resource for profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinition {
    /// Base resource fields
    #[serde(flatten)]
    pub resource: Resource,
    
    /// Canonical identifier for this structure definition
    pub url: String,
    
    /// Business version of the structure definition
    pub version: Option<String>,
    
    /// Name for this structure definition (computer friendly)
    pub name: String,
    
    /// Name for this structure definition (human friendly)
    pub title: Option<String>,
    
    /// draft | active | retired | unknown
    pub status: String,
    
    /// For testing purposes, not real usage
    pub experimental: Option<bool>,
    
    /// Type defined or constrained by this structure
    #[serde(rename = "type")]
    pub type_: String,
    
    /// Definition of elements in the resource (if no StructureDefinition)
    pub snapshot: Option<StructureDefinitionSnapshot>,
}

/// Snapshot view of the structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionSnapshot {
    /// Definition of elements in the resource
    pub element: Vec<ElementDefinition>,
}

/// Definition of an element in a resource or extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinition {
    /// Path of the element in the hierarchy of elements
    pub path: String,
    
    /// Minimum Cardinality
    pub min: Option<u32>,
    
    /// Maximum Cardinality (a number or *)
    pub max: Option<String>,
    
    /// Data type and Profile for this element
    #[serde(rename = "type")]
    pub type_: Option<Vec<ElementDefinitionType>>,
}

/// Data type and Profile for an element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinitionType {
    /// Data type or Resource (reference to definition)
    pub code: String,
    
    /// Profile (StructureDefinition or IG) - the base/constraint
    pub profile: Option<Vec<String>>,
}

impl FhirResource for Patient {
    fn resource_type(&self) -> &'static str {
        "Patient"
    }
    
    fn id(&self) -> Option<&str> {
        self.resource.id.as_deref()
    }
}

impl FhirResource for Observation {
    fn resource_type(&self) -> &'static str {
        "Observation"
    }
    
    fn id(&self) -> Option<&str> {
        self.resource.id.as_deref()
    }
}

impl FhirResource for StructureDefinition {
    fn resource_type(&self) -> &'static str {
        "StructureDefinition"
    }
    
    fn id(&self) -> Option<&str> {
        self.resource.id.as_deref()
    }
}
#
[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_patient_serialization() {
        let patient = Patient {
            resource: Resource {
                resource_type: ResourceType::Patient,
                id: Some("patient-123".to_string()),
                meta: Some(Meta {
                    version_id: Some("1".to_string()),
                    last_updated: Some("2023-01-01T00:00:00Z".to_string()),
                    profile: None,
                }),
            },
            active: Some(true),
            name: Some(vec![HumanName {
                use_: Some("official".to_string()),
                family: Some("Doe".to_string()),
                given: Some(vec!["John".to_string()]),
            }]),
        };

        let json = serde_json::to_string(&patient).expect("Failed to serialize patient");
        assert!(json.contains("Patient"));
        assert!(json.contains("patient-123"));
        assert!(json.contains("John"));
        assert!(json.contains("Doe"));
    }

    #[test]
    fn test_patient_deserialization() {
        let json = r#"{
            "resourceType": "Patient",
            "id": "patient-456",
            "active": true,
            "name": [{
                "use": "official",
                "family": "Smith",
                "given": ["Jane"]
            }]
        }"#;

        let patient: Patient = serde_json::from_str(json).expect("Failed to deserialize patient");
        assert_eq!(patient.resource.resource_type, ResourceType::Patient);
        assert_eq!(patient.resource.id, Some("patient-456".to_string()));
        assert_eq!(patient.active, Some(true));
        assert!(patient.name.is_some());
        
        let name = &patient.name.unwrap()[0];
        assert_eq!(name.family, Some("Smith".to_string()));
        assert_eq!(name.given, Some(vec!["Jane".to_string()]));
    }

    #[test]
    fn test_observation_serialization() {
        let observation = Observation {
            resource: Resource {
                resource_type: ResourceType::Observation,
                id: Some("obs-123".to_string()),
                meta: None,
            },
            status: "final".to_string(),
            code: CodeableConcept {
                coding: Some(vec![Coding {
                    system: Some("http://loinc.org".to_string()),
                    code: Some("8302-2".to_string()),
                    display: Some("Body height".to_string()),
                    version: None,
                }]),
                text: Some("Height".to_string()),
            },
            subject: Some(Reference {
                reference: Some("Patient/patient-123".to_string()),
                type_: Some("Patient".to_string()),
                display: Some("John Doe".to_string()),
            }),
        };

        let json = serde_json::to_string(&observation).expect("Failed to serialize observation");
        assert!(json.contains("Observation"));
        assert!(json.contains("final"));
        assert!(json.contains("8302-2"));
    }

    #[test]
    fn test_observation_deserialization() {
        let json = r#"{
            "resourceType": "Observation",
            "id": "obs-456",
            "status": "preliminary",
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "29463-7",
                    "display": "Body Weight"
                }],
                "text": "Weight"
            },
            "subject": {
                "reference": "Patient/patient-456"
            }
        }"#;

        let observation: Observation = serde_json::from_str(json).expect("Failed to deserialize observation");
        assert_eq!(observation.resource.resource_type, ResourceType::Observation);
        assert_eq!(observation.status, "preliminary");
        assert_eq!(observation.code.text, Some("Weight".to_string()));
        assert!(observation.code.coding.is_some());
        
        let coding = &observation.code.coding.unwrap()[0];
        assert_eq!(coding.code, Some("29463-7".to_string()));
    }

    #[test]
    fn test_structure_definition_serialization() {
        let structure_def = StructureDefinition {
            resource: Resource {
                resource_type: ResourceType::StructureDefinition,
                id: Some("custom-patient".to_string()),
                meta: None,
            },
            url: "http://example.org/StructureDefinition/CustomPatient".to_string(),
            version: Some("1.0.0".to_string()),
            name: "CustomPatient".to_string(),
            title: Some("Custom Patient Profile".to_string()),
            status: "active".to_string(),
            experimental: Some(false),
            type_: "Patient".to_string(),
            snapshot: None,
        };

        let json = serde_json::to_string(&structure_def).expect("Failed to serialize structure definition");
        assert!(json.contains("StructureDefinition"));
        assert!(json.contains("CustomPatient"));
        assert!(json.contains("http://example.org"));
    }

    #[test]
    fn test_fhir_resource_trait() {
        let patient = Patient {
            resource: Resource {
                resource_type: ResourceType::Patient,
                id: Some("test-patient".to_string()),
                meta: None,
            },
            active: None,
            name: None,
        };

        assert_eq!(patient.resource_type(), "Patient");
        assert_eq!(patient.id(), Some("test-patient"));
    }

    #[test]
    fn test_resource_type_serialization() {
        let resource_types = vec![
            ResourceType::Patient,
            ResourceType::Observation,
            ResourceType::Practitioner,
            ResourceType::Organization,
            ResourceType::StructureDefinition,
            ResourceType::ValueSet,
            ResourceType::CodeSystem,
        ];

        for resource_type in resource_types {
            let json = serde_json::to_string(&resource_type).expect("Failed to serialize resource type");
            let deserialized: ResourceType = serde_json::from_str(&json).expect("Failed to deserialize resource type");
            assert_eq!(resource_type, deserialized);
        }
    }

    #[test]
    fn test_invalid_json_parsing() {
        let invalid_json = r#"{"invalid": "json"}"#;
        
        let result: Result<Patient, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        
        let result: Result<Observation, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_optional_fields() {
        let minimal_patient = Patient {
            resource: Resource {
                resource_type: ResourceType::Patient,
                id: None,
                meta: None,
            },
            active: None,
            name: None,
        };

        let json = serde_json::to_string(&minimal_patient).expect("Failed to serialize minimal patient");
        let deserialized: Patient = serde_json::from_str(&json).expect("Failed to deserialize minimal patient");
        
        assert_eq!(deserialized.resource.resource_type, ResourceType::Patient);
        assert_eq!(deserialized.resource.id, None);
        assert_eq!(deserialized.active, None);
        assert_eq!(deserialized.name, None);
    }
}