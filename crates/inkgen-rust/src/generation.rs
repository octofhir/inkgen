//! Programmatic Rust emission from a resolved [`PackageIr`].
//!
//! This is the reference backend: it consumes `&PackageIr` and nothing else — no
//! provider, no resolver, no async — proving the IR is language-neutral. All
//! types are emitted into one module so cross-type references resolve without
//! imports; any type that is not itself generated (backbone elements, unknown
//! data types) falls back to `serde_json::Value`, so the output always compiles.

use std::collections::HashSet;

use inkgen_core::ir::{ElementDefinition, ElementMax, ResourceDefinition, expand_choice_elements};
use inkgen_core::{GenerationOutput, PackageIr};

/// Emit one Rust module containing a `struct` per resolved type.
pub fn generate(ir: &PackageIr) -> GenerationOutput {
    // Names of every struct we will emit — a field referencing one of these uses
    // the generated struct; anything else becomes an opaque `Value`.
    let generated: HashSet<String> = ir.types().filter_map(type_struct_name).collect();

    let mut body = String::new();
    body.push_str("//! Generated FHIR types (reference Rust backend).\n");
    body.push_str("//! Emitted from a resolved PackageIr — no FHIR resolution here.\n\n");
    body.push_str("#![allow(non_snake_case)]\n\n");
    body.push_str("use serde::{Deserialize, Serialize};\n");
    body.push_str("use serde_json::Value;\n\n");

    // Multiple StructureDefinitions (a base type and a profile, or two profiles)
    // can share a PascalCase name. Emit each name once — first wins — so the
    // module compiles; references still resolve to the retained struct.
    let mut emitted: HashSet<String> = HashSet::new();
    for def in ir.types() {
        let Some(name) = type_struct_name(def) else {
            continue;
        };
        if !emitted.insert(name.clone()) {
            continue;
        }
        emit_struct(&mut body, def, &name, &generated);
    }

    let mut out = GenerationOutput::new();
    out.add_file("mod.rs", body);
    out
}

fn type_struct_name(def: &ResourceDefinition) -> Option<String> {
    let raw = def.name.clone().unwrap_or_else(|| def.id.clone());
    if raw.is_empty() {
        return None;
    }
    Some(pascal_case(&raw))
}

fn emit_struct(
    out: &mut String,
    def: &ResourceDefinition,
    name: &str,
    generated: &HashSet<String>,
) {
    if let Some(doc) = def.description.as_deref().or(def.title.as_deref()) {
        for line in doc.lines().take(3) {
            // Strip control chars (e.g. stray `\r` from CRLF) — a bare CR is not
            // allowed in a doc comment.
            let clean: String = line.chars().filter(|c| !c.is_control()).collect();
            out.push_str("/// ");
            out.push_str(&clean);
            out.push('\n');
        }
    }
    out.push_str("#[derive(Debug, Clone, Serialize, Deserialize, Default)]\n");
    out.push_str(&format!("pub struct {name} {{\n"));

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
        let (field, rename) = sanitize_field(raw);
        if !seen.insert(field.clone()) {
            continue;
        }
        let ty = rust_type(&element, generated);
        if let Some(original) = rename {
            out.push_str(&format!("    #[serde(rename = \"{original}\", default)]\n"));
        } else {
            out.push_str("    #[serde(default)]\n");
        }
        out.push_str(&format!("    pub {field}: {ty},\n"));
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

fn rust_type(element: &ElementDefinition, generated: &HashSet<String>) -> String {
    let is_array = match element.cardinality.max {
        ElementMax::Unbounded => true,
        ElementMax::Finite(n) => n > 1,
    };
    let base = element
        .types
        .first()
        .map(|t| map_code(&t.code, generated))
        .unwrap_or_else(|| "Value".to_string());

    if is_array {
        format!("Vec<{base}>")
    } else {
        format!("Option<{base}>")
    }
}

fn map_code(code: &str, generated: &HashSet<String>) -> String {
    match code {
        "boolean" => "bool".to_string(),
        "integer" | "unsignedInt" | "positiveInt" => "i32".to_string(),
        "integer64" => "i64".to_string(),
        "decimal" => "f64".to_string(),
        "string" | "code" | "id" | "uri" | "url" | "canonical" | "oid" | "uuid" | "markdown"
        | "base64Binary" | "date" | "dateTime" | "time" | "instant" | "xhtml" => {
            "String".to_string()
        }
        other => {
            let pascal = pascal_case(other);
            if generated.contains(&pascal) {
                // Box to keep recursive types finitely sized.
                format!("Box<{pascal}>")
            } else {
                "Value".to_string()
            }
        }
    }
}

/// snake_case the field and escape Rust keywords; returns `(field, original)`
/// where `original` is `Some` when serde needs a rename back to the FHIR name.
fn sanitize_field(raw: &str) -> (String, Option<String>) {
    let snake = snake_case(raw);
    let escaped = match snake.as_str() {
        // Strict + reserved keywords that ARE valid as raw identifiers.
        "abstract" | "as" | "async" | "await" | "become" | "box" | "break" | "const"
        | "continue" | "do" | "dyn" | "else" | "enum" | "extern" | "false" | "final" | "fn"
        | "for" | "gen" | "if" | "impl" | "in" | "let" | "loop" | "macro" | "match" | "mod"
        | "move" | "mut" | "override" | "priv" | "pub" | "ref" | "return" | "static" | "struct"
        | "trait" | "true" | "try" | "type" | "typeof" | "union" | "unsafe" | "unsized" | "use"
        | "virtual" | "where" | "while" | "yield" => format!("r#{snake}"),
        // Keywords that cannot be raw identifiers — suffix instead.
        "self" | "super" | "crate" | "Self" => format!("{snake}_"),
        _ => snake.clone(),
    };
    let effective = escaped.trim_start_matches("r#");
    let rename = if effective == raw {
        None
    } else {
        Some(raw.to_string())
    };
    (escaped, rename)
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
    fn keyword_fields_get_raw_no_rename() {
        // A raw identifier (`r#type`) already serializes as `type` under serde,
        // so no rename is needed.
        let (field, rename) = sanitize_field("type");
        assert_eq!(field, "r#type");
        assert_eq!(rename, None);

        // camelCase -> snake_case needs a serde rename back to the FHIR name.
        let (field, rename) = sanitize_field("birthDate");
        assert_eq!(field, "birth_date");
        assert_eq!(rename.as_deref(), Some("birthDate"));

        let (field, rename) = sanitize_field("name");
        assert_eq!(field, "name");
        assert_eq!(rename, None);
    }

    #[test]
    fn primitive_and_unknown_mapping() {
        let g = HashSet::new();
        assert_eq!(map_code("boolean", &g), "bool");
        assert_eq!(map_code("dateTime", &g), "String");
        assert_eq!(map_code("BackboneElement", &g), "Value");
    }
}
