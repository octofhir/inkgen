//! Profile constraint handling for TypeScript generation.
//!
//! This module provides utilities for generating TypeScript interfaces from FHIR profiles
//! that constrain base resources with additional rules like mustSupport, fixed values,
//! and tightened cardinality.

use std::collections::{HashMap, HashSet};

use inkgen_core::config::{ExtensionAccessorStyle, ProfileMethodConfig};
use inkgen_core::ir::{Derivation, ElementDefinition, ResourceDefinition};
use serde::Serialize;

use crate::imports::{TypeRegistry, calculate_import_path};
use crate::naming;

/// TypeScript reserved words that cannot be used as property names in class declarations.
const TS_RESERVED_WORDS: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "module",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "let",
    "static",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "any",
    "boolean",
    "number",
    "string",
    "symbol",
    "type",
    "namespace",
    "abstract",
    "as",
    "async",
    "await",
    "constructor",
    "declare",
    "from",
    "get",
    "is",
    "of",
    "require",
    "set",
];

/// Sanitize a field name for TypeScript compatibility.
/// - Removes `[x]` suffix from FHIR choice type fields (e.g., "value[x]" -> "value")
/// - Escapes TypeScript reserved words by prefixing with underscore
fn sanitize_field_name(name: &str) -> String {
    // Remove [x] suffix from choice types
    let name = name.strip_suffix("[x]").unwrap_or(name);

    // Check if it's a reserved word
    if TS_RESERVED_WORDS.contains(&name.to_lowercase().as_str()) {
        format!("_{}", name)
    } else {
        name.to_string()
    }
}

/// Context for generating profile TypeScript code with proper imports.
#[derive(Debug, Clone)]
pub struct ProfileGenerationContext<'a> {
    /// Current package folder (e.g., "psychportal")
    pub current_package_folder: &'a str,
    /// Type registry for looking up type locations
    pub type_registry: &'a TypeRegistry,
}

/// Information about a FHIR profile for TypeScript generation.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// TypeScript type name for the profile
    pub type_name: String,
    /// Canonical URL of the profile
    pub canonical_url: String,
    /// Base resource type (e.g., "Patient", "Observation")
    pub base_type: String,
    /// Profile title
    pub title: Option<String>,
    /// Profile description
    pub description: Option<String>,
    /// Elements with mustSupport flag
    pub must_support_elements: Vec<String>,
    /// Elements with fixed values
    pub fixed_elements: Vec<FixedElement>,
    /// Elements with tightened cardinality
    pub constrained_elements: Vec<ConstrainedElement>,
    /// Extensions defined on this profile
    pub extensions: Vec<crate::extensions::RenderExtension>,
}

/// Represents an element with a fixed value in a profile.
#[derive(Debug, Clone)]
pub struct FixedElement {
    /// Element path (e.g., "Patient.active")
    pub path: String,
    /// TypeScript field name
    pub field_name: String,
    /// Fixed value as TypeScript literal
    pub fixed_value: String,
    /// Type of the fixed value
    pub value_type: String,
}

/// Represents an element with tightened cardinality in a profile.
#[derive(Debug, Clone)]
pub struct ConstrainedElement {
    /// Element path
    pub path: String,
    /// TypeScript field name
    pub field_name: String,
    /// Minimum cardinality (0 or 1+)
    pub min: u32,
    /// Maximum cardinality
    pub max: String,
    /// Whether this makes an optional field required
    pub makes_required: bool,
    /// For FHIR choice (`value[x]`) elements: the expanded variant member names
    /// (e.g. `effectiveDateTime`, `effectivePeriod`). Empty for non-choice
    /// elements. A choice element has no single member to make required, so the
    /// emitter narrows only when the profile constrains it to exactly one type.
    pub choice_members: Vec<String>,
}

/// Render context for profile template generation.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileRenderContext {
    /// Profile type name
    pub type_name: String,
    /// Base resource type being extended
    pub base_type: String,
    /// Canonical URL
    pub canonical_url: String,
    /// Profile title
    pub title: Option<String>,
    /// Profile description
    pub description: Option<String>,
    /// Import statements needed
    pub imports: Vec<ImportStatement>,
    /// Fixed value elements
    pub fixed_elements: Vec<FixedElementRender>,
    /// Must-support elements
    pub must_support_elements: Vec<MustSupportElementRender>,
    /// Extension accessors
    pub extension_accessors: Vec<ExtensionAccessor>,
    /// Style of extension accessors (Both, Typed, or Raw)
    pub extension_style: ExtensionAccessorStyle,
    /// Generate Zod schema
    pub generate_zod: bool,
    /// Generate serialization methods
    pub with_serialization: bool,
    /// Generate validation methods
    pub with_validation: bool,
}

/// Import statement for template rendering.
#[derive(Debug, Clone, Serialize)]
pub struct ImportStatement {
    /// Types to import
    pub types: Vec<String>,
    /// Import path
    pub path: String,
}

/// Fixed element for template rendering.
#[derive(Debug, Clone, Serialize)]
pub struct FixedElementRender {
    /// Field name
    pub field_name: String,
    /// Fixed value as literal
    pub fixed_value: String,
}

/// Must-support element for template rendering.
#[derive(Debug, Clone, Serialize)]
pub struct MustSupportElementRender {
    /// Element path
    pub path: String,
    /// Field name
    pub field_name: String,
    /// Zod constraint if applicable
    pub zod_constraint: Option<String>,
}

/// Extension accessor metadata for template rendering.
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionAccessor {
    /// Extension name (for documentation)
    pub name: String,
    /// Extension URL
    pub url: String,
    /// Getter/setter name (e.g., "race" for "raceExtension")
    pub getter_name: String,
    /// TypeScript value type (e.g., "Coding[]", "string | undefined")
    pub value_type: String,
    /// FHIR value field name (e.g., "valueString", "valueCoding")
    pub value_field: String,
    /// Whether this is a complex extension (returns Extension object)
    pub is_complex: bool,
    /// Whether this is an array
    pub is_array: bool,
    /// Extension description
    pub description: Option<String>,
}

impl ProfileInfo {
    /// Creates a ProfileInfo from a ResourceDefinition that represents a profile.
    ///
    /// Returns None if the resource is not a profile (constraint derivation).
    pub fn from_resource_definition(
        definition: &ResourceDefinition,
        all_extensions: &indexmap::IndexMap<String, crate::extensions::RenderExtension>,
    ) -> Option<Self> {
        // Only process profiles (derivation = constraint)
        if !matches!(definition.lineage.derivation, Some(Derivation::Constraint)) {
            return None;
        }

        let base_type = definition
            .lineage
            .base_id
            .clone()
            .or_else(|| definition.lineage.type_name.clone())?;

        // Sanitize type_name - names like "CDS Hooks GuidanceResponse" need to become "CdsHooksGuidanceResponse"
        let raw_name = definition
            .name
            .clone()
            .unwrap_or_else(|| definition.id.clone());
        let type_name = naming::pascal_case(&raw_name);

        let mut must_support_elements = Vec::new();
        let mut fixed_elements = Vec::new();
        let mut constrained_elements = Vec::new();

        tracing::info!(
            "ProfileInfo::from_resource_definition for {} - {} elements to scan",
            type_name,
            definition.elements.len()
        );
        for elem in &definition.elements {
            tracing::info!(
                "  Element: path={}, must_support={}, min={}, max={:?}",
                elem.path,
                elem.must_support,
                elem.cardinality.min,
                elem.cardinality.max
            );
        }

        let mut seen_field_names = std::collections::HashSet::new();
        extract_constraints(
            &definition.elements,
            &base_type,
            &mut must_support_elements,
            &mut fixed_elements,
            &mut constrained_elements,
            &mut seen_field_names,
        );

        tracing::info!(
            "  After extract_constraints: must_support={}, fixed={}, constrained={}",
            must_support_elements.len(),
            fixed_elements.len(),
            constrained_elements.len()
        );

        // Extract extension slices from the profile's definition
        let extensions_map = crate::extensions::extract_extensions(definition);

        // Enrich extensions with information from all_extensions
        let mut enriched_extensions = Vec::new();
        for (url, mut ext) in extensions_map {
            // Look up the full extension definition
            if let Some(full_ext) = all_extensions.get(&url) {
                tracing::info!(
                    "Enriching extension {} with full definition: is_complex={}, nested_types={}",
                    url,
                    full_ext.is_complex,
                    full_ext.nested_types.len()
                );

                // Enrich with full extension information
                ext.is_complex = full_ext.is_complex;
                ext.value_type = full_ext.value_type.clone();
                ext.nested_types = full_ext.nested_types.clone();
            } else {
                tracing::warn!("Extension {} not found in all_extensions map", url);
            }
            enriched_extensions.push(ext);
        }

        let extensions = enriched_extensions;

        Some(Self {
            type_name,
            canonical_url: definition.url.clone(),
            base_type,
            title: definition.title.clone(),
            description: definition.description.clone(),
            must_support_elements,
            fixed_elements,
            constrained_elements,
            extensions,
        })
    }

    /// Generates TypeScript code for this profile.
    ///
    /// Produces an interface or class that extends the base type with profile constraints.
    pub fn generate_typescript(
        &self,
        as_class: bool,
        with_methods: bool,
        with_zod: bool,
        context: Option<&ProfileGenerationContext>,
    ) -> String {
        let mut output = String::new();

        // Generate imports if context is provided
        if let Some(ctx) = context {
            self.generate_imports(&mut output, as_class, with_zod, ctx);
        }

        // Generate flat input types for complex extensions
        for extension in &self.extensions {
            if extension.is_complex && !extension.nested_types.is_empty() {
                self.generate_flat_input_type(&mut output, extension);
                self.generate_converter_functions(&mut output, extension);
            }
        }

        // Add JSDoc comment
        if self.title.is_some() || self.description.is_some() {
            output.push_str("/**\n");
            if let Some(title) = &self.title {
                output.push_str(&format!(" * {}\n", title));
            }
            if let Some(desc) = &self.description {
                if self.title.is_some() {
                    output.push_str(" *\n");
                }
                output.push_str(&format!(" * {}\n", desc));
            }
            output.push_str(&format!(" * @profile {}\n", self.canonical_url));
            output.push_str(" */\n");
        }

        if as_class {
            // Generate class extending base type
            output.push_str(&format!(
                "export class {} extends {} {{\n",
                self.type_name, self.base_type
            ));

            // Add __profile readonly property
            output.push_str("  /** Profile URL for runtime validation */\n");
            output.push_str(&format!(
                "  readonly __profile = '{}';\n\n",
                self.canonical_url
            ));

            // Add fixed value fields (override with specific literals)
            // Skip 'url' for Extension profiles: a string-literal type can't narrow
            // the branded `Extension['url']` (FhirString). Same skip as the
            // interface branch below.
            for fixed in &self.fixed_elements {
                if fixed.field_name == "url" && self.base_type == "Extension" {
                    continue;
                }
                output.push_str(&format!("  /** Fixed value: {} */\n", fixed.fixed_value));
                output.push_str(&format!(
                    "  declare {}: {};\n\n",
                    fixed.field_name, fixed.fixed_value
                ));
            }

            // Add constrained fields (override cardinality with declare)
            for constrained in &self.constrained_elements {
                if constrained.makes_required {
                    if let Some(member) = required_choice_member(constrained) {
                        output.push_str(&format!(
                            "  /** Required by profile (min: {}) */\n",
                            constrained.min
                        ));
                        output.push_str(&format!(
                            "  declare {}: NonNullable<{}['{}']>;\n\n",
                            member, self.base_type, member
                        ));
                    } else if let Some(doc) = unnarrowed_choice_doc(constrained) {
                        output.push_str(&doc);
                    } else {
                        output.push_str(&format!(
                            "  /** Required by profile (min: {}) */\n",
                            constrained.min
                        ));
                        // Use declare to override parent field as required
                        output.push_str(&format!(
                            "  declare {}: NonNullable<{}['{}']>;\n\n",
                            constrained.field_name, self.base_type, constrained.field_name
                        ));
                    }
                }
            }

            // Add extension methods if requested
            if with_methods && !self.extensions.is_empty() {
                output.push_str("  // Extension accessor methods\n\n");
                for extension in &self.extensions {
                    self.generate_extension_methods(&mut output, extension);
                }
            }

            output.push_str("}\n\n");
        } else {
            // Generate interface extending base type
            output.push_str(&format!(
                "export interface {} extends {} {{\n",
                self.type_name, self.base_type
            ));

            // Add __profileUrl metadata field
            output.push_str("  /** Profile URL for runtime validation */\n");
            output.push_str(&format!(
                "  readonly __profileUrl: \"{}\";\n",
                self.canonical_url
            ));

            // Add fixed value fields (override with specific literals)
            // Skip 'url' field for Extension profiles as it conflicts with FhirString type
            for fixed in &self.fixed_elements {
                if fixed.field_name == "url" && self.base_type == "Extension" {
                    // Skip - url is already typed as FhirString in Extension
                    continue;
                }
                output.push_str(&format!("  /** Fixed value: {} */\n", fixed.fixed_value));
                output.push_str(&format!("  {}: {};\n", fixed.field_name, fixed.fixed_value));
            }

            // Add constrained fields (override cardinality)
            for constrained in &self.constrained_elements {
                if constrained.makes_required {
                    if let Some(member) = required_choice_member(constrained) {
                        output.push_str(&format!(
                            "  /** Required by profile (min: {}) */\n",
                            constrained.min
                        ));
                        output.push_str(&format!(
                            "  {}: NonNullable<{}['{}']>;\n",
                            member, self.base_type, member
                        ));
                    } else if let Some(doc) = unnarrowed_choice_doc(constrained) {
                        output.push_str(&doc);
                    } else {
                        output.push_str(&format!(
                            "  /** Required by profile (min: {}) */\n",
                            constrained.min
                        ));
                        // Remove optional marker by redefining as required
                        output.push_str(&format!(
                            "  {}: NonNullable<{}['{}']>;\n",
                            constrained.field_name, self.base_type, constrained.field_name
                        ));
                    }
                }
            }

            output.push_str("}\n\n");
        }

        // Generate type guard
        output.push_str(&format!(
            "export function is{}(value: {}): value is {} {{\n",
            self.type_name, self.base_type, self.type_name
        ));
        output.push_str(&format!(
            "  return '__profileUrl' in value && value.__profileUrl === '{}';\n",
            self.canonical_url
        ));
        output.push_str("}\n");

        // Generate Zod schema if requested
        // Use z.intersection() instead of .extend() because .extend() doesn't work
        // on z.ZodType<object> which is used for recursive schemas like Extension
        if with_zod {
            output.push('\n');
            output.push_str(&format!("/**\n * Zod schema for {}\n */\n", self.type_name));

            if self.constrained_elements.is_empty() {
                // No additional constraints, just re-export the base schema
                output.push_str(&format!(
                    "export const {}Schema = {}Schema;\n",
                    self.type_name, self.base_type
                ));
            } else {
                // Use intersection to combine base schema with additional constraints
                output.push_str(&format!(
                    "export const {}Schema = z.intersection(\n  {}Schema,\n  z.object({{\n",
                    self.type_name, self.base_type
                ));

                // Add constrained fields to the schema
                for constrained in &self.constrained_elements {
                    if constrained.makes_required {
                        // Choice (`value[x]`) elements have no single member on the
                        // wire — requiring the collapsed name (`value`) would reject
                        // valid instances that carry `valueQuantity`, etc. Narrowing
                        // to "exactly one variant present" is a runtime-refinement
                        // follow-up; skip the phantom requirement here.
                        if !constrained.choice_members.is_empty() {
                            output.push_str(&format!(
                                "    // choice {} (one of {}) — enforced at runtime\n",
                                constrained.field_name,
                                constrained.choice_members.join(", ")
                            ));
                            continue;
                        }
                        // Override to make required
                        output.push_str(&format!(
                            "    {}: z.array(z.unknown()).min({}),\n",
                            constrained.field_name, constrained.min
                        ));
                    }
                }

                output.push_str("  })\n);\n");
            }
        }

        output
    }

    /// Generate extension accessor methods for a profile class.
    fn generate_extension_methods(
        &self,
        output: &mut String,
        extension: &crate::extensions::RenderExtension,
    ) {
        let method_name = extension
            .type_name
            .strip_suffix("Extension")
            .unwrap_or(&extension.type_name);

        // Generate getter method
        output.push_str(&format!(
            "  /**\n   * Get the {} extension\n   * @see {}\n   */\n",
            extension
                .description
                .as_deref()
                .unwrap_or(&extension.type_name),
            extension.url
        ));

        if extension.is_complex && !extension.nested_types.is_empty() {
            // Complex extension with flat API
            let input_type_name = format!("{}Input", method_name);

            // Generate overloaded getters
            output.push_str(&format!(
                "  get{}(raw: true): Extension | undefined;\n",
                method_name
            ));
            output.push_str(&format!(
                "  get{}(raw?: false): {} | undefined;\n",
                method_name, input_type_name
            ));
            output.push_str(&format!(
                "  get{}(raw?: boolean): Extension | {} | undefined {{\n",
                method_name, input_type_name
            ));
            output.push_str(&format!(
                "    const ext = this.extension?.find(e => e.url === '{}');\n",
                extension.url
            ));
            output.push_str("    if (!ext) return undefined;\n");
            output.push_str("    if (raw) return ext;\n");
            output.push_str(&format!("    return extensionTo{}(ext);\n", method_name));
            output.push_str("  }\n\n");
        } else if extension.is_complex {
            // Complex extension without nested types (fallback to raw)
            output.push_str(&format!(
                "  get{}(): {{ url: string; extension?: Extension[] }} | undefined {{\n",
                method_name
            ));
            output.push_str(&format!(
                "    return this.extension?.find(e => e.url === '{}');\n",
                extension.url
            ));
            output.push_str("  }\n\n");
        } else {
            // Simple extension returns the value
            let value_type = extension.value_type.as_deref().unwrap_or("unknown");
            output.push_str(&format!(
                "  get{}(): {} | undefined {{\n",
                method_name, value_type
            ));
            output.push_str(&format!(
                "    const ext = this.extension?.find(e => e.url === '{}');\n",
                extension.url
            ));

            // Use the wire-correct value member (`valueDateTime`, `valueReference`,
            // …) derived from the FHIR type code. When the value is untyped (a
            // multi-type choice), fall back to a raw read.
            match extension_value_member(extension) {
                Some(member) => output.push_str(&format!(
                    "    return ext?.{} as {} | undefined;\n",
                    member, value_type
                )),
                None => output.push_str(&format!(
                    "    return (ext as any)?.value as {} | undefined;\n",
                    value_type
                )),
            }
            output.push_str("  }\n\n");
        }

        // Generate setter method
        output.push_str(&format!(
            "  /**\n   * Set the {} extension\n   * @see {}\n   */\n",
            extension
                .description
                .as_deref()
                .unwrap_or(&extension.type_name),
            extension.url
        ));

        if extension.is_complex && !extension.nested_types.is_empty() {
            // Complex extension with flat API
            let input_type_name = format!("{}Input", method_name);
            output.push_str(&format!(
                "  set{}(value: {}): this {{\n",
                method_name, input_type_name
            ));
        } else if extension.is_complex {
            // Complex extension without nested types (fallback to raw)
            output.push_str(&format!(
                "  set{}(value: {{ url: string; extension?: Extension[] }}): this {{\n",
                method_name
            ));
        } else {
            let value_type = extension.value_type.as_deref().unwrap_or("unknown");
            output.push_str(&format!(
                "  set{}(value: {}): this {{\n",
                method_name, value_type
            ));
        }

        output.push_str("    if (!this.extension) {\n");
        output.push_str("      this.extension = [];\n");
        output.push_str("    }\n");
        output.push_str(&format!(
            "    const idx = this.extension.findIndex(e => e.url === '{}');\n",
            extension.url
        ));

        if extension.is_complex && !extension.nested_types.is_empty() {
            // Convert flat input to Extension format
            output.push_str(&format!(
                "    const ext = {}ToExtension(value);\n",
                method_name
            ));
            output.push_str("    if (idx !== undefined && idx >= 0) {\n");
            output.push_str("      this.extension[idx] = ext;\n");
            output.push_str("    } else {\n");
            output.push_str("      this.extension.push(ext);\n");
            output.push_str("    }\n");
        } else if extension.is_complex {
            // Raw Extension (no conversion)
            output.push_str("    if (idx !== undefined && idx >= 0) {\n");
            output.push_str("      this.extension[idx] = value;\n");
            output.push_str("    } else {\n");
            output.push_str("      this.extension.push(value);\n");
            output.push_str("    }\n");
        } else {
            // Build a minimal Extension with the wire-correct value member. The
            // `as unknown as Extension` cast is deliberate: with branded primitives
            // a plain string literal `url` and an unbranded `value` argument don't
            // structurally satisfy the branded `Extension` shape, but the runtime
            // object is correct FHIR.
            let value_member = extension_value_member(extension);
            let member = value_member.as_deref().unwrap_or("value");

            output.push_str("    const ext = {\n");
            output.push_str(&format!("      url: '{}',\n", extension.url));
            output.push_str(&format!("      {}: value,\n", member));
            output.push_str("    } as unknown as Extension;\n");
            output.push_str("    if (idx !== undefined && idx >= 0) {\n");
            output.push_str("      this.extension[idx] = ext;\n");
            output.push_str("    } else {\n");
            output.push_str("      this.extension.push(ext);\n");
            output.push_str("    }\n");
        }

        output.push_str("    return this;\n");
        output.push_str("  }\n\n");
    }

    /// Generate import statements for the profile.
    fn generate_imports(
        &self,
        output: &mut String,
        as_class: bool,
        with_zod: bool,
        ctx: &ProfileGenerationContext,
    ) {
        // Collect all types that need to be imported
        // Group by (package_folder, file_stem) -> (type_imports, value_imports)
        let mut imports_by_path: HashMap<(String, String), (HashSet<String>, HashSet<String>)> =
            HashMap::new();

        // Collect type imports
        let mut type_imports_needed: Vec<String> = Vec::new();
        let mut value_imports_needed: Vec<String> = Vec::new();

        // Import base type
        if as_class {
            value_imports_needed.push(self.base_type.clone());
        } else {
            type_imports_needed.push(self.base_type.clone());
        }

        // Import base type schema if using Zod
        if with_zod {
            let schema_name = format!("{}Schema", self.base_type);
            value_imports_needed.push(schema_name);
        }

        // Import the Extension type if we have extensions — unless it is already
        // the base type (imported above), which would double-import the identifier.
        if !self.extensions.is_empty() && self.base_type != "Extension" {
            type_imports_needed.push("Extension".to_string());
        }

        // Import the value types of simple extensions (e.g. `Reference`,
        // `CodeableConcept`) used by the accessor methods. Primitive value types
        // (`string`, `number`, `boolean`) need no import.
        for ext in &self.extensions {
            if !ext.is_complex
                && let Some(value_type) = ext.value_type.as_deref()
                && !is_primitive_typescript_type(value_type)
                && value_type != "unknown"
            {
                type_imports_needed.push(value_type.to_string());
            }
        }

        // Check if we need FhirString for extension URL casts
        // FhirString is handled separately since it's a primitive in primitives.ts
        let needs_fhir_string = self
            .extensions
            .iter()
            .any(|ext| ext.is_complex && !ext.nested_types.is_empty());

        // Import types used in extension nested types
        for ext in &self.extensions {
            for nested in &ext.nested_types {
                // Add the base type if it's not a primitive
                if !is_primitive_typescript_type(&nested.base_type) {
                    type_imports_needed.push(nested.base_type.clone());
                }
            }
        }

        // Add type imports to the map
        for type_name in type_imports_needed {
            if let Some((pkg, stem)) = ctx.type_registry.get(&type_name) {
                let key = (pkg.to_string(), stem.to_string());
                imports_by_path
                    .entry(key)
                    .or_insert_with(|| (HashSet::new(), HashSet::new()))
                    .0
                    .insert(type_name);
            }
        }

        // Add value imports to the map
        for type_name in value_imports_needed {
            // For schemas (e.g., "ScheduleSchema"), look up the base type ("Schedule")
            let lookup_name = if type_name.ends_with("Schema") {
                type_name.strip_suffix("Schema").unwrap_or(&type_name)
            } else {
                &type_name
            };

            if let Some((pkg, stem)) = ctx.type_registry.get(lookup_name) {
                let key = (pkg.to_string(), stem.to_string());
                imports_by_path
                    .entry(key)
                    .or_insert_with(|| (HashSet::new(), HashSet::new()))
                    .1
                    .insert(type_name);
            }
        }

        // Build import statements sorted by path
        let mut import_statements: Vec<String> = Vec::new();

        // Add zod import if needed
        if with_zod {
            import_statements.push("import { z } from \"zod\";".to_string());
        }

        // Sort paths for deterministic output
        let mut sorted_paths: Vec<_> = imports_by_path.into_iter().collect();
        sorted_paths.sort_by(|a, b| a.0.cmp(&b.0));

        for ((pkg_folder, file_stem), (type_imports, value_imports)) in sorted_paths {
            let import_path = calculate_import_path(
                ctx.current_package_folder,
                "profiles", // Profiles are always in a profiles subfolder
                &pkg_folder,
                &file_stem,
            );

            // Generate type imports
            if !type_imports.is_empty() {
                let mut types: Vec<_> = type_imports.into_iter().collect();
                types.sort();
                import_statements.push(format!(
                    "import type {{ {} }} from \"{}\";",
                    types.join(", "),
                    import_path
                ));
            }

            // Generate value imports
            if !value_imports.is_empty() {
                let mut values: Vec<_> = value_imports.into_iter().collect();
                values.sort();
                import_statements.push(format!(
                    "import {{ {} }} from \"{}\";",
                    values.join(", "),
                    import_path
                ));
            }
        }

        // Add FhirString import for extension URL casts if needed
        // FhirString is a primitive that lives in primitives.ts in r4-core
        if needs_fhir_string {
            let primitives_path = calculate_import_path(
                ctx.current_package_folder,
                "profiles", // Profiles are always in a profiles subfolder
                "r4-core",
                "primitives",
            );
            import_statements.push(format!(
                "import type {{ FhirString }} from \"{}\";",
                primitives_path
            ));
        }

        // Write import statements
        if !import_statements.is_empty() {
            for stmt in import_statements {
                output.push_str(&stmt);
                output.push('\n');
            }
            output.push('\n');
        }
    }

    /// Generate flat input type interface for a complex extension.
    fn generate_flat_input_type(
        &self,
        output: &mut String,
        extension: &crate::extensions::RenderExtension,
    ) {
        let type_name = extension
            .type_name
            .strip_suffix("Extension")
            .unwrap_or(&extension.type_name);
        let input_type_name = format!("{}Input", type_name);

        output.push_str(&format!(
            "/**\n * Flat input type for {} extension\n",
            extension
                .description
                .as_deref()
                .unwrap_or(&extension.type_name)
        ));
        output.push_str(&format!(" * @see {}\n */\n", extension.url));
        output.push_str(&format!("export interface {} {{\n", input_type_name));

        for nested in &extension.nested_types {
            if let Some(doc) = &nested.doc_comment {
                output.push_str(&format!("  /** {} */\n", doc));
            }

            // Determine if field is required (we'd need cardinality info from nested types)
            // For now, make all fields optional except the first one
            let is_required = false; // TODO: check nested.cardinality_min when available

            let optional_marker = if is_required { "" } else { "?" };
            output.push_str(&format!(
                "  {}{}: {};\n",
                nested.type_name, optional_marker, nested.base_type
            ));
        }

        output.push_str("}\n\n");
    }

    /// Generate converter functions between flat input and Extension format.
    fn generate_converter_functions(
        &self,
        output: &mut String,
        extension: &crate::extensions::RenderExtension,
    ) {
        let type_name = extension
            .type_name
            .strip_suffix("Extension")
            .unwrap_or(&extension.type_name);
        let input_type_name = format!("{}Input", type_name);

        // Generate "to Extension" converter
        output.push_str(&format!(
            "function {}ToExtension(input: {}): Extension {{\n",
            type_name, input_type_name
        ));
        output.push_str("  const subExtensions: Extension[] = [];\n\n");

        for nested in &extension.nested_types {
            output.push_str(&format!(
                "  if (input.{} !== undefined) {{\n",
                nested.type_name
            ));

            // Determine the value field based on type
            let value_field = match nested.base_type.as_str() {
                "string" => "valueString",
                "number" => "valueInteger",
                "boolean" => "valueBoolean",
                "Duration" => "valueDuration",
                "Period" => "valuePeriod",
                "CodeableConcept" => "valueCodeableConcept",
                "Coding" => "valueCoding",
                "Reference" => "valueReference",
                "Quantity" => "valueQuantity",
                _ => "value",
            };

            // Use 'as any' to bypass TypeScript's strict Extension type checking
            // Extension.value is a union but we need to set value[x] properties like valueDuration
            output.push_str("    subExtensions.push({\n");
            output.push_str(&format!(
                "      url: '{}' as FhirString,\n",
                nested.type_name
            ));
            output.push_str(&format!(
                "      {}: input.{}\n",
                value_field, nested.type_name
            ));
            output.push_str("    } as Extension);\n");
            output.push_str("  }\n\n");
        }

        output.push_str("  return {\n");
        output.push_str(&format!("    url: '{}' as FhirString,\n", extension.url));
        output.push_str("    extension: subExtensions\n");
        output.push_str("  };\n");
        output.push_str("}\n\n");

        // Generate "from Extension" converter
        output.push_str(&format!(
            "function extensionTo{}(ext: Extension): {} | undefined {{\n",
            type_name, input_type_name
        ));
        output.push_str("  if (!ext.extension) return undefined;\n\n");
        output.push_str(&format!(
            "  const result: Partial<{}> = {{}};\n\n",
            input_type_name
        ));

        for nested in &extension.nested_types {
            let value_field = match nested.base_type.as_str() {
                "string" => "valueString",
                "number" => "valueInteger",
                "boolean" => "valueBoolean",
                "Duration" => "valueDuration",
                "Period" => "valuePeriod",
                "CodeableConcept" => "valueCodeableConcept",
                "Coding" => "valueCoding",
                "Reference" => "valueReference",
                "Quantity" => "valueQuantity",
                _ => "value",
            };

            output.push_str(&format!(
                "  const {} = ext.extension.find(e => e.url === '{}');\n",
                nested.type_name, nested.type_name
            ));
            // Cast to 'any' to access value[x] properties like valueDuration, valueInteger, etc.
            output.push_str(&format!(
                "  if ({}) result.{} = ({} as any).{} as {};\n\n",
                nested.type_name, nested.type_name, nested.type_name, value_field, nested.base_type
            ));
        }

        output.push_str(&format!("  return result as {};\n", input_type_name));
        output.push_str("}\n\n");
    }

    /// Returns true if this profile has any constraints worth generating.
    pub fn has_constraints(&self) -> bool {
        !self.fixed_elements.is_empty()
            || !self.constrained_elements.is_empty()
            || !self.must_support_elements.is_empty()
    }

    /// Build a render context for template-based generation.
    pub fn to_render_context(
        &self,
        method_config: &ProfileMethodConfig,
        with_zod: bool,
    ) -> ProfileRenderContext {
        // Collect imports needed
        let mut imports = Vec::new();
        let mut import_types = std::collections::HashSet::new();

        // Add Extension import if there are extensions
        if !self.extensions.is_empty() {
            import_types.insert("Extension".to_string());
        }

        // Add types from extensions
        for ext in &self.extensions {
            if let Some(value_type) = &ext.value_type
                && !is_primitive_typescript_type(value_type)
            {
                import_types.insert(value_type.clone());
            }
        }

        // Create imports statement
        if !import_types.is_empty() {
            let mut types: Vec<String> = import_types.into_iter().collect();
            types.sort();
            imports.push(ImportStatement {
                types,
                path: "./types".to_string(),
            });
        }

        // Build fixed elements
        let fixed_elements: Vec<FixedElementRender> = self
            .fixed_elements
            .iter()
            .map(|f| FixedElementRender {
                field_name: f.field_name.clone(),
                fixed_value: f.fixed_value.clone(),
            })
            .collect();

        // Build must-support elements
        let must_support_elements: Vec<MustSupportElementRender> = self
            .constrained_elements
            .iter()
            .filter(|c| c.makes_required)
            .map(|c| MustSupportElementRender {
                path: c.path.clone(),
                field_name: c.field_name.clone(),
                zod_constraint: Some(format!("z.array(z.unknown()).min({})", c.min)),
            })
            .collect();

        // Build extension accessors only if enabled
        let extension_accessors: Vec<ExtensionAccessor> = if method_config.extension_accessors {
            self.extensions
                .iter()
                .map(build_extension_accessor)
                .collect()
        } else {
            Vec::new()
        };

        ProfileRenderContext {
            type_name: self.type_name.clone(),
            base_type: self.base_type.clone(),
            canonical_url: self.canonical_url.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            imports,
            fixed_elements,
            must_support_elements,
            extension_accessors,
            extension_style: method_config.extension_style,
            generate_zod: with_zod,
            with_serialization: method_config.serialization,
            with_validation: method_config.validation,
        }
    }

    /// Generate TypeScript code using Tera template.
    pub fn generate_typescript_with_template(
        &self,
        tera: &tera::Tera,
        method_config: &ProfileMethodConfig,
        with_zod: bool,
    ) -> Result<String, tera::Error> {
        let context = self.to_render_context(method_config, with_zod);
        let mut tera_context = tera::Context::new();
        tera_context.insert("profile", &context);
        tera.render("profile.ts.tera", &tera_context)
    }
}

/// Checks if an element path is a direct child of the base type.
/// E.g., "Patient.name" is a direct child of "Patient", but "Patient.name.given" is not.
fn is_direct_child(path: &str, base_type: &str) -> bool {
    if !path.starts_with(base_type) {
        return false;
    }
    // Check there's exactly one segment after base_type
    // e.g., "Patient.name" -> "name" (one segment), "Patient.name.given" -> "name.given" (two segments)
    let suffix = path
        .strip_prefix(base_type)
        .and_then(|s| s.strip_prefix('.'));
    match suffix {
        Some(s) => !s.contains('.'),
        None => false,
    }
}

/// Extracts profile constraints from element definitions.
/// Only extracts constraints for direct children of the base type to avoid duplicates.
fn extract_constraints(
    elements: &[ElementDefinition],
    base_type: &str,
    must_support: &mut Vec<String>,
    fixed: &mut Vec<FixedElement>,
    constrained: &mut Vec<ConstrainedElement>,
    seen_field_names: &mut std::collections::HashSet<String>,
) {
    for element in elements {
        // Extract constraints from non-root elements that are direct children
        if element.path != base_type && is_direct_child(&element.path, base_type) {
            // Extract mustSupport elements
            if element.must_support {
                must_support.push(element.path.clone());
            }

            let raw_field_name = element.path.split('.').next_back().unwrap_or(&element.path);
            let field_name = sanitize_field_name(raw_field_name);

            // Skip primitive element extensions (e.g., _type, _module)
            // Our generated TypeScript types don't include these underscore-prefixed fields
            if field_name.starts_with('_') {
                continue;
            }

            // Skip if we've already seen this field name (avoid duplicates)
            if seen_field_names.contains(&field_name) {
                continue;
            }

            // Extract fixed values
            if let Some(fixed_value) = &element.fixed
                && let Some(ts_value) = json_to_typescript_literal(fixed_value)
            {
                seen_field_names.insert(field_name.clone());
                fixed.push(FixedElement {
                    path: element.path.clone(),
                    field_name: field_name.clone(),
                    fixed_value: ts_value.clone(),
                    value_type: infer_type_from_value(&ts_value),
                });
            }

            // Extract tightened cardinality (min > 0 makes optional required)
            if element.cardinality.min > 0 && !seen_field_names.contains(&field_name) {
                seen_field_names.insert(field_name.clone());
                let max = match element.cardinality.max {
                    inkgen_core::ir::ElementMax::Unbounded => "*".to_string(),
                    inkgen_core::ir::ElementMax::Finite(n) => n.to_string(),
                };

                // For choice (`value[x]`) elements, the base member name no longer
                // exists on the generated type — it expands to `valueQuantity`,
                // `valueString`, … So record the variant member names instead.
                let choice_members = if raw_field_name.ends_with("[x]") {
                    let base = crate::naming::camel_case(raw_field_name.trim_end_matches("[x]"));
                    element
                        .types
                        .iter()
                        .map(|ty| inkgen_core::ir::choice_variant_name(&base, &ty.code))
                        .collect()
                } else {
                    Vec::new()
                };

                constrained.push(ConstrainedElement {
                    path: element.path.clone(),
                    field_name,
                    min: element.cardinality.min,
                    max,
                    makes_required: element.cardinality.min > 0,
                    choice_members,
                });
            }
        }

        // Recursively process children (but is_direct_child check will filter them)
        extract_constraints(
            &element.children,
            base_type,
            must_support,
            fixed,
            constrained,
            seen_field_names,
        );
    }
}

/// Converts a JSON value to a TypeScript literal.
/// Note: Booleans and numbers are skipped because they conflict with branded primitives
/// (FhirBoolean, FhirInteger, etc.) where literal types can't be assigned.
fn json_to_typescript_literal(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(format!("\"{}\"", escape_string(s))),
        // Skip number and boolean fixed values - they conflict with branded primitives
        // (e.g., literal `false` can't be assigned to FhirBoolean branded type)
        serde_json::Value::Number(_) => None,
        serde_json::Value::Bool(_) => None,
        serde_json::Value::Null => Some("null".to_string()),
        serde_json::Value::Array(arr) if arr.is_empty() => Some("[]".to_string()),
        serde_json::Value::Object(obj) if obj.is_empty() => Some("{}".to_string()),
        _ => None, // Complex values not supported as literals
    }
}

/// Infers TypeScript type from a literal value string.
fn infer_type_from_value(value: &str) -> String {
    if value.starts_with('"') {
        "string".to_string()
    } else if value == "true" || value == "false" {
        "boolean".to_string()
    } else if value == "null" {
        "null".to_string()
    } else if value.parse::<f64>().is_ok() {
        "number".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Escapes special characters in strings for TypeScript string literals.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Extracts profile type name from a canonical URL.
///
/// # Examples
///
/// ```
/// # use inkgen_typescript::profiles::profile_url_to_type_name;
/// assert_eq!(
///     profile_url_to_type_name("http://hl7.org/fhir/StructureDefinition/us-core-patient"),
///     "UsCorePatient"
/// );
/// ```
pub fn profile_url_to_type_name(url: &str) -> String {
    let segment = url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or("Profile");

    // Convert kebab-case or snake_case to PascalCase
    segment
        .split(&['-', '_'][..])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Check if a TypeScript type is a primitive (doesn't need import).
fn is_primitive_typescript_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "string" | "number" | "boolean" | "undefined" | "null" | "unknown" | "any"
    ) || type_name.ends_with(" | undefined")
        || type_name.ends_with("[]")
}

/// Build an extension accessor from a RenderExtension.
fn build_extension_accessor(ext: &crate::extensions::RenderExtension) -> ExtensionAccessor {
    // Generate getter name by stripping "Extension" suffix if present
    let getter_name = ext
        .type_name
        .strip_suffix("Extension")
        .unwrap_or(&ext.type_name)
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_lowercase().next().unwrap()
            } else {
                c
            }
        })
        .collect::<String>();

    // Determine value type and field
    let (value_type, value_field, is_complex, is_array) = if ext.is_complex {
        // Complex extensions return the Extension object
        (
            "Extension | undefined".to_string(),
            "extension".to_string(),
            true,
            false,
        )
    } else if let Some(vtype) = &ext.value_type {
        // Simple extension - determine value field from type
        let field = fhir_type_to_value_field(vtype);
        let is_arr = ext.cardinality_max.is_none() || ext.cardinality_max.unwrap() > 1;

        let final_type = if is_arr {
            format!("{}[]", vtype)
        } else {
            format!("{} | undefined", vtype)
        };

        (final_type, field, false, is_arr)
    } else {
        // Unknown type
        ("unknown".to_string(), "value".to_string(), false, false)
    };

    ExtensionAccessor {
        name: ext.type_name.clone(),
        url: ext.url.clone(),
        getter_name,
        value_type,
        value_field,
        is_complex,
        is_array,
        description: ext.description.clone(),
    }
}

/// The wire-correct value member name for a simple extension's `value[x]`
/// (e.g. `valueDateTime`, `valueReference`), derived from the raw FHIR type code.
/// Returns `None` when the value is untyped or a multi-type choice.
fn extension_value_member(extension: &crate::extensions::RenderExtension) -> Option<String> {
    extension
        .value_type_code
        .as_deref()
        .map(|code| inkgen_core::ir::choice_variant_name("value", code))
}

/// If a required choice (`value[x]`) element is narrowed by the profile to exactly
/// one type, return its single variant member name (e.g. `effectivePeriod`) so it
/// can be made required at the type level. Non-choice or multi-variant choices
/// return `None`.
fn required_choice_member(constrained: &ConstrainedElement) -> Option<&str> {
    match constrained.choice_members.as_slice() {
        [single] => Some(single.as_str()),
        _ => None,
    }
}

/// For a required choice (`value[x]`) element the profile leaves open to multiple
/// types, emit a documentation comment instead of a field: a flat interface can't
/// express "exactly one of N optional members is required", so the variant members
/// stay inherited (optional). Returns `None` for non-choice elements.
fn unnarrowed_choice_doc(constrained: &ConstrainedElement) -> Option<String> {
    if constrained.choice_members.len() > 1 {
        Some(format!(
            "  /** Required by profile (min: {}): one of {} (choice type — \
             enforce at runtime) */\n",
            constrained.min,
            constrained.choice_members.join(", ")
        ))
    } else {
        None
    }
}

/// Map a FHIR type to its value[x] field name.
#[allow(dead_code)]
fn fhir_type_to_value_field(fhir_type: &str) -> String {
    let type_name = fhir_type
        .trim_end_matches("[]")
        .trim_end_matches(" | undefined");

    match type_name {
        "string" => "valueString",
        "number" | "integer" => "valueInteger",
        "boolean" => "valueBoolean",
        "CodeableConcept" => "valueCodeableConcept",
        "Coding" => "valueCoding",
        "Reference" => "valueReference",
        "Quantity" => "valueQuantity",
        "Period" => "valuePeriod",
        "Range" => "valueRange",
        "Ratio" => "valueRatio",
        "dateTime" | "Date" => "valueDateTime",
        "date" => "valueDate",
        "time" => "valueTime",
        "instant" => "valueInstant",
        "uri" | "url" => "valueUri",
        "code" => "valueCode",
        "id" => "valueId",
        "markdown" => "valueMarkdown",
        "base64Binary" => "valueBase64Binary",
        "Identifier" => "valueIdentifier",
        "HumanName" => "valueHumanName",
        "Address" => "valueAddress",
        "ContactPoint" => "valueContactPoint",
        "Attachment" => "valueAttachment",
        _ => "value",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use inkgen_core::ir::{ElementCardinality, ElementMax, ProfileLineage, ResourceKind};
    use serde_json::json;
    use tera::Tera;

    /// Helper function to create a Tera instance with the profile template
    fn create_test_tera() -> Tera {
        let mut tera = Tera::default();
        tera.add_raw_template("profile.ts.tera", include_str!("templates/profile.ts.tera"))
            .expect("Failed to add profile template");
        tera
    }

    #[test]
    fn test_profile_url_to_type_name() {
        assert_eq!(
            profile_url_to_type_name("http://hl7.org/fhir/StructureDefinition/us-core-patient"),
            "UsCorePatient"
        );
        assert_eq!(
            profile_url_to_type_name("http://example.org/fhir/StructureDefinition/my_profile"),
            "MyProfile"
        );
    }

    #[test]
    fn test_json_to_typescript_literal() {
        assert_eq!(
            json_to_typescript_literal(&json!("test")),
            Some("\"test\"".to_string())
        );
        // Numbers and booleans are intentionally skipped: literal numeric/bool
        // fixed values conflict with branded primitive types.
        assert_eq!(json_to_typescript_literal(&json!(42)), None);
        assert_eq!(json_to_typescript_literal(&json!(true)), None);
        assert_eq!(
            json_to_typescript_literal(&json!(null)),
            Some("null".to_string())
        );
    }

    #[test]
    fn test_infer_type_from_value() {
        assert_eq!(infer_type_from_value("\"test\""), "string");
        assert_eq!(infer_type_from_value("42"), "number");
        assert_eq!(infer_type_from_value("true"), "boolean");
        assert_eq!(infer_type_from_value("false"), "boolean");
        assert_eq!(infer_type_from_value("null"), "null");
    }

    #[test]
    fn test_profile_info_from_non_profile() {
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

        let profile =
            ProfileInfo::from_resource_definition(&definition, &indexmap::IndexMap::new());
        assert!(profile.is_none());
    }

    #[test]
    fn test_profile_info_from_constraint_profile() {
        let definition = ResourceDefinition {
            id: "us-core-patient".to_string(),
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient".to_string(),
            name: Some("USCorePatientProfile".to_string()),
            title: Some("US Core Patient Profile".to_string()),
            description: Some("Defines constraints on Patient resource".to_string()),
            version: None,
            status: None,
            kind: ResourceKind::Resource,
            fhir_type: Some("Patient".to_string()),
            date: None,
            lineage: ProfileLineage {
                base_definition: Some(
                    "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
                ),
                base_id: Some("Patient".to_string()),
                derivation: Some(Derivation::Constraint),
                type_name: Some("Patient".to_string()),
            },
            elements: vec![
                create_test_element("Patient", 0, 1, false, None),
                create_test_element("Patient.identifier", 1, usize::MAX, true, None),
                // String fixed value: numeric/bool fixed values are intentionally
                // skipped (they conflict with branded primitive types).
                create_test_element("Patient.gender", 0, 1, false, Some(json!("female"))),
            ],
            flat_elements: vec![],
            extensions: vec![],
            invariants: vec![],
        };

        let profile =
            ProfileInfo::from_resource_definition(&definition, &indexmap::IndexMap::new()).unwrap();
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
        assert_eq!(profile.fixed_elements[0].fixed_value, "\"female\"");
        assert_eq!(profile.constrained_elements.len(), 1);
        assert!(profile.constrained_elements[0].makes_required);
    }

    #[test]
    fn test_generate_typescript() {
        let profile = ProfileInfo {
            type_name: "USCorePatient".to_string(),
            canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
                .to_string(),
            base_type: "Patient".to_string(),
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec!["Patient.identifier".to_string()],
            fixed_elements: vec![FixedElement {
                path: "Patient.active".to_string(),
                field_name: "active".to_string(),
                fixed_value: "true".to_string(),
                value_type: "boolean".to_string(),
            }],
            constrained_elements: vec![ConstrainedElement {
                path: "Patient.name".to_string(),
                field_name: "name".to_string(),
                min: 1,
                max: "*".to_string(),
                makes_required: true,
                choice_members: Vec::new(),
            }],
            extensions: vec![],
        };

        let output = profile.generate_typescript(false, false, false, None);

        assert!(output.contains("export interface USCorePatient extends Patient"));
        assert!(output.contains("readonly __profileUrl"));
        assert!(output.contains("active: true"));
        assert!(output.contains("NonNullable<Patient['name']>"));
        assert!(output.contains("export function isUSCorePatient"));
    }

    #[test]
    fn test_generate_profile_class_with_extensions() {
        // Create a simple extension for testing
        let race_extension = crate::extensions::RenderExtension {
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
            type_name: "USCoreRaceExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("CodeableConcept".to_string()),
            value_type_code: Some("CodeableConcept".to_string()),
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
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
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

        // Test class generation with extension methods
        let output = profile.generate_typescript(true, true, false, None);

        assert!(output.contains("export class USCorePatient extends Patient"));
        assert!(output.contains("readonly __profile ="));
        assert!(output.contains("declare identifier: NonNullable<Patient['identifier']>"));
        assert!(output.contains("getUSCoreRace()"));
        assert!(output.contains("setUSCoreRace(value: CodeableConcept)"));
        assert!(output.contains("valueCodeableConcept"));

        // Test class generation without extension methods
        let output_no_methods = profile.generate_typescript(true, false, false, None);
        assert!(output_no_methods.contains("export class USCorePatient extends Patient"));
        assert!(!output_no_methods.contains("getUSCoreRace()"));

        // Test interface generation (original behavior)
        let output_interface = profile.generate_typescript(false, false, false, None);
        assert!(output_interface.contains("export interface USCorePatient extends Patient"));
        assert!(output_interface.contains("readonly __profileUrl"));
        assert!(!output_interface.contains("getUSCoreRace()"));

        // Test Zod schema generation. Profile schemas use z.intersection() over
        // the base schema (Zod .extend() does not compose with branded primitives).
        let output_with_zod = profile.generate_typescript(false, false, true, None);
        assert!(output_with_zod.contains("export const USCorePatientSchema = z.intersection("));
        assert!(output_with_zod.contains("PatientSchema,"));
    }

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

    #[test]
    fn test_profile_render_context() {
        let profile = ProfileInfo {
            type_name: "USCorePatient".to_string(),
            canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
                .to_string(),
            base_type: "Patient".to_string(),
            title: Some("US Core Patient".to_string()),
            description: Some("Profile for US Core Patient".to_string()),
            must_support_elements: vec![],
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
            extensions: vec![],
        };

        let method_config = ProfileMethodConfig::default();
        let context = profile.to_render_context(&method_config, true);

        assert_eq!(context.type_name, "USCorePatient");
        assert_eq!(context.base_type, "Patient");
        assert_eq!(context.title, Some("US Core Patient".to_string()));
        assert_eq!(context.fixed_elements.len(), 1);
        assert_eq!(context.fixed_elements[0].field_name, "active");
        assert_eq!(context.must_support_elements.len(), 1);
        assert_eq!(context.must_support_elements[0].field_name, "identifier");
        assert_eq!(context.extension_style, ExtensionAccessorStyle::Both);
        assert!(context.generate_zod);
        assert!(context.with_serialization);
        assert!(context.with_validation);
    }

    #[test]
    fn test_fhir_type_to_value_field() {
        assert_eq!(fhir_type_to_value_field("string"), "valueString");
        assert_eq!(fhir_type_to_value_field("integer"), "valueInteger");
        assert_eq!(fhir_type_to_value_field("boolean"), "valueBoolean");
        assert_eq!(fhir_type_to_value_field("Coding"), "valueCoding");
        assert_eq!(
            fhir_type_to_value_field("CodeableConcept"),
            "valueCodeableConcept"
        );
        assert_eq!(fhir_type_to_value_field("Reference"), "valueReference");
        assert_eq!(fhir_type_to_value_field("unknown"), "value");
    }

    #[test]
    fn test_is_primitive_typescript_type() {
        assert!(is_primitive_typescript_type("string"));
        assert!(is_primitive_typescript_type("number"));
        assert!(is_primitive_typescript_type("boolean"));
        assert!(is_primitive_typescript_type("undefined"));
        assert!(is_primitive_typescript_type("string | undefined"));
        assert!(is_primitive_typescript_type("Coding[]"));
        assert!(!is_primitive_typescript_type("Coding"));
        assert!(!is_primitive_typescript_type("CodeableConcept"));
    }

    #[test]
    fn test_profile_generation_with_typed_extension_style() {
        let tera = create_test_tera();
        let race_extension = crate::extensions::RenderExtension {
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
            type_name: "USCoreRaceExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("CodeableConcept".to_string()),
            value_type_code: Some("CodeableConcept".to_string()),
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
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
            constrained_elements: vec![],
            extensions: vec![race_extension],
        };

        let method_config = ProfileMethodConfig {
            extension_accessors: true,
            extension_style: ExtensionAccessorStyle::Typed,
            serialization: true,
            validation: true,
        };

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should have typed accessors
        assert!(output.contains("get uSCoreRace()"));
        assert!(output.contains("set uSCoreRace(value: CodeableConcept"));
        assert!(output.contains("valueCodeableConcept"));

        // Should NOT have raw Extension accessors
        assert!(!output.contains("get uSCoreRaceExtension()"));
        assert!(!output.contains("Extension | undefined"));
    }

    #[test]
    fn test_profile_generation_with_raw_extension_style() {
        let tera = create_test_tera();
        let race_extension = crate::extensions::RenderExtension {
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
            type_name: "USCoreRaceExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("CodeableConcept".to_string()),
            value_type_code: Some("CodeableConcept".to_string()),
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
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
            constrained_elements: vec![],
            extensions: vec![race_extension],
        };

        let method_config = ProfileMethodConfig {
            extension_accessors: true,
            extension_style: ExtensionAccessorStyle::Raw,
            serialization: true,
            validation: true,
        };

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should have raw Extension accessors
        assert!(output.contains("get uSCoreRaceExtension()"));
        assert!(output.contains("set uSCoreRaceExtension(value: Extension | undefined)"));

        // Should NOT have typed accessors
        assert!(!output.contains("get uSCoreRace(): CodeableConcept"));
        assert!(!output.contains("set uSCoreRace(value: CodeableConcept"));
    }

    #[test]
    fn test_profile_generation_with_both_extension_style() {
        let tera = create_test_tera();
        let race_extension = crate::extensions::RenderExtension {
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
            type_name: "USCoreRaceExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("CodeableConcept".to_string()),
            value_type_code: Some("CodeableConcept".to_string()),
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
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
            constrained_elements: vec![],
            extensions: vec![race_extension],
        };

        let method_config = ProfileMethodConfig {
            extension_accessors: true,
            extension_style: ExtensionAccessorStyle::Both,
            serialization: true,
            validation: true,
        };

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should have both typed accessors
        assert!(output.contains("get uSCoreRace()"));
        assert!(output.contains("set uSCoreRace(value: CodeableConcept"));

        // AND raw Extension accessors
        assert!(output.contains("get uSCoreRaceExtension()"));
        assert!(output.contains("set uSCoreRaceExtension(value: Extension | undefined)"));
    }

    #[test]
    fn test_profile_generation_without_serialization() {
        let tera = create_test_tera();
        let profile = ProfileInfo {
            type_name: "USCorePatient".to_string(),
            canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
                .to_string(),
            base_type: "Patient".to_string(),
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
            constrained_elements: vec![],
            extensions: vec![],
        };

        let method_config = ProfileMethodConfig {
            extension_accessors: true,
            extension_style: ExtensionAccessorStyle::Both,
            serialization: false,
            validation: true,
        };

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should NOT have serialization methods
        assert!(!output.contains("toJson("));
        assert!(!output.contains("toObject()"));
    }

    #[test]
    fn test_profile_generation_without_validation() {
        let tera = create_test_tera();
        let profile = ProfileInfo {
            type_name: "USCorePatient".to_string(),
            canonical_url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient"
                .to_string(),
            base_type: "Patient".to_string(),
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
            constrained_elements: vec![],
            extensions: vec![],
        };

        let method_config = ProfileMethodConfig {
            extension_accessors: true,
            extension_style: ExtensionAccessorStyle::Both,
            serialization: true,
            validation: false,
        };

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should NOT have validation methods
        assert!(!output.contains("static fromJson("));
        assert!(!output.contains("static fromObject("));
    }

    #[test]
    fn test_profile_generation_without_extension_accessors() {
        let tera = create_test_tera();
        let race_extension = crate::extensions::RenderExtension {
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
            type_name: "USCoreRaceExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("CodeableConcept".to_string()),
            value_type_code: Some("CodeableConcept".to_string()),
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
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
            constrained_elements: vec![],
            extensions: vec![race_extension],
        };

        let method_config = ProfileMethodConfig {
            extension_accessors: false,
            extension_style: ExtensionAccessorStyle::Both,
            serialization: true,
            validation: true,
        };

        let context = profile.to_render_context(&method_config, false);

        // Should have no extension accessors
        assert_eq!(context.extension_accessors.len(), 0);

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should NOT have any extension accessors
        assert!(!output.contains("get uSCoreRace"));
        assert!(!output.contains("set uSCoreRace"));
        assert!(!output.contains("get uSCoreRaceExtension"));
        assert!(!output.contains("set uSCoreRaceExtension"));
    }

    #[test]
    fn test_profile_generation_all_methods_disabled() {
        let tera = create_test_tera();
        let race_extension = crate::extensions::RenderExtension {
            url: "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race".to_string(),
            type_name: "USCoreRaceExtension".to_string(),
            contexts: vec![],
            is_complex: false,
            value_type: Some("CodeableConcept".to_string()),
            value_type_code: Some("CodeableConcept".to_string()),
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
            title: Some("US Core Patient".to_string()),
            description: Some("US Core Patient Profile".to_string()),
            must_support_elements: vec![],
            fixed_elements: vec![],
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

        let method_config = ProfileMethodConfig {
            extension_accessors: false,
            extension_style: ExtensionAccessorStyle::Both,
            serialization: false,
            validation: false,
        };

        let output = profile
            .generate_typescript_with_template(&tera, &method_config, false)
            .unwrap();

        // Should have basic class structure
        assert!(output.contains("export class USCorePatient extends Patient"));
        assert!(output.contains("readonly __profile ="));
        assert!(output.contains("declare identifier"));

        // Should NOT have extension accessors
        assert!(!output.contains("get uSCoreRace"));

        // Should NOT have serialization methods
        assert!(!output.contains("toJson("));
        assert!(!output.contains("toObject()"));

        // Should NOT have validation methods
        assert!(!output.contains("static fromJson("));
        assert!(!output.contains("static fromObject("));
    }
}
