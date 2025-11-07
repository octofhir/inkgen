//! Language generator trait and interfaces
//!
//! This module defines the core trait that all language-specific code generators must implement.
//! It provides a plugin point for new language backends while maintaining compatibility with
//! the manifest-driven configuration system.

use anyhow::Result;
use async_trait::async_trait;

use crate::{PackageDescriptor, StructureDefinitionProvider, StructureProviderConfig};

/// Core trait for implementing code generators for different programming languages.
///
/// This trait defines the interface that all language-specific code generators must implement.
/// It provides a single async entry point that orchestrates the entire code generation pipeline
/// for a given FHIR package.
///
/// # Motivation
///
/// The `LanguageGenerator` trait abstracts away the complexity of FHIR code generation,
/// allowing teams to:
/// - Create backends for new programming languages
/// - Customize output through template overlays
/// - Reuse common patterns and utilities across backends
/// - Maintain version compatibility with the core codegen engine
///
/// # Lifecycle
///
/// The `generate` method is responsible for:
/// 1. Reading FHIR structure definitions from the service provider
/// 2. Building semantic models and relationships
/// 3. Rendering output using language-specific templates
/// 4. Writing files to the configured output directory
/// 5. Reporting errors in a standardized way
///
/// # Implementation Example
///
/// Here's a minimal example implementing a simple language generator:
///
/// ```ignore
/// use async_trait::async_trait;
/// use anyhow::Result;
///
/// pub struct MyLanguageGenerator {
///     config: MyLanguageConfig,
/// }
///
/// #[async_trait]
/// impl<S> LanguageGenerator<S> for MyLanguageGenerator
/// where
///     S: StructureDefinitionProvider + Sync + Send,
/// {
///     async fn generate(
///         &self,
///         service: &S,
///         descriptor: &PackageDescriptor,
///         provider_config: &StructureProviderConfig,
///     ) -> Result<()> {
///         // 1. Get all structure definitions for this package
///         let structures = service.structures(provider_config).await?;
///
///         // 2. Group by resource type or other criteria
///         let mut output_groups = HashMap::new();
///         for summary in structures {
///             // Fetch full definition
///             let definition = service.get_structure(&summary.url).await?;
///             // Process and group as needed
///         }
///
///         // 3. For each group, render templates and write output
///         for (group_name, structures) in output_groups {
///             let rendered = self.render_template(&structures)?;
///             let output_path = self.config.output_dir.join(format!("{}.rs", group_name));
///             std::fs::write(output_path, rendered)?;
///         }
///
///         Ok(())
///     }
/// }
/// ```
///
/// # Extension Points
///
/// Implementations can customize behavior in several ways:
/// - **Template System**: Use the Tera template engine with custom filters
/// - **Output Structure**: Choose between flat or hierarchical file organization
/// - **Naming Conventions**: Apply language-specific naming patterns (camelCase, snake_case, etc.)
/// - **Resource Grouping**: Organize output by package, resource type, or custom categories
///
/// # Error Handling
///
/// Implementations must properly propagate errors from the provider using `anyhow::Result`.
/// Common error scenarios:
/// - Missing or invalid structure definitions
/// - Template rendering failures
/// - I/O errors writing output files
/// - Invalid configuration values
///
/// All errors should include context about what operation failed for debugging.
#[async_trait]
pub trait LanguageGenerator<S>
where
    S: StructureDefinitionProvider + Sync + Send,
{
    /// Generate code for all structures in a FHIR package.
    ///
    /// This method is the main entry point for code generation. It receives a structure
    /// definition provider, package metadata, and configuration, then is responsible for
    /// orchestrating the complete code generation pipeline.
    ///
    /// # Arguments
    ///
    /// * `service` - A provider that can fetch FHIR structure definitions on demand.
    ///               Implementations should call `service.structures()` to get a list of
    ///               available structures, then fetch each one as needed.
    ///
    /// * `descriptor` - Metadata about the FHIR package, including name, version, URL,
    ///                  and other canonical information. Use this to:
    ///                  - Set version strings in generated output
    ///                  - Track package lineage and dependencies
    ///                  - Organize output directories by package
    ///
    /// * `provider_config` - Configuration for the structure definition provider, including:
    ///                       - Which structure kinds to include (resources, data types, profiles)
    ///                       - Filtering criteria (by URL pattern, name, etc.)
    ///                       - Resource limits to avoid memory exhaustion
    ///
    /// # Returns
    ///
    /// - `Ok(())` if generation succeeds
    /// - `Err(...)` if any stage of generation fails
    ///
    /// # Errors
    ///
    /// Implementations should return errors in these cases:
    /// - Unable to fetch structures from the provider
    /// - Template file not found or invalid
    /// - Template rendering fails (syntax errors, missing variables)
    /// - Cannot write output files (permissions, disk space)
    /// - Configuration validation fails (invalid paths, incompatible options)
    ///
    /// # Examples
    ///
    /// See the `TypescriptGenerator` for a complete production implementation.
    async fn generate(
        &self,
        service: &S,
        descriptor: &PackageDescriptor,
        provider_config: &StructureProviderConfig,
    ) -> Result<()>;
}
