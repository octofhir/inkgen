//! TypeScript code generator implementation

use inkgen_core::{Result, FhirResource, StructureDefinition};
use tera::{Tera, Context};
use crate::templates;

/// Language generator trait for code generation backends
pub trait LanguageGenerator {
    /// Generate code for a FHIR resource
    fn generate_resource(&self, resource: &dyn FhirResource) -> Result<String>;
    
    /// Generate code for a FHIR StructureDefinition (profile)
    fn generate_profile(&self, profile: &StructureDefinition) -> Result<String>;
    
    /// Generate a complete SDK package
    fn generate_package(&self, resources: &[&dyn FhirResource]) -> Result<String>;
}

/// TypeScript code generator
pub struct TypeScriptGenerator {
    /// Tera template engine
    tera: Tera,
}

impl TypeScriptGenerator {
    /// Create a new TypeScript generator
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();
        
        // Register built-in templates
        tera.add_raw_template("interface", templates::INTERFACE_TEMPLATE)?;
        tera.add_raw_template("resource", templates::RESOURCE_TEMPLATE)?;
        tera.add_raw_template("package", templates::PACKAGE_TEMPLATE)?;
        
        Ok(Self { tera })
    }
    
    /// Generate TypeScript interface from FHIR resource type
    fn generate_interface(&self, resource_type: &str) -> Result<String> {
        let mut context = Context::new();
        context.insert("resource_type", resource_type);
        context.insert("interface_name", &format!("{}Resource", resource_type));
        
        let output = self.tera.render("interface", &context)?;
        Ok(output)
    }
    
    /// Generate TypeScript code for a specific resource
    fn generate_resource_code(&self, resource: &dyn FhirResource) -> Result<String> {
        let mut context = Context::new();
        context.insert("resource_type", resource.resource_type());
        context.insert("resource_id", &resource.id().unwrap_or("unknown"));
        
        let output = self.tera.render("resource", &context)?;
        Ok(output)
    }
}

impl LanguageGenerator for TypeScriptGenerator {
    fn generate_resource(&self, resource: &dyn FhirResource) -> Result<String> {
        self.generate_resource_code(resource)
    }
    
    fn generate_profile(&self, profile: &StructureDefinition) -> Result<String> {
        let mut context = Context::new();
        context.insert("profile_name", &profile.name);
        context.insert("profile_type", &profile.type_);
        context.insert("profile_url", &profile.url);
        
        // Generate TypeScript interface for the profile
        let output = self.tera.render("interface", &context)?;
        Ok(output)
    }
    
    fn generate_package(&self, resources: &[&dyn FhirResource]) -> Result<String> {
        let mut context = Context::new();
        
        // Collect resource types
        let resource_types: Vec<&str> = resources
            .iter()
            .map(|r| r.resource_type())
            .collect();
        
        context.insert("resource_types", &resource_types);
        context.insert("resource_count", &resources.len());
        
        let output = self.tera.render("package", &context)?;
        Ok(output)
    }
}

impl Default for TypeScriptGenerator {
    fn default() -> Self {
        Self::new().expect("Failed to create TypeScript generator")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkgen_core::{Patient, Resource, ResourceType};

    #[test]
    fn test_generator_creation() {
        let generator = TypeScriptGenerator::new();
        assert!(generator.is_ok());
    }

    #[test]
    fn test_interface_generation() {
        let generator = TypeScriptGenerator::new().unwrap();
        let result = generator.generate_interface("Patient");
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("PatientResource"));
    }

    #[test]
    fn test_resource_generation() {
        let generator = TypeScriptGenerator::new().unwrap();
        
        let patient = Patient {
            resource: Resource {
                resource_type: ResourceType::Patient,
                id: Some("test-patient".to_string()),
                meta: None,
            },
            active: Some(true),
            name: None,
        };
        
        let result = generator.generate_resource(&patient);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("Patient"));
    }
}