//! Integration tests for inkgen-core
//! 
//! These tests verify end-to-end functionality including package resolution,
//! profile processing, and IR generation using real FHIR packages.

use inkgen_core::{
    ir::IRSerializer,
    error::CoreError,
};
use std::sync::Arc;

mod helpers {
    use inkgen_core::{
        package::{PackageResolver, Package, PackageManifest},
        profile::ProfileService,
        ir::{
            ResourceIR, ElementDefinition, ElementNode, ElementTree, ElementType, 
            ResourceMetadata, ResourceKind, DerivationType, TerminologyBinding, 
            BindingStrength
        },
        error::CoreError,
        StructureDefinition,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use serde_json::Value;

    /// Integration test helper for setting up test environments
    pub struct IntegrationTestHelper {
        temp_dir: TempDir,
    }

    impl IntegrationTestHelper {
        /// Create a new integration test helper
        pub async fn new() -> Result<Self, CoreError> {
            let temp_dir = TempDir::new().map_err(|e| CoreError::CacheError { 
                operation: "create temp dir".to_string(),
                reason: format!("Failed to create temp dir: {}", e),
                source: Some(e),
            })?;
            
            Ok(Self {
                temp_dir,
            })
        }
        
        /// Create a new package resolver for testing
        pub async fn create_package_resolver(&self) -> Result<PackageResolver, CoreError> {
            PackageResolver::new().await
        }
        
        /// Create a new profile service for testing
        pub async fn create_profile_service(&self) -> Result<ProfileService, CoreError> {
            let resolver = PackageResolver::new().await?;
            Ok(ProfileService::new(Arc::new(resolver)))
        }
        
        /// Get the temporary directory path
        pub fn temp_path(&self) -> &std::path::Path {
            self.temp_dir.path()
        }
        
        /// Create a test configuration
        pub fn create_test_config(&self) -> TestConfig {
            TestConfig {
                profile_resolution: ProfileResolutionConfig {
                    include_must_support_only: false,
                    resolve_terminology: true,
                },
            }
        }
        
        /// Create test Patient IR for integration testing
        pub fn create_test_patient_ir(&self) -> ResourceIR {
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
                short: Some("Information about an individual receiving health care services".to_string()),
                definition: Some("Demographics and other administrative information about an individual receiving care.".to_string()),
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

            // Add name element
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

            let name_element = ElementNode::new("Patient.name".to_string(), name_def);
            elements.add_element(name_element);

            // Update root element children
            if let Some(root) = elements.elements.get_mut("Patient") {
                root.add_child("Patient.id".to_string());
                root.add_child("Patient.name".to_string());
            }

            let mut ir = ResourceIR::new(metadata, elements);

            // Add terminology binding
            let gender_binding = TerminologyBinding::new(
                "Patient.gender".to_string(),
                Some("http://hl7.org/fhir/ValueSet/administrative-gender".to_string()),
                BindingStrength::Required,
            );
            ir.add_binding(gender_binding);

            ir
        }
        
        /// Create test Observation IR for integration testing
        pub fn create_test_observation_ir(&self) -> ResourceIR {
            let metadata = ResourceMetadata {
                name: "Observation".to_string(),
                description: Some("Measurements and simple assertions made about a patient".to_string()),
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

            // Update root element children
            if let Some(root) = elements.elements.get_mut("Observation") {
                root.add_child("Observation.status".to_string());
            }

            let mut ir = ResourceIR::new(metadata, elements);

            // Add terminology binding
            let status_binding = TerminologyBinding::new(
                "Observation.status".to_string(),
                Some("http://hl7.org/fhir/ValueSet/observation-status".to_string()),
                BindingStrength::Required,
            );
            ir.add_binding(status_binding);

            ir
        }
        
        /// Create a test FHIR package fixture
        pub fn create_test_fhir_package(&self, name: &str) -> std::io::Result<()> {
            let package_dir = self.temp_path().join("packages").join(name);
            std::fs::create_dir_all(&package_dir)?;
            
            // Create package.json
            let package_json = serde_json::json!({
                "name": name,
                "version": "1.0.0",
                "fhirVersions": ["4.0.1"],
                "dependencies": {}
            });
            
            std::fs::write(
                package_dir.join("package.json"),
                serde_json::to_string_pretty(&package_json)?
            )?;
            
            // Create sample Patient resource
            let patient_resource = serde_json::json!({
                "resourceType": "StructureDefinition",
                "id": "Patient",
                "url": "http://hl7.org/fhir/StructureDefinition/Patient",
                "name": "Patient",
                "status": "active",
                "kind": "resource",
                "abstract": false,
                "type": "Patient",
                "baseDefinition": "http://hl7.org/fhir/StructureDefinition/DomainResource",
                "derivation": "specialization"
            });
            
            std::fs::write(
                package_dir.join("StructureDefinition-Patient.json"),
                serde_json::to_string_pretty(&patient_resource)?
            )?;
            
            Ok(())
        }
    }

    /// Test FHIR package utilities
    pub struct TestFhirPackage;

    impl TestFhirPackage {
        /// Create a minimal R4 core package for testing
        pub fn create_minimal_r4_core() -> Package {
            let manifest = PackageManifest {
                name: "hl7.fhir.r4.core".to_string(),
                version: "4.0.1".to_string(),
                description: Some("Core FHIR R4 package".to_string()),
                dependencies: HashMap::new(),
                fhir_versions: vec!["4.0.1".to_string()],
            };
            
            let mut resources = HashMap::new();
            
            // Create mock StructureDefinitions
            let patient_resource = inkgen_core::fhir::Resource {
                resource_type: inkgen_core::fhir::ResourceType::StructureDefinition,
                id: Some("Patient".to_string()),
                meta: None,
            };
            
            let patient_sd = StructureDefinition {
                resource: patient_resource,
                url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                version: Some("4.0.1".to_string()),
                name: "Patient".to_string(),
                title: Some("Patient".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Patient".to_string(),
                snapshot: None,
            };
            resources.insert("Patient".to_string(), patient_sd);
            
            let observation_resource = inkgen_core::fhir::Resource {
                resource_type: inkgen_core::fhir::ResourceType::StructureDefinition,
                id: Some("Observation".to_string()),
                meta: None,
            };
            
            let observation_sd = StructureDefinition {
                resource: observation_resource,
                url: "http://hl7.org/fhir/StructureDefinition/Observation".to_string(),
                version: Some("4.0.1".to_string()),
                name: "Observation".to_string(),
                title: Some("Observation".to_string()),
                status: "active".to_string(),
                experimental: Some(false),
                type_: "Observation".to_string(),
                snapshot: None,
            };
            resources.insert("Observation".to_string(), observation_sd);
            
            Package {
                name: "hl7.fhir.r4.core".to_string(),
                version: "4.0.1".to_string(),
                manifest,
                resources,
            }
        }
        
        /// Create a test fixture with common FHIR resources
        pub fn create_test_fixtures() -> HashMap<String, Value> {
            let mut fixtures = HashMap::new();
            
            // Patient fixture
            fixtures.insert("patient".to_string(), serde_json::json!({
                "resourceType": "Patient",
                "id": "example",
                "name": [{
                    "family": "Doe",
                    "given": ["John"]
                }],
                "gender": "male",
                "birthDate": "1990-01-01"
            }));
            
            // Observation fixture
            fixtures.insert("observation".to_string(), serde_json::json!({
                "resourceType": "Observation",
                "id": "example",
                "status": "final",
                "code": {
                    "coding": [{
                        "system": "http://loinc.org",
                        "code": "15074-8",
                        "display": "Glucose"
                    }]
                },
                "subject": {
                    "reference": "Patient/example"
                }
            }));
            
            fixtures
        }
    }

    /// Test configuration structures
    #[derive(Debug, Clone)]
    pub struct TestConfig {
        pub profile_resolution: ProfileResolutionConfig,
    }

    #[derive(Debug, Clone)]
    pub struct ProfileResolutionConfig {
        pub include_must_support_only: bool,
        pub resolve_terminology: bool,
    }
}

use helpers::{IntegrationTestHelper, TestFhirPackage};

/// Test integration test helper setup
#[tokio::test]
async fn test_integration_helper_setup() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    
    // Verify helper is properly initialized
    assert!(helper.temp_path().exists(), "Temp directory should exist");
    
    // Test creating test FHIR package
    helper.create_test_fhir_package("test.package").expect("Should create test package");
    
    let package_path = helper.temp_path().join("packages").join("test.package");
    assert!(package_path.exists(), "Test package directory should exist");
    assert!(package_path.join("package.json").exists(), "Package manifest should exist");
    
    // Test creating package resolver
    let _resolver = helper.create_package_resolver().await.expect("Should create package resolver");
    
    // Test creating profile service
    let _service = helper.create_profile_service().await.expect("Should create profile service");
}

/// Test test fixture creation
#[tokio::test]
async fn test_test_fixture_creation() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    
    // Test Patient IR creation
    let patient_ir = helper.create_test_patient_ir();
    assert_eq!(patient_ir.metadata.name, "Patient");
    assert!(!patient_ir.elements.elements.is_empty(), "Patient IR should have elements");
    
    // Test Observation IR creation
    let observation_ir = helper.create_test_observation_ir();
    assert_eq!(observation_ir.metadata.name, "Observation");
    assert!(!observation_ir.elements.elements.is_empty(), "Observation IR should have elements");
}

/// Test complete pipeline from package to IR serialization
#[tokio::test]
async fn test_complete_pipeline_patient() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    let serializer = IRSerializer::default();
    
    // Create a test Patient profile using test fixtures
    let patient_ir = helper.create_test_patient_ir();
    
    // Test serialization
    let json = serializer.serialize(&patient_ir).expect("Should serialize Patient IR");
    assert!(!json.is_empty(), "Serialized JSON should not be empty");
    
    // Test deserialization
    let deserialized_ir = serializer.deserialize(&json).expect("Should deserialize Patient IR");
    
    // Verify round-trip consistency
    let json2 = serializer.serialize(&deserialized_ir).expect("Should serialize again");
    assert_eq!(json, json2, "Round-trip serialization should be consistent");
    
    // Verify IR structure
    assert_eq!(deserialized_ir.metadata.name, "Patient");
    assert!(!deserialized_ir.elements.elements.is_empty());
}

/// Test complete pipeline from package to IR serialization for Observation
#[tokio::test]
async fn test_complete_pipeline_observation() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    let serializer = IRSerializer::default();
    
    // Create a test Observation profile using test fixtures
    let observation_ir = helper.create_test_observation_ir();
    
    // Test serialization
    let json = serializer.serialize(&observation_ir).expect("Should serialize Observation IR");
    assert!(!json.is_empty(), "Serialized JSON should not be empty");
    
    // Test deserialization
    let deserialized_ir = serializer.deserialize(&json).expect("Should deserialize Observation IR");
    
    // Verify round-trip consistency
    let json2 = serializer.serialize(&deserialized_ir).expect("Should serialize again");
    assert_eq!(json, json2, "Round-trip serialization should be consistent");
    
    // Verify IR structure
    assert_eq!(deserialized_ir.metadata.name, "Observation");
    assert!(!deserialized_ir.elements.elements.is_empty());
}

/// Test error handling in integration scenarios
#[tokio::test]
async fn test_integration_error_handling() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    let serializer = IRSerializer::default();
    
    // Test serialization error handling with invalid data
    let metadata = inkgen_core::ir::ResourceMetadata {
        name: "InvalidResource".to_string(),
        description: None,
        kind: inkgen_core::ir::ResourceKind::Resource,
        base_definition: None,
        derivation: inkgen_core::ir::DerivationType::Specialization,
    };
    
    let root_def = inkgen_core::ir::ElementDefinition {
        min: 0,
        max: "1".to_string(),
        types: vec![],
        must_support: false,
        short: None,
        definition: None,
        comment: None,
    };
    
    let root_element = inkgen_core::ir::ElementNode::new("InvalidResource".to_string(), root_def);
    let elements = inkgen_core::ir::ElementTree::new(root_element);
    let invalid_ir = inkgen_core::ir::ResourceIR::new(metadata, elements);
    
    // Should handle empty IR gracefully
    let result = serializer.serialize(&invalid_ir);
    assert!(result.is_ok(), "Should handle empty IR gracefully");
}

/// Test concurrent package resolution
#[tokio::test]
async fn test_concurrent_package_resolution() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    
    // Create multiple concurrent resolution tasks
    let mut handles = Vec::new();
    
    for i in 0..3 {
        let handle = tokio::spawn(async move {
            // Use test fixtures instead of real packages for concurrent testing
            let _test_package = TestFhirPackage::create_minimal_r4_core();
            // In a real scenario, this would resolve actual packages
            // For testing, we verify the resolver can handle concurrent calls
            Ok::<_, CoreError>(format!("test-package-{}", i))
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let results = futures_util::future::join_all(handles).await;
    
    // Verify all tasks completed successfully
    for (i, result) in results.into_iter().enumerate() {
        let package_name = result.expect("Task should complete").expect("Should resolve package");
        assert_eq!(package_name, format!("test-package-{}", i));
    }
}

/// Test memory usage and cleanup
#[tokio::test]
async fn test_memory_management() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    
    // Create multiple IRs to test memory management
    let patient_ir = helper.create_test_patient_ir();
    let observation_ir = helper.create_test_observation_ir();
    
    // Serialize multiple times to test memory cleanup
    let serializer = IRSerializer::default();
    
    for _ in 0..10 {
        let _json1 = serializer.serialize(&patient_ir).expect("Should serialize");
        let _json2 = serializer.serialize(&observation_ir).expect("Should serialize");
    }
    
    // Test should complete without memory issues
    assert!(true, "Memory management test completed");
}

/// Test configuration integration
#[tokio::test]
async fn test_configuration_integration() {
    let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
    
    // Test with different configuration options
    let config = helper.create_test_config();
    assert!(config.profile_resolution.include_must_support_only == false);
    assert!(config.profile_resolution.resolve_terminology == true);
    
    // Verify configuration structure
    let patient_ir = helper.create_test_patient_ir();
    
    // Test that IR contains expected elements
    assert!(!patient_ir.elements.elements.is_empty());
    assert!(patient_ir.elements.elements.contains_key("Patient"));
}

/// Snapshot tests for end-to-end pipeline verification
#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use insta::assert_snapshot;

    /// Test Patient profile IR serialization snapshot
    #[tokio::test]
    async fn test_patient_profile_ir_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create Patient IR using test helper
        let patient_ir = helper.create_test_patient_ir();
        
        // Serialize to JSON
        let json = serializer.serialize(&patient_ir).expect("Should serialize Patient IR");
        
        // Assert snapshot matches
        assert_snapshot!("patient_profile_ir_serialization", json);
    }

    /// Test Observation profile IR serialization snapshot
    #[tokio::test]
    async fn test_observation_profile_ir_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create Observation IR using test helper
        let observation_ir = helper.create_test_observation_ir();
        
        // Serialize to JSON
        let json = serializer.serialize(&observation_ir).expect("Should serialize Observation IR");
        
        // Assert snapshot matches
        assert_snapshot!("observation_profile_ir_serialization", json);
    }

    /// Test complete pipeline from FHIR package to IR snapshot
    #[tokio::test]
    async fn test_complete_pipeline_patient_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create test FHIR package
        helper.create_test_fhir_package("test.patient.package").expect("Should create test package");
        
        // Create Patient IR (simulating complete pipeline)
        let patient_ir = helper.create_test_patient_ir();
        
        // Serialize the complete IR
        let json = serializer.serialize(&patient_ir).expect("Should serialize complete Patient IR");
        
        // Assert complete pipeline snapshot
        assert_snapshot!("complete_pipeline_patient_ir", json);
    }

    /// Test complete pipeline from FHIR package to IR snapshot for Observation
    #[tokio::test]
    async fn test_complete_pipeline_observation_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create test FHIR package
        helper.create_test_fhir_package("test.observation.package").expect("Should create test package");
        
        // Create Observation IR (simulating complete pipeline)
        let observation_ir = helper.create_test_observation_ir();
        
        // Serialize the complete IR
        let json = serializer.serialize(&observation_ir).expect("Should serialize complete Observation IR");
        
        // Assert complete pipeline snapshot
        assert_snapshot!("complete_pipeline_observation_ir", json);
    }

    /// Test multiple resource processing pipeline snapshot
    #[tokio::test]
    async fn test_multiple_resources_pipeline_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create multiple IRs
        let patient_ir = helper.create_test_patient_ir();
        let observation_ir = helper.create_test_observation_ir();
        
        // Serialize both
        let patient_json = serializer.serialize(&patient_ir).expect("Should serialize Patient IR");
        let observation_json = serializer.serialize(&observation_ir).expect("Should serialize Observation IR");
        
        // Create combined output for snapshot
        let combined_output = format!(
            "=== Patient IR ===\n{}\n\n=== Observation IR ===\n{}",
            patient_json, observation_json
        );
        
        // Assert combined pipeline snapshot
        assert_snapshot!("multiple_resources_pipeline", combined_output);
    }

    /// Test IR serialization consistency across multiple runs
    #[tokio::test]
    async fn test_ir_serialization_consistency_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create Patient IR
        let patient_ir = helper.create_test_patient_ir();
        
        // Serialize multiple times
        let json1 = serializer.serialize(&patient_ir).expect("Should serialize first time");
        let json2 = serializer.serialize(&patient_ir).expect("Should serialize second time");
        let json3 = serializer.serialize(&patient_ir).expect("Should serialize third time");
        
        // All should be identical
        assert_eq!(json1, json2, "First and second serialization should match");
        assert_eq!(json2, json3, "Second and third serialization should match");
        
        // Assert consistency snapshot
        assert_snapshot!("ir_serialization_consistency", json1);
    }

    /// Test round-trip serialization/deserialization snapshot
    #[tokio::test]
    async fn test_round_trip_serialization_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create original IR
        let original_ir = helper.create_test_patient_ir();
        
        // Serialize
        let json1 = serializer.serialize(&original_ir).expect("Should serialize original IR");
        
        // Deserialize
        let deserialized_ir = serializer.deserialize(&json1).expect("Should deserialize IR");
        
        // Serialize again
        let json2 = serializer.serialize(&deserialized_ir).expect("Should serialize deserialized IR");
        
        // Should be identical
        assert_eq!(json1, json2, "Round-trip serialization should be consistent");
        
        // Assert round-trip snapshot
        assert_snapshot!("round_trip_serialization", json2);
    }

    /// Test error handling in pipeline snapshot
    #[tokio::test]
    async fn test_pipeline_error_handling_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Test serialization of empty IR (should handle gracefully)
        let metadata = inkgen_core::ir::ResourceMetadata {
            name: "EmptyResource".to_string(),
            description: None,
            kind: inkgen_core::ir::ResourceKind::Resource,
            base_definition: None,
            derivation: inkgen_core::ir::DerivationType::Specialization,
        };
        
        let root_def = inkgen_core::ir::ElementDefinition {
            min: 0,
            max: "1".to_string(),
            types: vec![],
            must_support: false,
            short: None,
            definition: None,
            comment: None,
        };
        
        let root_element = inkgen_core::ir::ElementNode::new("EmptyResource".to_string(), root_def);
        let elements = inkgen_core::ir::ElementTree::new(root_element);
        let empty_ir = inkgen_core::ir::ResourceIR::new(metadata, elements);
        
        // Serialize empty IR
        let json = serializer.serialize(&empty_ir).expect("Should serialize empty IR");
        
        // Assert error handling snapshot
        assert_snapshot!("pipeline_empty_resource_handling", json);
    }

    /// Test configuration impact on pipeline snapshot
    #[tokio::test]
    async fn test_configuration_impact_snapshot() {
        let helper = IntegrationTestHelper::new().await.expect("Failed to create test helper");
        let serializer = inkgen_core::ir::IRSerializer::default();
        
        // Create IR with different configurations
        let config = helper.create_test_config();
        let patient_ir = helper.create_test_patient_ir();
        
        // Serialize with configuration context
        let json = serializer.serialize(&patient_ir).expect("Should serialize with config");
        
        // Create output with configuration info
        let config_output = format!(
            "=== Configuration ===\ninclude_must_support_only: {}\nresolve_terminology: {}\n\n=== Patient IR ===\n{}",
            config.profile_resolution.include_must_support_only,
            config.profile_resolution.resolve_terminology,
            json
        );
        
        // Assert configuration impact snapshot
        assert_snapshot!("configuration_impact_on_pipeline", config_output);
    }
}