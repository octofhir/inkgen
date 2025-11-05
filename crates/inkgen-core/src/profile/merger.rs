//! Profile merger for combining base and differential constraints

use crate::ir::{ResourceIR, ElementNode, ElementDefinition, ElementType, ResourceMetadata, ResourceKind, DerivationType, ElementTree};
use crate::{CoreError, Result, StructureDefinition};
use tracing::{debug, warn, instrument};

/// Handles merging of base profiles with differential constraints
pub struct ProfileMerger {
    base: ResourceIR,
}

impl ProfileMerger {
    /// Create a new ProfileMerger with a base ResourceIR
    pub fn new(base: ResourceIR) -> Self {
        Self { base }
    }

    /// Merge differential constraints with the base definition
    #[instrument(skip(self, differential))]
    pub fn merge(&self, differential: &StructureDefinition) -> Result<ResourceIR> {
        debug!("Starting profile merge operation for: {}", differential.name);

        // Create a new ResourceIR based on the differential
        let mut merged_ir = self.create_merged_metadata(differential)?;

        // Start with base elements
        merged_ir.elements = self.base.elements.clone();

        // Apply differential constraints if snapshot exists
        if let Some(snapshot) = &differential.snapshot {
            self.apply_differential_elements(&mut merged_ir, &snapshot.element)?;
        }

        // Copy bindings and invariants from base
        merged_ir.bindings = self.base.bindings.clone();
        merged_ir.invariants = self.base.invariants.clone();

        debug!("Profile merge completed for: {}", differential.name);
        Ok(merged_ir)
    }

    /// Create merged metadata from differential StructureDefinition
    fn create_merged_metadata(&self, differential: &StructureDefinition) -> Result<ResourceIR> {
        let metadata = ResourceMetadata {
            name: differential.name.clone(),
            description: differential.title.clone(),
            kind: ResourceKind::Resource,
            base_definition: Some(self.base.metadata.base_definition.clone().unwrap_or_else(|| {
                format!("http://hl7.org/fhir/StructureDefinition/{}", differential.type_)
            })),
            derivation: DerivationType::Constraint, // Profiles are constraints
        };

        // Create empty element tree - will be populated by differential application
        let root_element = ElementNode::new(
            differential.type_.clone(),
            ElementDefinition {
                min: 1,
                max: "1".to_string(),
                types: vec![ElementType {
                    code: differential.type_.clone(),
                    profile: Some(differential.url.clone()),
                    target_profile: None,
                }],
                must_support: false,
                short: differential.title.clone(),
                definition: Some(format!("Profile definition for {}", differential.name)),
                comment: None,
            },
        );

        let elements = ElementTree::new(root_element);
        Ok(ResourceIR::new(metadata, elements))
    }

    /// Apply differential elements to the merged IR
    fn apply_differential_elements(&self, merged_ir: &mut ResourceIR, differential_elements: &[crate::fhir::ElementDefinition]) -> Result<()> {
        debug!("Applying {} differential elements", differential_elements.len());

        for diff_element in differential_elements {
            self.apply_single_differential_element(merged_ir, diff_element)?;
        }

        Ok(())
    }

    /// Apply a single differential element
    fn apply_single_differential_element(&self, merged_ir: &mut ResourceIR, diff_element: &crate::fhir::ElementDefinition) -> Result<()> {
        let path = &diff_element.path;
        
        // Check if element exists in base
        if let Some(base_element) = self.base.elements.get_element(path) {
            // Merge with existing base element
            let merged_element = self.merge_element_definitions(base_element, diff_element)?;
            merged_ir.elements.add_element(merged_element);
        } else {
            // New element introduced by differential
            let new_element = self.create_element_from_differential(diff_element)?;
            merged_ir.elements.add_element(new_element);
        }

        Ok(())
    }

    /// Merge base and differential element definitions
    fn merge_element_definitions(&self, base_element: &ElementNode, diff_element: &crate::fhir::ElementDefinition) -> Result<ElementNode> {
        debug!("Merging element: {}", diff_element.path);

        let mut merged_definition = base_element.definition.clone();

        // Apply cardinality constraints
        if let Some(diff_min) = diff_element.min {
            // Differential min must be >= base min
            if diff_min >= merged_definition.min {
                merged_definition.min = diff_min;
            } else {
                warn!("Differential min ({}) is less than base min ({}) for path: {}", 
                      diff_min, merged_definition.min, diff_element.path);
            }
        }

        if let Some(ref diff_max) = diff_element.max {
            // Apply max constraint (differential can only be more restrictive)
            merged_definition.max = self.merge_max_cardinality(&merged_definition.max, diff_max)?;
        }

        // Apply type constraints
        if let Some(ref diff_types) = diff_element.type_ {
            merged_definition.types = self.merge_element_types(&merged_definition.types, diff_types)?;
        }

        // Handle must-support flag propagation
        // If differential doesn't specify must-support, inherit from base
        // If differential specifies must-support, it takes precedence
        // Note: In real FHIR, must-support is typically in extensions, but we'll handle it simply here

        let mut merged_element = ElementNode::new(diff_element.path.clone(), merged_definition);
        merged_element.children = base_element.children.clone();
        merged_element.slicing = base_element.slicing.clone();

        Ok(merged_element)
    }

    /// Create new element from differential (not in base)
    fn create_element_from_differential(&self, diff_element: &crate::fhir::ElementDefinition) -> Result<ElementNode> {
        debug!("Creating new element from differential: {}", diff_element.path);

        let types = if let Some(diff_types) = &diff_element.type_ {
            diff_types.iter().map(|t| ElementType {
                code: t.code.clone(),
                profile: t.profile.clone().and_then(|profiles| profiles.first().cloned()),
                target_profile: None,
            }).collect()
        } else {
            vec![ElementType {
                code: "Element".to_string(),
                profile: None,
                target_profile: None,
            }]
        };

        let element_definition = ElementDefinition {
            min: diff_element.min.unwrap_or(0),
            max: diff_element.max.clone().unwrap_or("*".to_string()),
            types,
            must_support: false, // Would be extracted from extensions in real implementation
            short: None,
            definition: None,
            comment: None,
        };

        Ok(ElementNode::new(diff_element.path.clone(), element_definition))
    }

    /// Merge max cardinality constraints
    fn merge_max_cardinality(&self, base_max: &str, diff_max: &str) -> Result<String> {
        // If either is unbounded (*), use the more restrictive one
        match (base_max, diff_max) {
            ("*", other) | (other, "*") => Ok(other.to_string()),
            (base, diff) => {
                let base_num: u32 = base.parse().map_err(|_| CoreError::InvalidStructure {
                    message: format!("Invalid base max cardinality: {}", base),
                    resource_type: Some("ElementDefinition".to_string()),
                    element_path: Some("max".to_string()),
                })?;
                let diff_num: u32 = diff.parse().map_err(|_| CoreError::InvalidStructure {
                    message: format!("Invalid differential max cardinality: {}", diff),
                    resource_type: Some("ElementDefinition".to_string()),
                    element_path: Some("max".to_string()),
                })?;
                
                // Differential can only be more restrictive (smaller or equal)
                if diff_num <= base_num {
                    Ok(diff.to_string())
                } else {
                    warn!("Differential max ({}) is greater than base max ({}) - using base", diff_num, base_num);
                    Ok(base.to_string())
                }
            }
        }
    }

    /// Merge element types from base and differential
    fn merge_element_types(&self, base_types: &[ElementType], diff_types: &[crate::fhir::ElementDefinitionType]) -> Result<Vec<ElementType>> {
        let mut merged_types = Vec::new();

        // Convert differential types to IR format
        for diff_type in diff_types {
            let ir_type = ElementType {
                code: diff_type.code.clone(),
                profile: diff_type.profile.clone().and_then(|profiles| profiles.first().cloned()),
                target_profile: None,
            };

            // Check if this type is compatible with base types
            let is_compatible = base_types.iter().any(|base_type| {
                self.are_types_compatible(&base_type.code, &ir_type.code)
            });

            if is_compatible {
                merged_types.push(ir_type);
            } else {
                warn!("Differential type {} is not compatible with base types for merging", diff_type.code);
            }
        }

        // If no compatible types found, fall back to base types
        if merged_types.is_empty() {
            merged_types = base_types.to_vec();
        }

        Ok(merged_types)
    }

    /// Check if two FHIR types are compatible for merging
    fn are_types_compatible(&self, base_type: &str, diff_type: &str) -> bool {
        // Exact match
        if base_type == diff_type {
            return true;
        }

        // Check for inheritance relationships (simplified)
        match (base_type, diff_type) {
            // Exact match
            (base, diff) if base == diff => true,
            // Element is the base of all types (but not exact match)
            ("Element", _) => true,
            // Resource types
            ("Resource", "DomainResource") => true,
            ("DomainResource", resource_type) if self.is_domain_resource_type(resource_type) => true,
            // Primitive types
            ("string", "code") | ("string", "id") | ("string", "uri") => true,
            ("integer", "positiveInt") | ("integer", "unsignedInt") => true,
            // Default: not compatible
            _ => false,
        }
    }

    /// Check if a type is a domain resource type
    fn is_domain_resource_type(&self, type_name: &str) -> bool {
        matches!(type_name, 
            "Patient" | "Observation" | "Practitioner" | "Organization" | 
            "Condition" | "Procedure" | "MedicationRequest" | "DiagnosticReport" |
            "Encounter" | "AllergyIntolerance" | "CarePlan" | "Goal" |
            "Immunization" | "Location" | "Device" | "Medication"
        )
    }

    /// Static method to merge base and differential StructureDefinitions
    pub fn merge_structure_definitions(base: ResourceIR, differential: &StructureDefinition) -> Result<ResourceIR> {
        let merger = ProfileMerger::new(base);
        merger.merge(differential)
    }

    /// Handle must-support flag propagation
    fn propagate_must_support(&self, element: &mut ElementNode, is_must_support: bool) {
        element.definition.must_support = is_must_support;
        
        // In FHIR, if a parent element is must-support, it doesn't automatically make children must-support
        // But if a child is must-support, the path to it should be supported
        // This is a simplified implementation
        debug!("Set must-support to {} for element: {}", is_must_support, element.path);
    }

    /// Validate merged profile for consistency
    pub fn validate_merged_profile(&self, merged_ir: &ResourceIR) -> Result<()> {
        debug!("Validating merged profile: {}", merged_ir.metadata.name);

        // Check for cardinality conflicts
        for (path, element) in &merged_ir.elements.elements {
            if element.definition.min > 1 && element.definition.max == "0" {
                return Err(CoreError::ProfileMergeConflict {
                    path: path.clone(),
                    details: format!("Invalid cardinality: min {} > max {}", element.definition.min, element.definition.max),
                    base_profile: None,
                    differential_profile: None,
                });
            }

            if element.definition.max != "*" {
                if let Ok(max_num) = element.definition.max.parse::<u32>() {
                    if element.definition.min > max_num {
                        return Err(CoreError::ProfileMergeConflict {
                            path: path.clone(),
                            details: format!("Min cardinality {} exceeds max cardinality {}", element.definition.min, max_num),
                            base_profile: None,
                            differential_profile: None,
                        });
                    }
                }
            }
        }

        // Check for type consistency
        for (path, element) in &merged_ir.elements.elements {
            if element.definition.types.is_empty() {
                warn!("Element {} has no types defined", path);
            }
        }

        debug!("Profile validation completed successfully");
        Ok(())
    }
}