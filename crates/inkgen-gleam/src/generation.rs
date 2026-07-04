//! Programmatic Gleam emission from a resolved [`PackageIr`].
//!
//! Consumes `&PackageIr` and nothing else — no provider, no resolver, no async.
//! All types are emitted into one module (`fhir.gleam`) so cross-type references
//! resolve without imports; any type that is not itself generated (backbone
//! elements, unknown data types) falls back to `Dynamic`, so the output always
//! type-checks.
//!
//! Gleam-specific adaptations over the Rust reference backend:
//! - FHIR primitive types are not emitted as records — their codes map to native
//!   Gleam types (`Bool`, `Int`, `Float`, `String`), avoiding name clashes with
//!   built-ins like `String`.
//! - Type/constructor names that collide with Gleam built-ins (`List`, `Result`,
//!   …) or field labels that collide with keywords (`type`, `case`, …) are
//!   suffixed with `_`.
//! - Cardinality maps to `List(T)` (0..* or n>1) or `Option(T)` (0..1); Gleam
//!   records are boxed on the BEAM, so recursive references need no indirection.

use std::collections::HashSet;

use inkgen_core::ir::{
    ElementDefinition, ElementMax, ResourceDefinition, ResourceKind, expand_choice_elements,
};
use inkgen_core::{GenerationOutput, PackageIr};

/// Emit one Gleam module containing a `pub type` record per resolved type.
pub fn generate(ir: &PackageIr) -> GenerationOutput {
    // Names of every record we will emit — a field referencing one of these uses
    // the generated type; anything else becomes an opaque `Dynamic`. Primitive
    // types are excluded: their codes map to native Gleam types instead.
    let generated: HashSet<String> = ir
        .types()
        .filter(|d| !is_primitive(d))
        .filter_map(type_name)
        .collect();

    let mut body = String::new();
    body.push_str("//// Generated FHIR types (Gleam backend).\n");
    body.push_str("//// Emitted from a resolved PackageIr — no FHIR resolution here.\n\n");
    body.push_str("import gleam/dynamic.{type Dynamic}\n");
    body.push_str("import gleam/option.{type Option}\n\n");

    // Multiple StructureDefinitions (a base type and a profile, or two profiles)
    // can share a PascalCase name. Emit each name once — first wins — so the
    // module type-checks; references still resolve to the retained type.
    let mut emitted: HashSet<String> = HashSet::new();
    for def in ir.types() {
        if is_primitive(def) {
            continue;
        }
        let Some(name) = type_name(def) else {
            continue;
        };
        if !emitted.insert(name.clone()) {
            continue;
        }
        emit_type(&mut body, def, &name, &generated);
    }

    let mut out = GenerationOutput::new();
    out.add_file("fhir.gleam", body);
    out
}

fn is_primitive(def: &ResourceDefinition) -> bool {
    matches!(def.kind, ResourceKind::PrimitiveType)
}

fn type_name(def: &ResourceDefinition) -> Option<String> {
    let raw = def.name.clone().unwrap_or_else(|| def.id.clone());
    if raw.is_empty() {
        return None;
    }
    Some(escape_type(&pascal_case(&raw)))
}

fn emit_type(
    out: &mut String,
    def: &ResourceDefinition,
    name: &str,
    generated: &HashSet<String>,
) {
    if let Some(doc) = def.description.as_deref().or(def.title.as_deref()) {
        for line in doc.lines().take(3) {
            let clean: String = line.chars().filter(|c| !c.is_control()).collect();
            out.push_str("/// ");
            out.push_str(&clean);
            out.push('\n');
        }
    }
    out.push_str(&format!("pub type {name} {{\n"));

    // Collect the record's fields first so we can pick between the empty-record
    // and populated-record syntax.
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    for element in top_level(def) {
        let raw = element
            .path
            .rsplit('.')
            .next()
            .unwrap_or("")
            .trim_end_matches("[x]");
        if raw.is_empty() {
            continue;
        }
        let field = escape_label(&snake_case(raw));
        if !seen.insert(field.clone()) {
            continue;
        }
        let ty = gleam_type(&element, generated);
        fields.push((field, ty));
    }

    if fields.is_empty() {
        out.push_str(&format!("  {name}\n"));
    } else {
        out.push_str(&format!("  {name}(\n"));
        for (field, ty) in fields {
            out.push_str(&format!("    {field}: {ty},\n"));
        }
        out.push_str("  )\n");
    }
    out.push_str("}\n\n");
}

/// Top-level fields of a type, with choice (`value[x]`) elements expanded by core.
fn top_level(def: &ResourceDefinition) -> Vec<ElementDefinition> {
    let root = def
        .elements
        .iter()
        .find(|e| e.path == def.id)
        .or_else(|| {
            def.name
                .as_ref()
                .and_then(|n| def.elements.iter().find(|e| &e.path == n))
        })
        .or_else(|| def.elements.iter().find(|e| !e.path.contains('.')));

    let refs: Vec<ElementDefinition> = match root {
        Some(root) if !root.children.is_empty() => root.children.clone(),
        _ => def
            .flat_elements
            .iter()
            .filter(|e| e.path.matches('.').count() == 1)
            .cloned()
            .collect(),
    };

    expand_choice_elements(refs)
}

fn gleam_type(element: &ElementDefinition, generated: &HashSet<String>) -> String {
    let is_array = match element.cardinality.max {
        ElementMax::Unbounded => true,
        ElementMax::Finite(n) => n > 1,
    };
    let base = element
        .types
        .first()
        .map(|t| map_code(&t.code, generated))
        .unwrap_or_else(|| "Dynamic".to_string());

    if is_array {
        format!("List({base})")
    } else {
        format!("Option({base})")
    }
}

fn map_code(code: &str, generated: &HashSet<String>) -> String {
    match code {
        "boolean" => "Bool".to_string(),
        "integer" | "unsignedInt" | "positiveInt" | "integer64" => "Int".to_string(),
        "decimal" => "Float".to_string(),
        "string" | "code" | "id" | "uri" | "url" | "canonical" | "oid" | "uuid" | "markdown"
        | "base64Binary" | "date" | "dateTime" | "time" | "instant" | "xhtml" => {
            "String".to_string()
        }
        other => {
            let name = escape_type(&pascal_case(other));
            if generated.contains(&name) {
                name
            } else {
                "Dynamic".to_string()
            }
        }
    }
}

/// Rename a type/constructor name that collides with a Gleam built-in. Gleam
/// type names must be UpperCamelCase with no underscores, so we append a
/// `Type` suffix rather than the `_` used for field labels.
fn escape_type(name: &str) -> String {
    match name {
        "Bool" | "Int" | "Float" | "String" | "List" | "Nil" | "Result" | "BitArray"
        | "UtfCodepoint" | "Dynamic" | "Option" | "Ok" | "Error" | "Some" | "None" | "True"
        | "False" => format!("{name}Type"),
        _ => name.to_string(),
    }
}

/// Suffix a field label that collides with a Gleam keyword.
fn escape_label(label: &str) -> String {
    match label {
        // Reserved keywords and words reserved for future use.
        "as" | "assert" | "auto" | "case" | "const" | "delegate" | "derive" | "echo" | "else"
        | "fn" | "if" | "implement" | "import" | "let" | "macro" | "opaque" | "panic" | "pub"
        | "test" | "todo" | "type" | "use" => format!("{label}_"),
        _ => label.to_string(),
    }
}

fn pascal_case(value: &str) -> String {
    split_tokens(value)
        .into_iter()
        .map(|tok| upper_first(&tok))
        .collect()
}

fn snake_case(value: &str) -> String {
    split_tokens(value)
        .into_iter()
        .map(|t| t.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

fn upper_first(token: &str) -> String {
    let mut chars = token.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn split_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            if prev_lower && ch.is_ascii_uppercase() && !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            current.push(ch);
            prev_lower = ch.is_ascii_lowercase();
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
            prev_lower = false;
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if tokens.is_empty() {
        tokens.push(value.to_string());
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_and_pascal() {
        assert_eq!(snake_case("birthDate"), "birth_date");
        assert_eq!(pascal_case("codeableConcept"), "CodeableConcept");
    }

    #[test]
    fn builtin_type_names_are_escaped() {
        assert_eq!(escape_type("List"), "ListType");
        assert_eq!(escape_type("Patient"), "Patient");
    }

    #[test]
    fn keyword_labels_are_escaped() {
        assert_eq!(escape_label("type"), "type_");
        assert_eq!(escape_label("case"), "case_");
        assert_eq!(escape_label("name"), "name");
    }

    #[test]
    fn primitive_and_unknown_mapping() {
        let g = HashSet::new();
        assert_eq!(map_code("boolean", &g), "Bool");
        assert_eq!(map_code("dateTime", &g), "String");
        assert_eq!(map_code("BackboneElement", &g), "Dynamic");
    }

    #[test]
    fn generated_type_is_referenced() {
        let mut g = HashSet::new();
        g.insert("HumanName".to_string());
        assert_eq!(map_code("HumanName", &g), "HumanName");
    }
}
