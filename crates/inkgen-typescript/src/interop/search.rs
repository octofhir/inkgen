//! FHIR search parameter helper generation.
//!
//! This module generates TypeScript helpers for constructing FHIR search queries
//! with type-safe parameter interfaces and URL builders.

use serde::Serialize;
use std::collections::BTreeSet;

/// Configuration for search helper generation
#[derive(Debug, Clone, Serialize)]
pub struct SearchConfig {
    /// Generate search parameter interfaces
    pub interfaces: bool,
    /// Generate URL builder functions
    pub url_builders: bool,
    /// Generate advanced search features (_include types, _has, _filter, enhanced chaining)
    pub advanced_search: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            interfaces: true,
            url_builders: true,
            advanced_search: true,
        }
    }
}

/// FHIR version for modifier generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FhirVersion {
    /// FHIR R4/R4B (12 modifiers)
    R4,
    /// FHIR R5 (15 modifiers)
    R5,
}

/// Search parameter helper generation
#[derive(Debug, Clone, Serialize)]
pub struct SearchHelpers {
    /// Resource types to generate search helpers for
    pub resource_types: Vec<String>,
    /// Search parameters loaded from FHIR package
    #[serde(skip)]
    pub search_parameters: Vec<inkgen_core::SearchParameterInfo>,
    /// FHIR version for modifier generation
    pub fhir_version: FhirVersion,
    /// Whether to generate interfaces
    pub has_interfaces: bool,
    /// Whether to generate URL builders
    pub has_url_builders: bool,
    /// Whether to generate advanced search features
    pub has_advanced_search: bool,
}

impl SearchHelpers {
    /// Creates search helpers for a set of resource types
    pub fn new(
        resource_types: Vec<String>,
        search_parameters: Vec<inkgen_core::SearchParameterInfo>,
        config: &SearchConfig,
    ) -> Self {
        Self::new_with_version(resource_types, search_parameters, config, FhirVersion::R4)
    }

    /// Creates search helpers with explicit FHIR version
    pub fn new_with_version(
        resource_types: Vec<String>,
        search_parameters: Vec<inkgen_core::SearchParameterInfo>,
        config: &SearchConfig,
        fhir_version: FhirVersion,
    ) -> Self {
        Self {
            resource_types,
            search_parameters,
            fhir_version,
            has_interfaces: config.interfaces,
            has_url_builders: config.url_builders,
            has_advanced_search: config.advanced_search,
        }
    }

    /// Convenience constructor for R4B FHIR version
    pub fn for_r4b(
        resource_types: Vec<String>,
        search_parameters: Vec<inkgen_core::SearchParameterInfo>,
        config: &SearchConfig,
    ) -> Self {
        Self::new_with_version(resource_types, search_parameters, config, FhirVersion::R4)
    }

    /// Convenience constructor for R5 FHIR version
    pub fn for_r5(
        resource_types: Vec<String>,
        search_parameters: Vec<inkgen_core::SearchParameterInfo>,
        config: &SearchConfig,
    ) -> Self {
        Self::new_with_version(resource_types, search_parameters, config, FhirVersion::R5)
    }

    /// Get modifier TypeScript type for a FHIR parameter type
    fn get_modifier_type(&self, param_type: &str) -> &'static str {
        match param_type {
            "string" => "StringModifier",
            "token" => "TokenModifier",
            "reference" => "ReferenceModifier",
            "uri" => "UriModifier",
            _ => "never",
        }
    }

    /// Get comparator TypeScript type for a FHIR parameter type
    fn get_comparator_type(&self, param_type: &str) -> &'static str {
        match param_type {
            "date" => "DateComparator",
            "number" => "NumberComparator",
            "quantity" => "QuantityComparator",
            _ => "never",
        }
    }

    /// Clean up verbose parameter descriptions
    ///
    /// Removes "Multiple Resources:" lists and keeps only the first meaningful description
    fn clean_description(description: &str) -> String {
        let trimmed = description.trim();

        // Check if this starts with "Multiple Resources:"
        if trimmed.starts_with("Multiple Resources:") {
            // For multi-resource params, use a concise generic description
            return "Search parameter (applies to multiple resource types)".to_string();
        }

        // For regular descriptions, take only the first line or first 120 characters
        let first_line = description.lines().next().unwrap_or(description).trim();
        if first_line.len() > 120 {
            format!("{}...", &first_line[..117])
        } else {
            first_line.to_string()
        }
    }

    /// Generate typed _include union for a resource
    /// Returns (type_definitions, use_in_interface)
    fn generate_include_type(&self, resource_type: &str) -> (String, String) {
        if !self.has_advanced_search {
            // Legacy behavior: simple string array
            return (String::new(), "string | string[]".to_string());
        }

        // Collect ALL reference parameters for this resource
        let ref_params: Vec<&inkgen_core::SearchParameterInfo> = self.search_parameters
            .iter()
            .filter(|sp| sp.applies_to(resource_type) && sp.param_type == "reference")
            .collect();

        if ref_params.is_empty() {
            return (String::new(), "never".to_string());
        }

        // Generate parameter union type
        let param_union_name = format!("{}ReferenceParams", resource_type);
        let param_codes: Vec<String> = ref_params
            .iter()
            .map(|sp| format!("\"{}\"", sp.code))
            .collect();

        let param_union_def = format!(
            "export type {} = {};",
            param_union_name,
            param_codes.join(" | ")
        );

        // Generate include type using template literal
        let type_name = format!("{}Include", resource_type);
        let type_def = format!(
            "export type {} = `{}:${{{}}}`;",
            type_name,
            resource_type,
            param_union_name
        );

        // Support both R4 and R5 styles
        let use_in_interface = if self.fhir_version == FhirVersion::R5 {
            // R5: Support :iterate modifier
            format!(
                "{} | SearchParamValue<IncludeModifier, never> | Array<{} | SearchParamValue<IncludeModifier, never>>",
                type_name, type_name
            )
        } else {
            // R4: Simple array support
            format!("{} | Array<{}>", type_name, type_name)
        };

        (format!("{}\n{}", param_union_def, type_def), use_in_interface)
    }

    /// Generate typed _revinclude union for a resource
    /// Returns (type_definitions, use_in_interface)
    fn generate_revinclude_type(&self, resource_type: &str) -> (String, String) {
        if !self.has_advanced_search {
            // Legacy behavior: simple string array
            return (String::new(), "string | string[]".to_string());
        }

        // Find all reference parameters where target includes this resource
        let mut revinclude_map: std::collections::BTreeMap<(String, String), ()> =
            std::collections::BTreeMap::new();

        for sp in &self.search_parameters {
            if sp.param_type == "reference" && sp.target.contains(&resource_type.to_string()) {
                for base in &sp.base {
                    revinclude_map.insert((base.clone(), sp.code.clone()), ());
                }
            }
        }

        if revinclude_map.is_empty() {
            return (String::new(), "never".to_string());
        }

        // Generate union of all revinclude paths
        let revinclude_paths: Vec<String> = revinclude_map
            .keys()
            .map(|(base, code)| format!("\"{}:{}\"", base, code))
            .collect();

        let type_name = format!("{}RevInclude", resource_type);
        let type_def = format!(
            "export type {} = {};",
            type_name,
            revinclude_paths.join(" | ")
        );

        let use_in_interface = if self.fhir_version == FhirVersion::R5 {
            format!(
                "{} | SearchParamValue<IncludeModifier, never> | Array<{} | SearchParamValue<IncludeModifier, never>>",
                type_name, type_name
            )
        } else {
            format!("{} | Array<{}>", type_name, type_name)
        };

        (type_def, use_in_interface)
    }

    /// Generate _has reverse chaining parameters for a resource
    /// Returns vector of parameter definitions like '_has:Observation:subject:code'
    fn generate_has_parameters(&self, resource_type: &str) -> Vec<String> {
        if !self.has_advanced_search {
            return Vec::new();
        }

        let mut has_params = Vec::new();

        // Find all resources that can reference this resource
        for sp in &self.search_parameters {
            if sp.param_type == "reference" && sp.target.contains(&resource_type.to_string()) {
                // For each referring resource base
                for base_resource in &sp.base {
                    // Get search parameters for the referring resource (all of them!)
                    let referring_params: Vec<&inkgen_core::SearchParameterInfo> = self
                        .search_parameters
                        .iter()
                        .filter(|p| p.applies_to(base_resource))
                        .collect();

                    // Generate _has parameters for each search parameter on the referring resource
                    for ref_param in referring_params {
                        // Skip _id and other common params to avoid clutter (they're rarely used in _has)
                        if ref_param.code.starts_with('_') {
                            continue;
                        }

                        let has_param_name = format!(
                            "'_has:{}:{}:{}'",
                            base_resource, sp.code, ref_param.code
                        );

                        let modifier_type = self.get_modifier_type(&ref_param.param_type);
                        let comparator_type = self.get_comparator_type(&ref_param.param_type);

                        let type_expr = if modifier_type != "never" || comparator_type != "never" {
                            format!("string | SearchParamValue<{}, {}>", modifier_type, comparator_type)
                        } else {
                            "string | SearchParamValue<never, never>".to_string()
                        };

                        let clean_desc = Self::clean_description(&ref_param.description);
                        has_params.push(format!(
                            "\n  /** Find {}s where {} (via {}.{}) matches */\n  {}?: {};",
                            resource_type,
                            clean_desc,
                            base_resource,
                            sp.code,
                            has_param_name,
                            type_expr
                        ));
                    }
                }
            }
        }

        has_params
    }

    /// Generate common search parameters interface
    ///
    /// Creates a base interface with parameters common to all resources
    fn generate_common_parameters(&self) -> String {
        // Generate modifier types based on FHIR version
        let token_modifiers = match self.fhir_version {
            FhirVersion::R4 => "'not' | 'text' | 'in' | 'not-in' | 'below' | 'above'",
            FhirVersion::R5 => "'not' | 'text' | 'in' | 'not-in' | 'below' | 'above' | 'code-text' | 'text-advanced'",
        };

        let reference_modifiers = match self.fhir_version {
            FhirVersion::R4 => "'identifier'",
            FhirVersion::R5 => "'identifier' | 'of-type'",
        };

        let include_modifier = match self.fhir_version {
            FhirVersion::R4 => "",
            FhirVersion::R5 => "\nexport type IncludeModifier = 'iterate';",
        };

        let include_type = match self.fhir_version {
            FhirVersion::R4 => "string | string[]",
            FhirVersion::R5 => "SearchParamValue<IncludeModifier, never> | Array<SearchParamValue<IncludeModifier, never>>",
        };

        let filter_param = if self.has_advanced_search {
            r#"
  /**
   * Advanced filtering using FHIRPath-like syntax
   *
   * Supports complex boolean queries with the following operators:
   * - Comparisons: eq, ne, gt, lt, ge, le, sa (starts after), eb (ends before), ap (approximately)
   * - String ops: co (contains), sw (starts with), ew (ends with)
   * - Logical: and, or, not
   * - Existence: pr (present)
   *
   * Examples:
   *   - Simple: "birthdate ge 1990-01-01"
   *   - Logical: "gender eq male and active eq true"
   *   - Complex: "(code eq 1234-5 and value gt 100) or (code eq 5678-9)"
   */
  _filter?: string;"#
        } else {
            ""
        };

        format!(
            r#"/**
 * FHIR Search Modifiers by parameter type
 */
export type StringModifier = 'exact' | 'contains';
export type TokenModifier = {token_modifiers};
export type ReferenceModifier = {reference_modifiers};
export type UriModifier = 'below' | 'above';{include_modifier}

/**
 * FHIR Search Comparators for date/number/quantity parameters
 */
export type DateComparator = 'eq' | 'ne' | 'gt' | 'lt' | 'ge' | 'le' | 'sa' | 'eb' | 'ap';
export type NumberComparator = 'eq' | 'ne' | 'gt' | 'lt' | 'ge' | 'le' | 'ap';
export type QuantityComparator = 'eq' | 'ne' | 'gt' | 'lt' | 'ge' | 'le' | 'ap';

/**
 * Generic search parameter value with optional modifier/comparator
 */
export type SearchParamValue<
  Modifier extends string = never,
  Comparator extends string = never
> = {{
  value: string;
  modifier?: Modifier;
  comparator?: Comparator;
}};

/**
 * Common FHIR search parameters available for all resources
 */
export interface CommonSearchParams {{
  /** Resource ID */
  _id?: string | SearchParamValue<never, never>;
  /** Last updated timestamp */
  _lastUpdated?: string | SearchParamValue<never, DateComparator>;
  /** Resource profile */
  _profile?: string | SearchParamValue<UriModifier, never>;
  /** Resource security labels */
  _security?: string | SearchParamValue<TokenModifier, never>;
  /** Resource tags */
  _tag?: string | SearchParamValue<TokenModifier, never>;
  /** Text search across narrative */
  _text?: string | SearchParamValue<StringModifier, never>;
  /** Text search across all content */
  _content?: string | SearchParamValue<StringModifier, never>;
  /** Include referenced resources */
  _include?: {include_type};
  /** Reverse include */
  _revinclude?: {include_type};
  /** Sort order */
  _sort?: string;
  /** Number of results */
  _count?: number;
  /** Result offset */
  _offset?: number;
  /** Summary mode */
  _summary?: 'true' | 'text' | 'data' | 'count' | 'false';
  /** Elements to include */
  _elements?: string | string[];{filter_param}
}}"#,
            token_modifiers = token_modifiers,
            reference_modifiers = reference_modifiers,
            include_modifier = include_modifier,
            include_type = include_type,
            filter_param = filter_param,
        )
    }

    /// Generate search parameter interface for a specific resource type
    ///
    /// Creates:
    /// ```typescript
    /// export interface PatientSearchParams extends CommonSearchParams {
    ///   name?: string;
    ///   birthdate?: string;
    ///   gender?: string;
    ///   identifier?: string;
    ///   // ...
    /// }
    /// ```
    pub fn generate_search_interface(&self, resource_type: &str) -> String {
        use std::collections::HashMap;

        // Filter search parameters for this resource type and deduplicate by code
        let mut params_map: HashMap<String, &inkgen_core::SearchParameterInfo> = HashMap::new();

        for sp in self.search_parameters.iter() {
            // Direct match
            if sp.applies_to(resource_type) {
                params_map.entry(sp.code.clone()).or_insert(sp);
            }
            // Expand DomainResource/Resource parameters to all concrete resources
            else if (sp.base.contains(&"DomainResource".to_string()) || sp.base.contains(&"Resource".to_string()))
                && !["Resource", "DomainResource"].contains(&resource_type)
            {
                params_map.entry(sp.code.clone()).or_insert(sp);
            }
        }

        // Sort parameters by code for deterministic output
        let mut params: Vec<_> = params_map.values().collect();
        params.sort_by_key(|p| &p.code);

        let mut interface = format!(
            r#"/**
 * Search parameters for {} resources
 * @see http://hl7.org/fhir/{}#search
 */
export interface {}SearchParams extends CommonSearchParams {{"#,
            resource_type,
            resource_type.to_lowercase(),
            resource_type
        );

        // List of common parameters that are already in CommonSearchParams
        // These should NOT be duplicated in resource-specific interfaces
        let common_params = [
            "_id", "_lastUpdated", "_profile", "_security", "_tag",
            "_text", "_content", "_include", "_revinclude", "_sort",
            "_count", "_offset", "_summary", "_elements", "_filter",
            "_query", "_source"  // Also common in R4
        ];

        for param in &params {
            // Skip parameters that are already in CommonSearchParams
            if common_params.contains(&param.code.as_str()) {
                continue;
            }

            let modifier_type = self.get_modifier_type(&param.param_type);
            let comparator_type = self.get_comparator_type(&param.param_type);

            // Build type expression - allow both plain string and SearchParamValue
            let type_expr = if modifier_type != "never" || comparator_type != "never" {
                format!("string | SearchParamValue<{}, {}>", modifier_type, comparator_type)
            } else {
                // For types that don't support modifiers/comparators, still allow plain string or object
                match param.param_type.as_str() {
                    "number" => "string | SearchParamValue<never, NumberComparator>".to_string(),
                    _ => "string | SearchParamValue<never, never>".to_string(),
                }
            };

            // Handle parameter names with hyphens (use quoted properties)
            let param_name = if param.code.contains('-') {
                format!("\"{}\"", param.code)
            } else {
                param.code.clone()
            };

            let clean_desc = Self::clean_description(&param.description);
            interface.push_str(&format!(
                "\n  /** {} */\n  {}?: {};",
                clean_desc, param_name, type_expr
            ));
        }

        // Add resource-specific _include/_revinclude overrides (advanced search)
        if self.has_advanced_search {
            let (_, include_use) = self.generate_include_type(resource_type);
            let (_, revinclude_use) = self.generate_revinclude_type(resource_type);

            if include_use != "never" {
                interface.push_str(&format!(
                    "\n\n  /** Include referenced resources (resource-specific) */\n  _include?: {};",
                    include_use
                ));
            }

            if revinclude_use != "never" {
                interface.push_str(&format!(
                    "\n  /** Reverse include (resource-specific) */\n  _revinclude?: {};",
                    revinclude_use
                ));
            }
        }

        // Add _has reverse chaining parameters (advanced search)
        let has_params = self.generate_has_parameters(resource_type);
        if !has_params.is_empty() {
            interface.push_str("\n\n  // Reverse chaining (_has) parameters");
            for has_param in has_params {
                interface.push_str(&has_param);
            }
        }

        // Add chaining support for reference parameters
        let reference_params: Vec<_> = params.iter()
            .filter(|p| p.param_type == "reference" && !p.target.is_empty())
            .collect();

        if !reference_params.is_empty() {
            interface.push_str("\n\n  // Chained search parameters");

            if self.has_advanced_search {
                // Enhanced chaining: Generate ALL search parameters on target resources
                for param in reference_params {
                    for target_type in &param.target {
                        // Get ALL search parameters for this target resource
                        let target_params: Vec<&inkgen_core::SearchParameterInfo> = self
                            .search_parameters
                            .iter()
                            .filter(|sp| sp.applies_to(target_type))
                            .collect();

                        // Generate chained parameter for each target search parameter
                        for target_param in target_params {
                            // Skip common params that start with _ (except _id)
                            if target_param.code.starts_with('_') && target_param.code != "_id" {
                                continue;
                            }

                            let modifier_type = self.get_modifier_type(&target_param.param_type);
                            let comparator_type = self.get_comparator_type(&target_param.param_type);

                            let type_expr = if modifier_type != "never" || comparator_type != "never" {
                                format!("string | SearchParamValue<{}, {}>", modifier_type, comparator_type)
                            } else {
                                "string | SearchParamValue<never, never>".to_string()
                            };

                            // Basic chain: refParam.targetParam
                            let param_desc = Self::clean_description(&param.description);
                            let target_desc = Self::clean_description(&target_param.description);
                            interface.push_str(&format!(
                                "\n  /** {}: {} (chained) */\n  '{}.{}'?: {};",
                                param_desc,
                                target_desc,
                                param.code,
                                target_param.code,
                                type_expr
                            ));

                            // R5: Also generate type-specific chains for multi-target references
                            if self.fhir_version == FhirVersion::R5 && param.target.len() > 1 {
                                interface.push_str(&format!(
                                    "\n  /** {}: {} (chained, {}-specific) */\n  '{}:{}.{}'?: {};",
                                    param_desc,
                                    target_desc,
                                    target_type,
                                    param.code,
                                    target_type,
                                    target_param.code,
                                    type_expr
                                ));
                            }
                        }
                    }
                }
            } else {
                // Legacy behavior: hardcoded common chains
                for param in reference_params {
                    let param_desc = Self::clean_description(&param.description);
                    interface.push_str(&format!(
                        "\n  /** {}'s name (chained) */\n  '{}.name'?: string | SearchParamValue<StringModifier, never>;",
                        param_desc, param.code
                    ));
                    interface.push_str(&format!(
                        "\n  /** {}'s identifier (chained) */\n  '{}.identifier'?: string | SearchParamValue<TokenModifier, never>;",
                        param_desc, param.code
                    ));
                    interface.push_str(&format!(
                        "\n  /** {}'s resource ID (chained) */\n  '{}._id'?: string | SearchParamValue<never, never>;",
                        param_desc, param.code
                    ));
                }
            }
        }

        interface.push_str("\n}");
        interface
    }

    /// Generate URL builder function for a resource type
    ///
    /// Creates:
    /// ```typescript
    /// export function buildPatientSearchUrl(
    ///   baseUrl: string,
    ///   params: PatientSearchParams
    /// ): string {
    ///   const query = new URLSearchParams();
    ///   // Uses appendSearchParam helper to handle SearchParamValue objects
    ///   // ...
    ///   return `${baseUrl}/Patient?${query}`;
    /// }
    /// ```
    pub fn generate_url_builder(&self, resource_type: &str) -> String {
        format!(
            r#"/**
 * Build a FHIR search URL for {} resources
 * @param baseUrl - The FHIR server base URL
 * @param params - Search parameters
 * @returns Complete search URL
 */
export function build{}SearchUrl(
  baseUrl: string,
  params: {}SearchParams
): string {{
  const query = new URLSearchParams();

  // Add all non-undefined parameters to query string
  Object.entries(params).forEach(([key, value]) => {{
    if (value !== undefined && value !== null) {{
      if (Array.isArray(value)) {{
        value.forEach(v => appendSearchParam(query, key, v));
      }} else if (typeof value === 'object' && 'value' in value) {{
        // SearchParamValue object
        appendSearchParam(query, key, value);
      }} else {{
        // Primitive types (_count, _offset, etc.)
        query.set(key, String(value));
      }}
    }}
  }});

  const queryString = query.toString();
  return queryString
    ? `${{baseUrl}}/{}?${{queryString}}`
    : `${{baseUrl}}/{}`;
}}"#,
            resource_type, resource_type, resource_type, resource_type, resource_type
        )
    }

    /// Generate utility functions for search
    pub fn generate_utilities() -> String {
        r#"/**
 * Helper function to append a SearchParamValue to URLSearchParams
 * Handles modifiers (param:modifier=value) and comparators (param=comparatorValue)
 * @param query - URLSearchParams to append to
 * @param key - Parameter name (may include dots for chaining, e.g., 'subject.name')
 * @param value - SearchParamValue object or primitive
 */
export function appendSearchParam(
  query: URLSearchParams,
  key: string,
  value: SearchParamValue<any, any> | string | number | boolean
): void {
  if (typeof value === 'object' && value !== null && 'value' in value) {
    // SearchParamValue object
    const paramKey = value.modifier ? `${key}:${value.modifier}` : key;
    const paramValue = value.comparator ? `${value.comparator}${value.value}` : value.value;
    query.append(paramKey, paramValue);
  } else {
    // Primitive value
    query.append(key, String(value));
  }
}

/**
 * Parse a search parameter value that may include a comparator prefix
 * @param value - Raw parameter value (e.g., "ge2000-01-01" or "lt100")
 * @returns Parsed comparator and value
 */
export function parseComparator(value: string): {
  comparator?: DateComparator | NumberComparator | QuantityComparator;
  value: string;
} {
  const comparatorMatch = value.match(/^(eq|ne|gt|lt|ge|le|sa|eb|ap)(.+)$/);
  if (comparatorMatch) {
    return {
      comparator: comparatorMatch[1] as DateComparator | NumberComparator | QuantityComparator,
      value: comparatorMatch[2]
    };
  }
  return { value };
}

/**
 * Parse search parameters from a URL query string
 * Handles modifiers (param:modifier=value) and comparators (param=comparatorValue)
 * @param queryString - URL query string (without leading '?')
 * @returns Parsed search parameters
 */
export function parseSearchParams(queryString: string): Record<string, string | string[]> {
  const params = new URLSearchParams(queryString);
  const result: Record<string, string | string[]> = {};

  params.forEach((value, key) => {
    // Extract modifier if present (e.g., "name:exact" -> key="name", modifier="exact")
    const [paramKey, modifier] = key.includes(':') ? key.split(':', 2) : [key, undefined];

    // Parse comparator if present
    const parsed = parseComparator(value);

    // Build the final key (use original key with modifier)
    const finalKey = modifier ? `${paramKey}:${modifier}` : paramKey;

    if (result[finalKey]) {
      // Convert to array if multiple values
      if (Array.isArray(result[finalKey])) {
        (result[finalKey] as string[]).push(value);
      } else {
        result[finalKey] = [result[finalKey] as string, value];
      }
    } else {
      result[finalKey] = value;
    }
  });

  return result;
}

/**
 * Build a search query string from parameters
 * Uses appendSearchParam to handle SearchParamValue objects
 * @param params - Search parameters
 * @returns URL-encoded query string
 */
export function buildSearchQuery(params: Record<string, any>): string {
  const query = new URLSearchParams();

  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined && value !== null) {
      if (Array.isArray(value)) {
        value.forEach(v => appendSearchParam(query, key, v));
      } else if (typeof value === 'object' && 'value' in value) {
        // SearchParamValue object
        appendSearchParam(query, key, value);
      } else {
        // Primitive value
        query.set(key, String(value));
      }
    }
  });

  return query.toString();
}"#
        .to_string()
    }

    /// Generate all search helper code split into multiple files
    /// Returns a map of file paths (relative to search directory) to file contents
    pub fn generate_all_split(&self) -> std::collections::HashMap<String, String> {
        use std::collections::{BTreeSet, HashMap};

        let mut files = HashMap::new();

        // Collect all unique resource types from search parameters
        let mut all_resource_types: BTreeSet<String> = BTreeSet::new();
        for sp in &self.search_parameters {
            for base in &sp.base {
                all_resource_types.insert(base.clone());
            }
        }

        // 1. Generate common.ts - Common types and parameters
        files.insert(
            "common.ts".to_string(),
            self.generate_common_file(),
        );

        // 2. Generate types.ts - Include/RevInclude type definitions (if advanced search)
        if self.has_advanced_search && self.has_interfaces {
            let types_content = self.generate_types_file(&all_resource_types);
            if !types_content.is_empty() {
                files.insert("types.ts".to_string(), types_content);
            }
        }

        // 3. Generate interfaces.ts - All SearchParams interfaces
        if self.has_interfaces {
            files.insert(
                "interfaces.ts".to_string(),
                self.generate_interfaces_file(&all_resource_types),
            );
        }

        // 4. Generate builders.ts - All URL builder functions
        if self.has_url_builders {
            files.insert(
                "builders.ts".to_string(),
                self.generate_builders_file(&all_resource_types),
            );
        }

        // 5. Generate index.ts - Re-exports all symbols
        files.insert(
            "index.ts".to_string(),
            self.generate_index_file(),
        );

        files
    }

    /// Generate common.ts - Common types, modifiers, comparators, and CommonSearchParams
    fn generate_common_file(&self) -> String {
        let parts = vec![
            "// FHIR Search Parameters - Common Types and Parameters".to_string(),
            "// Auto-generated by InkGen - do not edit manually".to_string(),
            "".to_string(),
            self.generate_common_parameters(),
            "".to_string(),
            Self::generate_utilities(),
        ];

        parts.join("\n")
    }

    /// Generate types.ts - Include/RevInclude type definitions
    fn generate_types_file(&self, all_resource_types: &BTreeSet<String>) -> String {
        let mut parts = vec![
            "// FHIR Search Parameters - Include/RevInclude Type Definitions".to_string(),
            "// Auto-generated by InkGen - do not edit manually".to_string(),
            "".to_string(),
            "// Type imports".to_string(),
            "import type { SearchParamValue, IncludeModifier } from './common';".to_string(),
            "".to_string(),
        ];

        for resource_type in all_resource_types {
            let (include_defs, _) = self.generate_include_type(resource_type);
            if !include_defs.is_empty() {
                parts.push(include_defs);
            }
            let (revinclude_defs, _) = self.generate_revinclude_type(resource_type);
            if !revinclude_defs.is_empty() {
                parts.push(revinclude_defs);
            }
        }

        if parts.len() <= 6 {
            // Only headers, no actual content
            return String::new();
        }

        parts.join("\n")
    }

    /// Generate interfaces.ts - All SearchParams interfaces
    fn generate_interfaces_file(&self, all_resource_types: &BTreeSet<String>) -> String {
        let mut parts = vec![
            "// FHIR Search Parameters - Resource-Specific Interfaces".to_string(),
            "// Auto-generated by InkGen - do not edit manually".to_string(),
            "".to_string(),
            "// Type imports".to_string(),
            "import type { CommonSearchParams, SearchParamValue } from './common';".to_string(),
        ];

        // Import modifiers/comparators
        parts.push("import type {".to_string());
        parts.push("  StringModifier, TokenModifier, ReferenceModifier, UriModifier,".to_string());
        parts.push("  DateComparator, NumberComparator, QuantityComparator".to_string());
        parts.push("} from './common';".to_string());

        // Import include/revinclude types if advanced search is enabled
        if self.has_advanced_search {
            parts.push("".to_string());
            parts.push("// Import include/revinclude types".to_string());
            let mut type_imports = vec![];
            for resource_type in all_resource_types {
                // Check if this resource has include types
                let (include_defs, _) = self.generate_include_type(resource_type);
                if !include_defs.is_empty() {
                    type_imports.push(format!("  {}Include, {}ReferenceParams", resource_type, resource_type));
                }
                let (revinclude_defs, _) = self.generate_revinclude_type(resource_type);
                if !revinclude_defs.is_empty() {
                    type_imports.push(format!("  {}RevInclude", resource_type));
                }
            }
            if !type_imports.is_empty() {
                parts.push("import type {".to_string());
                parts.push(type_imports.join(",\n"));
                parts.push("} from './types';".to_string());
            }
        }

        parts.push("".to_string());

        // Generate all interfaces
        for resource_type in all_resource_types {
            parts.push(self.generate_search_interface(resource_type));
            parts.push("".to_string());
        }

        parts.join("\n")
    }

    /// Generate builders.ts - All URL builder functions
    fn generate_builders_file(&self, all_resource_types: &BTreeSet<String>) -> String {
        let mut parts = vec![
            "// FHIR Search Parameters - URL Builders".to_string(),
            "// Auto-generated by InkGen - do not edit manually".to_string(),
            "".to_string(),
            "// Type imports".to_string(),
        ];

        // Build import list
        let interface_imports: Vec<String> = all_resource_types
            .iter()
            .map(|rt| format!("  {}SearchParams", rt))
            .collect();

        parts.push("import type {".to_string());
        parts.push(interface_imports.join(",\n"));
        parts.push("} from './interfaces';".to_string());
        parts.push("import { appendSearchParam } from './common';".to_string());
        parts.push("".to_string());

        // Generate all URL builders
        for resource_type in all_resource_types {
            parts.push(self.generate_url_builder(resource_type));
            parts.push("".to_string());
        }

        parts.join("\n")
    }

    /// Generate index.ts - Re-exports all symbols
    fn generate_index_file(&self) -> String {
        let mut parts = vec![
            "// FHIR Search Parameters - Main Index".to_string(),
            "// Auto-generated by InkGen - do not edit manually".to_string(),
            "".to_string(),
            "// Re-export everything from all modules".to_string(),
            "export * from './common';".to_string(),
        ];

        if self.has_advanced_search && self.has_interfaces {
            parts.push("export * from './types';".to_string());
        }

        if self.has_interfaces {
            parts.push("export * from './interfaces';".to_string());
        }

        if self.has_url_builders {
            parts.push("export * from './builders';".to_string());
        }

        parts.join("\n")
    }

    /// Generate all search helper code (legacy single-file method)
    /// Kept for backward compatibility with tests
    pub fn generate_all(&self) -> String {
        use std::collections::BTreeSet;

        let mut parts = vec![
            "// Common search parameters".to_string(),
            self.generate_common_parameters(),
        ];

        // Collect all unique resource types from search parameters
        let mut all_resource_types: BTreeSet<String> = BTreeSet::new();
        for sp in &self.search_parameters {
            for base in &sp.base {
                all_resource_types.insert(base.clone());
            }
        }

        // Generate _include/_revinclude type definitions (advanced search feature)
        if self.has_advanced_search && self.has_interfaces {
            parts.push("\n// Include/Revinclude type definitions".to_string());
            for resource_type in &all_resource_types {
                let (include_defs, _) = self.generate_include_type(resource_type);
                if !include_defs.is_empty() {
                    parts.push(include_defs);
                }
                let (revinclude_defs, _) = self.generate_revinclude_type(resource_type);
                if !revinclude_defs.is_empty() {
                    parts.push(revinclude_defs);
                }
            }
        }

        if self.has_interfaces {
            parts.push("\n// Resource-specific search parameters".to_string());
            for resource_type in &all_resource_types {
                parts.push(self.generate_search_interface(resource_type));
            }
        }

        if self.has_url_builders {
            parts.push("\n// URL builder functions".to_string());
            for resource_type in &all_resource_types {
                parts.push(self.generate_url_builder(resource_type));
            }
        }

        // Always include utilities
        parts.push("\n// Utility functions".to_string());
        parts.push(Self::generate_utilities());

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_config_default() {
        let config = SearchConfig::default();
        assert!(config.interfaces);
        assert!(config.url_builders);
        assert!(config.advanced_search);
    }

    #[test]
    fn test_generate_common_parameters() {
        let helpers = SearchHelpers::new(vec![], vec![], &SearchConfig::default());
        let code = helpers.generate_common_parameters();
        assert!(code.contains("CommonSearchParams"));
        assert!(code.contains("_id"));
        assert!(code.contains("_lastUpdated"));
        assert!(code.contains("_count"));
    }

    #[test]
    fn test_generate_search_interface() {
        use inkgen_core::SearchParameterInfo;
        use serde_json::json;

        let config = SearchConfig::default();

        // Create mock search parameters for Patient
        let name_param = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "name",
            "base": ["Patient"],
            "type": "string",
            "description": "A server defined search that may match any of the string fields in the HumanName"
        })).unwrap();

        let birthdate_param = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "birthdate",
            "base": ["Patient"],
            "type": "date",
            "description": "The patient's date of birth"
        })).unwrap();

        let gender_param = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "gender",
            "base": ["Patient"],
            "type": "token",
            "description": "Gender of the patient"
        })).unwrap();

        let search_params = vec![name_param, birthdate_param, gender_param];
        let helpers = SearchHelpers::new(vec!["Patient".to_string()], search_params, &config);

        let code = helpers.generate_search_interface("Patient");
        assert!(code.contains("PatientSearchParams"));
        assert!(code.contains("extends CommonSearchParams"));
        assert!(code.contains("name?:"));
        assert!(code.contains("birthdate?:"));
        assert!(code.contains("gender?:"));
    }

    #[test]
    fn test_generate_url_builder() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::new(vec!["Patient".to_string()], vec![], &config);

        let code = helpers.generate_url_builder("Patient");
        assert!(code.contains("buildPatientSearchUrl"));
        assert!(code.contains("baseUrl: string"));
        assert!(code.contains("PatientSearchParams"));
        assert!(code.contains("URLSearchParams"));
    }

    #[test]
    fn test_generate_utilities() {
        let code = SearchHelpers::generate_utilities();
        assert!(code.contains("parseSearchParams"));
        assert!(code.contains("buildSearchQuery"));
        assert!(code.contains("URLSearchParams"));
    }

    #[test]
    fn test_generate_all() {
        use inkgen_core::SearchParameterInfo;
        use serde_json::json;

        let config = SearchConfig::default();

        // Create mock search parameters for both Patient and Observation
        let patient_name = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "name",
            "base": ["Patient"],
            "type": "string",
            "description": "Patient name"
        })).unwrap();

        let obs_code = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "code",
            "base": ["Observation"],
            "type": "token",
            "description": "Observation code"
        })).unwrap();

        let search_params = vec![patient_name, obs_code];

        let helpers = SearchHelpers::new(
            vec!["Patient".to_string(), "Observation".to_string()],
            search_params,
            &config,
        );

        let code = helpers.generate_all();

        // Should contain common parameters
        assert!(code.contains("CommonSearchParams"));

        // Should contain interfaces
        assert!(code.contains("PatientSearchParams"));
        assert!(code.contains("ObservationSearchParams"));

        // Should contain URL builders
        assert!(code.contains("buildPatientSearchUrl"));
        assert!(code.contains("buildObservationSearchUrl"));

        // Should contain utilities
        assert!(code.contains("parseSearchParams"));
        assert!(code.contains("buildSearchQuery"));
    }

    #[test]
    fn test_r4b_modifier_generation() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::for_r4b(vec![], vec![], &config);
        let code = helpers.generate_common_parameters();

        // R4B should have 12 modifiers
        assert!(code.contains("type StringModifier = 'exact' | 'contains'"));
        assert!(code.contains("type TokenModifier = 'not' | 'text' | 'in' | 'not-in' | 'below' | 'above'"));
        assert!(code.contains("type ReferenceModifier = 'identifier'"));
        assert!(code.contains("type UriModifier = 'below' | 'above'"));

        // Should NOT have R5-only modifiers
        assert!(!code.contains("'code-text'"));
        assert!(!code.contains("'text-advanced'"));
        assert!(!code.contains("'of-type'"));
        assert!(!code.contains("'iterate'"));
    }

    #[test]
    fn test_r5_modifier_generation() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::for_r5(vec![], vec![], &config);
        let code = helpers.generate_common_parameters();

        // R5 should have 15 modifiers (includes R4B + extras)
        assert!(code.contains("type TokenModifier = 'not' | 'text' | 'in' | 'not-in' | 'below' | 'above' | 'code-text' | 'text-advanced'"));
        assert!(code.contains("type ReferenceModifier = 'identifier' | 'of-type'"));
        assert!(code.contains("type IncludeModifier = 'iterate'"));
    }

    #[test]
    fn test_domain_resource_expansion() {
        use inkgen_core::SearchParameterInfo;
        use serde_json::json;

        let config = SearchConfig::default();

        // Create a DomainResource-level search parameter
        let text_param = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "_text",
            "base": ["DomainResource"],
            "type": "string",
            "description": "Search on the narrative of the resource"
        })).unwrap();

        let search_params = vec![text_param];
        let helpers = SearchHelpers::new(
            vec!["Patient".to_string(), "Observation".to_string()],
            search_params,
            &config,
        );

        // _text is a common parameter, so it should NOT appear in resource-specific interfaces
        // (it's already in CommonSearchParams which Patient/Observation extend)
        let patient_code = helpers.generate_search_interface("Patient");
        assert!(!patient_code.contains("_text?:"), "Common param _text should not be duplicated in Patient interface");

        let observation_code = helpers.generate_search_interface("Observation");
        assert!(!observation_code.contains("_text?:"), "Common param _text should not be duplicated in Observation interface");
    }

    #[test]
    fn test_url_builder_with_modifiers() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::new(vec!["Patient".to_string()], vec![], &config);
        let code = helpers.generate_url_builder("Patient");

        // Should call appendSearchParam helper (but not define it inline)
        assert!(code.contains("appendSearchParam"));

        // Should handle SearchParamValue objects
        assert!(code.contains("'value' in value"));

        // Should NOT contain the inline function definition (moved to utilities)
        assert!(!code.contains("function appendSearchParam"));

        // Verify the utilities section contains the appendSearchParam definition
        let utilities = SearchHelpers::generate_utilities();
        assert!(utilities.contains("export function appendSearchParam"));
        assert!(utilities.contains("value.modifier"));
        assert!(utilities.contains("`${key}:${value.modifier}`"));
    }

    #[test]
    fn test_url_builder_with_comparators() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::new(vec!["Observation".to_string()], vec![], &config);
        let code = helpers.generate_url_builder("Observation");

        // Should call appendSearchParam helper (but not define it inline)
        assert!(code.contains("appendSearchParam"));

        // Should NOT contain the inline function definition (moved to utilities)
        assert!(!code.contains("function appendSearchParam"));

        // Verify the utilities section contains the appendSearchParam definition with comparator logic
        let utilities = SearchHelpers::generate_utilities();
        assert!(utilities.contains("export function appendSearchParam"));
        assert!(utilities.contains("value.comparator"));
        assert!(utilities.contains("`${value.comparator}${value.value}`"));
    }

    #[test]
    fn test_chaining_generation() {
        use inkgen_core::SearchParameterInfo;
        use serde_json::json;

        let config = SearchConfig::default();

        // Create a reference parameter
        let subject_param = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "subject",
            "base": ["Observation"],
            "type": "reference",
            "description": "The subject that the observation is about",
            "target": ["Patient", "Group", "Device", "Location"]
        })).unwrap();

        // Add Patient search parameters for enhanced chaining
        let patient_name = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "name",
            "base": ["Patient"],
            "type": "string",
            "description": "A server defined search that may match any of the string fields in the HumanName"
        })).unwrap();

        let patient_identifier = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "identifier",
            "base": ["Patient"],
            "type": "token",
            "description": "A patient identifier"
        })).unwrap();

        let patient_id = SearchParameterInfo::from_json(&json!({
            "resourceType": "SearchParameter",
            "code": "_id",
            "base": ["Patient"],
            "type": "token",
            "description": "Logical id of this artifact"
        })).unwrap();

        let search_params = vec![subject_param, patient_name, patient_identifier, patient_id];
        let helpers = SearchHelpers::new(vec!["Observation".to_string()], search_params, &config);

        let code = helpers.generate_search_interface("Observation");

        // Should contain chained parameters
        assert!(code.contains("// Chained search parameters"));
        assert!(code.contains("'subject.name'"));
        assert!(code.contains("'subject.identifier'"));
        assert!(code.contains("'subject._id'"));
    }

    #[test]
    fn test_utilities_parsecomparator() {
        let code = SearchHelpers::generate_utilities();

        // Should have parseComparator function
        assert!(code.contains("export function parseComparator"));
        assert!(code.contains("comparator?: DateComparator | NumberComparator | QuantityComparator"));

        // Should parse comparator prefixes
        assert!(code.contains("match(/^(eq|ne|gt|lt|ge|le|sa|eb|ap)(.+)$/"));
    }

    #[test]
    fn test_utilities_updated_for_search_param_value() {
        let code = SearchHelpers::generate_utilities();

        // parseSearchParams should handle modifiers
        assert!(code.contains("key.includes(':')"));
        assert!(code.contains("key.split(':', 2)"));

        // buildSearchQuery should use appendSearchParam
        assert!(code.contains("appendSearchParam(query, key, v)"));
    }

    #[test]
    fn test_search_param_value_type_generation() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::new(vec![], vec![], &config);
        let code = helpers.generate_common_parameters();

        // Should generate SearchParamValue generic type
        assert!(code.contains("export type SearchParamValue<"));
        assert!(code.contains("Modifier extends string = never"));
        assert!(code.contains("Comparator extends string = never"));
        assert!(code.contains("value: string"));
        assert!(code.contains("modifier?: Modifier"));
        assert!(code.contains("comparator?: Comparator"));
    }

    #[test]
    fn test_modifier_type_mapping() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::for_r4b(vec![], vec![], &config);

        // Test modifier mappings
        assert_eq!(helpers.get_modifier_type("string"), "StringModifier");
        assert_eq!(helpers.get_modifier_type("token"), "TokenModifier");
        assert_eq!(helpers.get_modifier_type("reference"), "ReferenceModifier");
        assert_eq!(helpers.get_modifier_type("uri"), "UriModifier");
        assert_eq!(helpers.get_modifier_type("number"), "never");
        assert_eq!(helpers.get_modifier_type("date"), "never");
    }

    #[test]
    fn test_comparator_type_mapping() {
        let config = SearchConfig::default();
        let helpers = SearchHelpers::for_r4b(vec![], vec![], &config);

        // Test comparator mappings
        assert_eq!(helpers.get_comparator_type("date"), "DateComparator");
        assert_eq!(helpers.get_comparator_type("number"), "NumberComparator");
        assert_eq!(helpers.get_comparator_type("quantity"), "QuantityComparator");
        assert_eq!(helpers.get_comparator_type("string"), "never");
        assert_eq!(helpers.get_comparator_type("token"), "never");
    }
}
