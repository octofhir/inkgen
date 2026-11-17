//! Python identifier casing and escaping helpers.

/// Python reserved words that cannot be used as attribute names.
const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// `BaseModel` members that a generated field must not shadow.
const RESERVED: &[&str] = &[
    "schema",
    "json",
    "dict",
    "copy",
    "construct",
    "validate",
    "parse_obj",
    "parse_raw",
    "parse_file",
    "from_orm",
    "update_forward_refs",
];

/// Convert a FHIR camelCase/PascalCase name to Python `snake_case`.
///
/// `birthDate` -> `birth_date`, `valueDateTime` -> `value_date_time`,
/// `USCorePatient` -> `us_core_patient`. Non-alphanumeric characters become
/// underscore separators.
pub(crate) fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower_or_digit =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let prev_upper = i > 0 && chars[i - 1].is_ascii_uppercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            if prev_lower_or_digit || (prev_upper && next_lower) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// Convert a canonical ValueSet name to a PascalCase type identifier.
///
/// Splits on non-alphanumeric characters and upper-cases the first letter of
/// each word, keeping the rest verbatim: `my-vs` -> `MyVs`,
/// `administrative-gender` -> `AdministrativeGender`.
pub(crate) fn pascal_case(name: &str) -> String {
    let mut out = String::new();
    for word in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        out.push_str("UnknownValueSet");
    }
    out
}

/// Upper-case the first letter of a path segment, keeping the rest verbatim
/// (`contact` -> `Contact`), for composite backbone class names.
pub(crate) fn upper_first(segment: &str) -> String {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Make a wire element name safe to use as a Pydantic field attribute.
///
/// Keywords and `BaseModel` members get a trailing underscore; leading
/// underscores and the `model_` namespace are reserved by Pydantic itself.
pub(crate) fn safe_attr(attr: &str) -> String {
    if KEYWORDS.contains(&attr)
        || RESERVED.contains(&attr)
        || attr.starts_with('_')
        || attr.starts_with("model_")
    {
        format!("{attr}_")
    } else {
        attr.to_string()
    }
}

/// Builtin names that generated annotations and validators rely on. A module
/// with such a stem would shadow the builtin in the package namespace where
/// `model_rebuild()` evaluates forward references (e.g. the FHIR `List`
/// resource must not become `list.py`).
const BUILTIN_SHADOWS: &[&str] = &[
    "all",
    "any",
    "bool",
    "bytes",
    "dict",
    "float",
    "frozenset",
    "int",
    "len",
    "list",
    "object",
    "set",
    "slice",
    "str",
    "sum",
    "tuple",
    "type",
];

/// Make a module stem safe as a submodule of the generated package.
pub(crate) fn safe_module(stem: &str) -> String {
    if KEYWORDS.contains(&stem) || BUILTIN_SHADOWS.contains(&stem) {
        format!("{stem}_")
    } else {
        stem.to_string()
    }
}

/// Whether `name` is usable as a top-level Python package identifier: a
/// non-empty ASCII identifier (`[A-Za-z_][A-Za-z0-9_]*`) that is not a keyword.
pub(crate) fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && !KEYWORDS.contains(&name)
        && name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}

/// Sanitize a FHIR type name into a valid Python class identifier.
pub(crate) fn class_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if out.is_empty() {
        out.push_str("UnknownType");
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'T');
    }
    out
}

/// Escape a string for embedding in a Python double-quoted string literal.
pub(crate) fn py_string_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

/// Collapse a docstring to a single safe line for a `"""..."""` literal.
///
/// Every double quote is escaped so a trailing quote cannot merge with the
/// closing delimiter.
pub(crate) fn doc_line(s: &str) -> String {
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_camel_and_acronyms() {
        assert_eq!(snake_case("birthDate"), "birth_date");
        assert_eq!(snake_case("valueDateTime"), "value_date_time");
        assert_eq!(snake_case("resourceType"), "resource_type");
        assert_eq!(snake_case("url"), "url");
        assert_eq!(snake_case("USCorePatient"), "us_core_patient");
        assert_eq!(snake_case("serviceURL"), "service_url");
        assert_eq!(snake_case("Patient"), "patient");
        assert_eq!(snake_case("my-vs"), "my_vs");
    }

    #[test]
    fn pascal_case_words() {
        assert_eq!(pascal_case("AdministrativeGender"), "AdministrativeGender");
        assert_eq!(pascal_case("administrative-gender"), "AdministrativeGender");
        assert_eq!(pascal_case("my-vs"), "MyVs");
        assert_eq!(pascal_case(""), "UnknownValueSet");
    }

    #[test]
    fn safe_attr_escapes_keywords_and_reserved() {
        assert_eq!(safe_attr("class"), "class_");
        assert_eq!(safe_attr("import"), "import_");
        assert_eq!(safe_attr("json"), "json_");
        assert_eq!(safe_attr("model_config"), "model_config_");
        assert_eq!(safe_attr("birth_date"), "birth_date");
    }

    #[test]
    fn safe_module_avoids_builtin_shadowing() {
        assert_eq!(safe_module("list"), "list_");
        assert_eq!(safe_module("str"), "str_");
        assert_eq!(safe_module("class"), "class_");
        assert_eq!(safe_module("patient"), "patient");
    }

    #[test]
    fn package_name_validation() {
        assert!(is_valid_package_name("fhir"));
        assert!(is_valid_package_name("fhir_r4"));
        assert!(is_valid_package_name("_private"));
        assert!(!is_valid_package_name(""));
        assert!(!is_valid_package_name("fhir-r4"));
        assert!(!is_valid_package_name("4fhir"));
        assert!(!is_valid_package_name("class"));
        assert!(!is_valid_package_name("a.b"));
    }

    #[test]
    fn class_name_sanitizes() {
        assert_eq!(class_name("Patient"), "Patient");
        assert_eq!(class_name("123bad"), "T123bad");
        assert_eq!(class_name(""), "UnknownType");
        assert_eq!(class_name("a-b"), "ab");
    }

    #[test]
    fn py_string_escape_quotes_and_newlines() {
        assert_eq!(py_string_escape("a\"b\\c\nd"), "a\\\"b\\\\c d");
    }

    #[test]
    fn doc_line_collapses_and_escapes() {
        assert_eq!(doc_line("a\n  b"), "a b");
        assert_eq!(doc_line("x \"\"\" y"), "x \\\"\\\"\\\" y");
        assert_eq!(doc_line("is \"expanded\""), "is \\\"expanded\\\"");
    }
}
