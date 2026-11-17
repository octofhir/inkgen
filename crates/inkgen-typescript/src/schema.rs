//! Structural validation-schema generation for FHIR types.
//!
//! Schemas are built as a dialect-neutral [`SchemaExpr`] tree and rendered to
//! either Zod or Valibot source via [`SchemaExpr::render`], so cardinality and
//! type-mapping logic is shared between validation libraries.

use inkgen_core::ir::{ElementCardinality, ElementDefinition, ElementMax};

use crate::ValidationLibrary;
use crate::naming;

/// FHIR primitive types that map to inline validators (no schema imports).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FhirPrimitive {
    /// FHIR `string`.
    String,
    /// FHIR `markdown`.
    Markdown,
    /// FHIR `code`.
    Code,
    /// FHIR `id`.
    Id,
    /// FHIR `uri`.
    Uri,
    /// FHIR `url`.
    Url,
    /// FHIR `canonical`.
    Canonical,
    /// FHIR `oid`.
    Oid,
    /// FHIR `uuid`.
    Uuid,
    /// FHIR `date`.
    Date,
    /// FHIR `dateTime`.
    DateTime,
    /// FHIR `instant`.
    Instant,
    /// FHIR `time`.
    Time,
    /// FHIR `boolean`.
    Boolean,
    /// FHIR `integer`.
    Integer,
    /// FHIR `unsignedInt`.
    UnsignedInt,
    /// FHIR `positiveInt`.
    PositiveInt,
    /// FHIR `decimal`.
    Decimal,
    /// FHIR `base64Binary`.
    Base64Binary,
    /// FHIR `xhtml`.
    Xhtml,
}

/// A dialect-neutral validation schema expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaExpr {
    /// Inline validator for a FHIR primitive.
    Primitive(FhirPrimitive),
    /// Direct reference to a `{Name}Schema` constant.
    Ref(String),
    /// Deferred reference to a `{Name}Schema` constant (circular types).
    Lazy(String),
    /// Array with optional item-count bounds.
    Array {
        /// Item schema.
        item: Box<SchemaExpr>,
        /// Minimum item count, when constrained.
        min: Option<u32>,
        /// Maximum item count, when constrained.
        max: Option<u32>,
    },
    /// Value may be absent.
    Optional(Box<SchemaExpr>),
    /// Value may be `null` (parallel primitive-extension arrays).
    Nullable(Box<SchemaExpr>),
    /// Union of alternative schemas.
    Union(Vec<SchemaExpr>),
    /// Enumeration over a ValueSet `...Values` array (name includes suffix).
    Enum(String),
    /// String literal (e.g. a concrete `resourceType`).
    Literal(String),
    /// Unconstrained value.
    Unknown,
}

impl SchemaExpr {
    /// Render the expression as source code for the given validation library.
    pub fn render(&self, library: ValidationLibrary) -> String {
        match library {
            ValidationLibrary::Zod => self.render_zod(),
            ValidationLibrary::Valibot => self.render_valibot(),
        }
    }

    fn render_zod(&self) -> String {
        match self {
            Self::Primitive(primitive) => match primitive {
                FhirPrimitive::String
                | FhirPrimitive::Markdown
                | FhirPrimitive::Code
                | FhirPrimitive::Xhtml => "z.string()".to_string(),
                FhirPrimitive::Id => "z.string().regex(/^[A-Za-z0-9\\-\\.]{1,64}$/)".to_string(),
                FhirPrimitive::Uri | FhirPrimitive::Url | FhirPrimitive::Canonical => {
                    "z.url()".to_string()
                }
                FhirPrimitive::Oid => {
                    "z.string().regex(/^urn:oid:[0-2](\\.(0|[1-9][0-9]*))+$/)".to_string()
                }
                FhirPrimitive::Uuid => "z.uuidv4()".to_string(),
                FhirPrimitive::Date => "z.iso.date()".to_string(),
                FhirPrimitive::DateTime | FhirPrimitive::Instant => "z.iso.datetime()".to_string(),
                FhirPrimitive::Time => "z.iso.time()".to_string(),
                FhirPrimitive::Boolean => "z.boolean()".to_string(),
                FhirPrimitive::Integer => "z.number().int()".to_string(),
                FhirPrimitive::UnsignedInt => "z.number().int().nonnegative()".to_string(),
                FhirPrimitive::PositiveInt => "z.number().int().positive()".to_string(),
                FhirPrimitive::Decimal => "z.number()".to_string(),
                FhirPrimitive::Base64Binary => {
                    "z.string().regex(/^[A-Za-z0-9+/]*={0,2}$/)".to_string()
                }
            },
            Self::Ref(name) => format!("{name}Schema"),
            Self::Lazy(name) => format!("z.lazy(() => {name}Schema)"),
            Self::Array { item, min, max } => {
                let mut schema = format!("z.array({})", item.render_zod());
                if let Some(min) = min {
                    schema = format!("{schema}.min({min})");
                }
                if let Some(max) = max {
                    schema = format!("{schema}.max({max})");
                }
                schema
            }
            Self::Optional(inner) => format!("{}.optional()", inner.render_zod()),
            Self::Nullable(inner) => format!("{}.nullable()", inner.render_zod()),
            Self::Union(alternatives) => {
                let parts: Vec<String> = alternatives.iter().map(Self::render_zod).collect();
                format!("z.union([{}])", parts.join(", "))
            }
            Self::Enum(values_name) => format!("z.enum({values_name})"),
            Self::Literal(value) => format!("z.literal(\"{value}\")"),
            Self::Unknown => "z.unknown()".to_string(),
        }
    }

    fn render_valibot(&self) -> String {
        match self {
            Self::Primitive(primitive) => match primitive {
                FhirPrimitive::String
                | FhirPrimitive::Markdown
                | FhirPrimitive::Code
                | FhirPrimitive::Xhtml => "v.string()".to_string(),
                FhirPrimitive::Id => {
                    "v.pipe(v.string(), v.regex(/^[A-Za-z0-9\\-\\.]{1,64}$/))".to_string()
                }
                FhirPrimitive::Uri | FhirPrimitive::Url | FhirPrimitive::Canonical => {
                    "v.pipe(v.string(), v.url())".to_string()
                }
                FhirPrimitive::Oid => {
                    "v.pipe(v.string(), v.regex(/^urn:oid:[0-2](\\.(0|[1-9][0-9]*))+$/))"
                        .to_string()
                }
                FhirPrimitive::Uuid => "v.pipe(v.string(), v.uuid())".to_string(),
                FhirPrimitive::Date => "v.pipe(v.string(), v.isoDate())".to_string(),
                FhirPrimitive::DateTime => "v.pipe(v.string(), v.isoDateTime())".to_string(),
                FhirPrimitive::Instant => "v.pipe(v.string(), v.isoTimestamp())".to_string(),
                FhirPrimitive::Time => "v.pipe(v.string(), v.isoTime())".to_string(),
                FhirPrimitive::Boolean => "v.boolean()".to_string(),
                FhirPrimitive::Integer => "v.pipe(v.number(), v.integer())".to_string(),
                FhirPrimitive::UnsignedInt => {
                    "v.pipe(v.number(), v.integer(), v.minValue(0))".to_string()
                }
                FhirPrimitive::PositiveInt => {
                    "v.pipe(v.number(), v.integer(), v.minValue(1))".to_string()
                }
                FhirPrimitive::Decimal => "v.number()".to_string(),
                FhirPrimitive::Base64Binary => "v.pipe(v.string(), v.base64())".to_string(),
            },
            Self::Ref(name) => format!("{name}Schema"),
            Self::Lazy(name) => format!("v.lazy(() => {name}Schema)"),
            Self::Array { item, min, max } => {
                let array = format!("v.array({})", item.render_valibot());
                let mut actions = Vec::new();
                if let Some(min) = min {
                    actions.push(format!("v.minLength({min})"));
                }
                if let Some(max) = max {
                    actions.push(format!("v.maxLength({max})"));
                }
                if actions.is_empty() {
                    array
                } else {
                    format!("v.pipe({array}, {})", actions.join(", "))
                }
            }
            Self::Optional(inner) => format!("v.optional({})", inner.render_valibot()),
            Self::Nullable(inner) => format!("v.nullable({})", inner.render_valibot()),
            Self::Union(alternatives) => {
                let parts: Vec<String> = alternatives.iter().map(Self::render_valibot).collect();
                format!("v.union([{}])", parts.join(", "))
            }
            Self::Enum(values_name) => format!("v.picklist({values_name})"),
            Self::Literal(value) => format!("v.literal(\"{value}\")"),
            Self::Unknown => "v.unknown()".to_string(),
        }
    }
}

/// A schema expression together with the complex types it references.
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    /// The schema expression tree.
    pub expr: SchemaExpr,
    /// Complex types referenced that need schema imports (e.g., ["Address"]).
    /// Does NOT include primitives since they use inline validators.
    pub type_refs: Vec<String>,
}

impl SchemaInfo {
    /// Render the schema expression for the given validation library.
    pub fn render(&self, library: ValidationLibrary) -> String {
        self.expr.render(library)
    }
}

/// Normalize a FHIR type reference to a simple type name.
///
/// Handles both simple type names and full URLs.
/// Examples:
/// - "string" -> "string"
/// - "http://hl7.org/fhirpath/System.String" -> "string"
/// - "http://hl7.org/fhir/StructureDefinition/Address" -> "Address"
fn normalize_type_name(fhir_type: &str) -> String {
    // If it's a URL, extract the last part
    if fhir_type.starts_with("http://") || fhir_type.starts_with("https://") {
        let last_part = fhir_type.split('/').next_back().unwrap_or(fhir_type);

        // For System types like "System.String", extract after the dot
        if let Some(type_name) = last_part.split('.').next_back() {
            match type_name {
                "String" => return "string".to_string(),
                "Boolean" => return "boolean".to_string(),
                "Integer" => return "integer".to_string(),
                "Decimal" => return "decimal".to_string(),
                "Date" => return "date".to_string(),
                "DateTime" => return "dateTime".to_string(),
                "Time" => return "time".to_string(),
                "Instant" => return "instant".to_string(),
                _ => return type_name.to_string(),
            }
        }
        last_part.to_string()
    } else {
        fhir_type.to_string()
    }
}

/// Map a FHIR type reference to a schema expression with type tracking.
///
/// Primitive types use inline validators (no type refs). Complex types return
/// a deferred reference to their schema with the type added to `type_refs`.
pub fn fhir_to_schema(fhir_type: &str) -> SchemaInfo {
    let normalized = normalize_type_name(fhir_type);
    let primitive = match normalized.as_str() {
        "string" => Some(FhirPrimitive::String),
        "markdown" => Some(FhirPrimitive::Markdown),
        "code" => Some(FhirPrimitive::Code),
        "id" => Some(FhirPrimitive::Id),
        "uri" => Some(FhirPrimitive::Uri),
        "url" => Some(FhirPrimitive::Url),
        "canonical" => Some(FhirPrimitive::Canonical),
        "oid" => Some(FhirPrimitive::Oid),
        "uuid" => Some(FhirPrimitive::Uuid),
        "date" => Some(FhirPrimitive::Date),
        "dateTime" => Some(FhirPrimitive::DateTime),
        "instant" => Some(FhirPrimitive::Instant),
        "time" => Some(FhirPrimitive::Time),
        "boolean" => Some(FhirPrimitive::Boolean),
        "integer" => Some(FhirPrimitive::Integer),
        "unsignedInt" => Some(FhirPrimitive::UnsignedInt),
        "positiveInt" => Some(FhirPrimitive::PositiveInt),
        "decimal" => Some(FhirPrimitive::Decimal),
        "base64Binary" => Some(FhirPrimitive::Base64Binary),
        "xhtml" => Some(FhirPrimitive::Xhtml),
        _ => None,
    };

    match primitive {
        Some(primitive) => SchemaInfo {
            expr: SchemaExpr::Primitive(primitive),
            type_refs: Vec::new(),
        },
        // Complex types are deferred to tolerate circular dependencies.
        None => SchemaInfo {
            expr: SchemaExpr::Lazy(normalized.clone()),
            type_refs: vec![normalized],
        },
    }
}

/// Apply cardinality constraints to a schema expression.
///
/// Handles array wrapping, min/max items, and optional/required.
/// Preserves type references from the input.
pub fn apply_cardinality(base_info: SchemaInfo, cardinality: &ElementCardinality) -> SchemaInfo {
    let is_array = match &cardinality.max {
        ElementMax::Unbounded => true,
        ElementMax::Finite(n) => *n > 1,
    };

    let mut expr = base_info.expr;
    if is_array {
        let min = (cardinality.min > 0).then_some(cardinality.min);
        let max = match &cardinality.max {
            ElementMax::Finite(max) if *max > 1 => Some(*max),
            _ => None,
        };
        expr = SchemaExpr::Array {
            item: Box::new(expr),
            min,
            max,
        };
    }

    if cardinality.min == 0 {
        expr = SchemaExpr::Optional(Box::new(expr));
    }

    SchemaInfo {
        expr,
        type_refs: base_info.type_refs,
    }
}

/// Build the schema expression for an element, with type tracking.
pub fn element_to_schema_info(element: &ElementDefinition) -> Option<SchemaInfo> {
    // Handle elements that reference other definitions via contentReference
    if element.types.is_empty() {
        if let Some(reference) = &element.content_reference {
            let reference_name = reference.trim_start_matches('#');
            let type_name = naming::pascal_case(reference_name);
            let base_info = SchemaInfo {
                expr: SchemaExpr::Lazy(type_name.clone()),
                type_refs: vec![type_name],
            };
            return Some(apply_cardinality(base_info, &element.cardinality));
        }
        return None;
    }

    let base_info = if element.types.len() == 1 {
        fhir_to_schema(&element.types[0].code)
    } else {
        // Choice type - union of possibilities
        let mut all_type_refs = Vec::new();
        let alternatives: Vec<SchemaExpr> = element
            .types
            .iter()
            .map(|t| {
                let info = fhir_to_schema(&t.code);
                all_type_refs.extend(info.type_refs);
                info.expr
            })
            .collect();

        SchemaInfo {
            expr: SchemaExpr::Union(alternatives),
            type_refs: all_type_refs,
        }
    };

    Some(apply_cardinality(base_info, &element.cardinality))
}

/// Generate the schema field name for an element (last path segment).
pub fn element_to_field_name(element: &ElementDefinition) -> String {
    element
        .path
        .split('.')
        .next_back()
        .unwrap_or(&element.path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkgen_core::ir::{ElementCardinality, ElementMax};

    fn render_zod(fhir_type: &str) -> String {
        fhir_to_schema(fhir_type).render(ValidationLibrary::Zod)
    }

    fn render_valibot(fhir_type: &str) -> String {
        fhir_to_schema(fhir_type).render(ValidationLibrary::Valibot)
    }

    #[test]
    fn test_normalize_type_name() {
        assert_eq!(normalize_type_name("string"), "string");
        assert_eq!(normalize_type_name("Address"), "Address");
        assert_eq!(
            normalize_type_name("http://hl7.org/fhirpath/System.String"),
            "string"
        );
        assert_eq!(
            normalize_type_name("http://hl7.org/fhirpath/System.Boolean"),
            "boolean"
        );
        assert_eq!(
            normalize_type_name("http://hl7.org/fhir/StructureDefinition/Address"),
            "Address"
        );
    }

    #[test]
    fn test_fhir_to_schema_zod_primitives() {
        assert_eq!(render_zod("string"), "z.string()");
        assert_eq!(render_zod("boolean"), "z.boolean()");
        assert_eq!(render_zod("integer"), "z.number().int()");
        assert_eq!(
            render_zod("http://hl7.org/fhirpath/System.String"),
            "z.string()"
        );
        assert_eq!(
            render_zod("http://hl7.org/fhirpath/System.Boolean"),
            "z.boolean()"
        );
        assert_eq!(render_zod("decimal"), "z.number()");
        assert!(render_zod("id").contains("regex"));
        assert_eq!(render_zod("date"), "z.iso.date()");
        assert_eq!(render_zod("dateTime"), "z.iso.datetime()");
        assert_eq!(render_zod("instant"), "z.iso.datetime()");
        assert_eq!(render_zod("unsignedInt"), "z.number().int().nonnegative()");
        assert_eq!(render_zod("positiveInt"), "z.number().int().positive()");
    }

    #[test]
    fn test_fhir_to_schema_valibot_primitives() {
        assert_eq!(render_valibot("string"), "v.string()");
        assert_eq!(render_valibot("boolean"), "v.boolean()");
        assert_eq!(render_valibot("integer"), "v.pipe(v.number(), v.integer())");
        assert_eq!(render_valibot("decimal"), "v.number()");
        assert_eq!(render_valibot("date"), "v.pipe(v.string(), v.isoDate())");
        assert_eq!(
            render_valibot("dateTime"),
            "v.pipe(v.string(), v.isoDateTime())"
        );
        assert_eq!(
            render_valibot("instant"),
            "v.pipe(v.string(), v.isoTimestamp())"
        );
        assert_eq!(render_valibot("time"), "v.pipe(v.string(), v.isoTime())");
        assert_eq!(render_valibot("uri"), "v.pipe(v.string(), v.url())");
        assert_eq!(render_valibot("uuid"), "v.pipe(v.string(), v.uuid())");
        assert_eq!(
            render_valibot("unsignedInt"),
            "v.pipe(v.number(), v.integer(), v.minValue(0))"
        );
        assert_eq!(
            render_valibot("positiveInt"),
            "v.pipe(v.number(), v.integer(), v.minValue(1))"
        );
        assert_eq!(
            render_valibot("base64Binary"),
            "v.pipe(v.string(), v.base64())"
        );
        assert!(render_valibot("id").contains("v.regex("));
        assert!(render_valibot("oid").contains("v.regex("));
    }

    #[test]
    fn test_fhir_to_schema_complex_types() {
        let identifier_info = fhir_to_schema("Identifier");
        assert_eq!(
            identifier_info.render(ValidationLibrary::Zod),
            "z.lazy(() => IdentifierSchema)"
        );
        assert_eq!(
            identifier_info.render(ValidationLibrary::Valibot),
            "v.lazy(() => IdentifierSchema)"
        );
        assert_eq!(identifier_info.type_refs, vec!["Identifier"]);

        let human_name_info = fhir_to_schema("HumanName");
        assert_eq!(
            human_name_info.render(ValidationLibrary::Zod),
            "z.lazy(() => HumanNameSchema)"
        );
        assert_eq!(human_name_info.type_refs, vec!["HumanName"]);
    }

    fn cardinality_test(min: u32, max: ElementMax, zod_expected: &str, valibot_expected: &str) {
        let cardinality = ElementCardinality { min, max };
        let base_info = fhir_to_schema("string");
        let info = apply_cardinality(base_info, &cardinality);
        assert_eq!(info.render(ValidationLibrary::Zod), zod_expected);
        assert_eq!(info.render(ValidationLibrary::Valibot), valibot_expected);
    }

    #[test]
    fn test_apply_cardinality_optional() {
        cardinality_test(
            0,
            ElementMax::Finite(1),
            "z.string().optional()",
            "v.optional(v.string())",
        );
    }

    #[test]
    fn test_apply_cardinality_required() {
        cardinality_test(1, ElementMax::Finite(1), "z.string()", "v.string()");
    }

    #[test]
    fn test_apply_cardinality_array_optional() {
        cardinality_test(
            0,
            ElementMax::Unbounded,
            "z.array(z.string()).optional()",
            "v.optional(v.array(v.string()))",
        );
    }

    #[test]
    fn test_apply_cardinality_array_required() {
        cardinality_test(
            1,
            ElementMax::Unbounded,
            "z.array(z.string()).min(1)",
            "v.pipe(v.array(v.string()), v.minLength(1))",
        );
    }

    #[test]
    fn test_apply_cardinality_array_with_max() {
        cardinality_test(
            1,
            ElementMax::Finite(5),
            "z.array(z.string()).min(1).max(5)",
            "v.pipe(v.array(v.string()), v.minLength(1), v.maxLength(5))",
        );
    }

    #[test]
    fn test_apply_cardinality_array_max_only() {
        cardinality_test(
            0,
            ElementMax::Finite(3),
            "z.array(z.string()).max(3).optional()",
            "v.optional(v.pipe(v.array(v.string()), v.maxLength(3)))",
        );
    }

    #[test]
    fn test_enum_and_literal_and_unknown() {
        let enum_expr = SchemaExpr::Enum("AdministrativeGenderValues".to_string());
        assert_eq!(
            enum_expr.render(ValidationLibrary::Zod),
            "z.enum(AdministrativeGenderValues)"
        );
        assert_eq!(
            enum_expr.render(ValidationLibrary::Valibot),
            "v.picklist(AdministrativeGenderValues)"
        );

        let literal = SchemaExpr::Literal("Patient".to_string());
        assert_eq!(
            literal.render(ValidationLibrary::Zod),
            "z.literal(\"Patient\")"
        );
        assert_eq!(
            literal.render(ValidationLibrary::Valibot),
            "v.literal(\"Patient\")"
        );

        assert_eq!(
            SchemaExpr::Unknown.render(ValidationLibrary::Zod),
            "z.unknown()"
        );
        assert_eq!(
            SchemaExpr::Unknown.render(ValidationLibrary::Valibot),
            "v.unknown()"
        );
    }

    #[test]
    fn test_nullable_companion_shape() {
        let expr = SchemaExpr::Optional(Box::new(SchemaExpr::Array {
            item: Box::new(SchemaExpr::Nullable(Box::new(SchemaExpr::Lazy(
                "Element".to_string(),
            )))),
            min: None,
            max: None,
        }));
        assert_eq!(
            expr.render(ValidationLibrary::Zod),
            "z.array(z.lazy(() => ElementSchema).nullable()).optional()"
        );
        assert_eq!(
            expr.render(ValidationLibrary::Valibot),
            "v.optional(v.array(v.nullable(v.lazy(() => ElementSchema))))"
        );
    }
}
