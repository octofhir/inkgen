//! TypeScript code generation templates

/// Template for generating TypeScript interfaces
pub const INTERFACE_TEMPLATE: &str = r#"
/**
 * Generated TypeScript interface for {{ resource_type | default(value="Unknown") }}
 */
export interface {{ interface_name | default(value="UnknownResource") }} {
  /** Resource type identifier */
  resourceType: "{{ resource_type | default(value="Unknown") }}";
  
  /** Logical id of this artifact */
  id?: string;
  
  /** Metadata about the resource */
  meta?: {
    versionId?: string;
    lastUpdated?: string;
    profile?: string[];
  };
}
"#;

/// Template for generating TypeScript resource implementations
pub const RESOURCE_TEMPLATE: &str = r#"
/**
 * Generated TypeScript resource for {{ resource_type | default(value="Unknown") }}
 * Resource ID: {{ resource_id | default(value="unknown") }}
 */
export const {{ resource_type | default(value="Unknown") | lower }}Resource = {
  resourceType: "{{ resource_type | default(value="Unknown") }}" as const,
  id: "{{ resource_id | default(value="unknown") }}",
  
  // Add resource-specific properties here
} satisfies {{ resource_type | default(value="Unknown") }}Resource;
"#;

/// Template for generating complete TypeScript packages
pub const PACKAGE_TEMPLATE: &str = r#"
/**
 * Generated TypeScript SDK Package
 * Contains {{ resource_count | default(value=0) }} FHIR resources
 */

// Export all resource types
{% for resource_type in resource_types -%}
export type { {{ resource_type }}Resource } from './{{ resource_type | lower }}';
{% endfor %}

// Export utility types
export interface FhirResource {
  resourceType: string;
  id?: string;
  meta?: {
    versionId?: string;
    lastUpdated?: string;
    profile?: string[];
  };
}

// Resource type union
export type ResourceType = {% for resource_type in resource_types %}"{{ resource_type }}"{% if not loop.last %} | {% endif %}{% endfor %};

// Package metadata
export const PACKAGE_INFO = {
  name: "fhir-typescript-sdk",
  version: "1.0.0",
  resourceCount: {{ resource_count | default(value=0) }},
  resourceTypes: [{% for resource_type in resource_types %}"{{ resource_type }}"{% if not loop.last %}, {% endif %}{% endfor %}],
  generatedAt: new Date().toISOString(),
} as const;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tera::{Tera, Context};

    #[test]
    fn test_interface_template() {
        let mut tera = Tera::default();
        tera.add_raw_template("interface", INTERFACE_TEMPLATE).unwrap();
        
        let mut context = Context::new();
        context.insert("resource_type", "Patient");
        context.insert("interface_name", "PatientResource");
        
        let result = tera.render("interface", &context);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("PatientResource"));
        assert!(output.contains("resourceType: \"Patient\""));
    }

    #[test]
    fn test_resource_template() {
        let mut tera = Tera::default();
        tera.add_raw_template("resource", RESOURCE_TEMPLATE).unwrap();
        
        let mut context = Context::new();
        context.insert("resource_type", "Patient");
        context.insert("resource_id", "test-123");
        
        let result = tera.render("resource", &context);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("patientResource"));
        assert!(output.contains("id: \"test-123\""));
    }

    #[test]
    fn test_package_template() {
        let mut tera = Tera::default();
        tera.add_raw_template("package", PACKAGE_TEMPLATE).unwrap();
        
        let mut context = Context::new();
        context.insert("resource_types", &vec!["Patient", "Observation"]);
        context.insert("resource_count", &2);
        
        let result = tera.render("package", &context);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("PatientResource"));
        assert!(output.contains("ObservationResource"));
        assert!(output.contains("resourceCount: 2"));
    }
}