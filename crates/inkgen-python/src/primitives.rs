//! FHIR primitive datatype mapping to Python/Pydantic types.

/// How a FHIR primitive code maps to a Python annotation.
pub(crate) struct PrimitiveMapping {
    /// Python annotation for the value (`str`, `FhirDate`, ...).
    pub annotation: &'static str,
    /// Name of the shared alias in `_runtime.py`, when one is needed.
    pub runtime_alias: Option<&'static str>,
}

/// Map a FHIR primitive type code to its Python annotation, or `None` when the
/// code is not a primitive. `string`/`markdown`/`xhtml`/`integer64` stay plain
/// `str`; validated primitives use shared `Annotated` aliases from
/// `_runtime.py`.
pub(crate) fn primitive(code: &str) -> Option<PrimitiveMapping> {
    let (annotation, alias) = match code {
        "string" | "markdown" | "xhtml" | "integer64" => ("str", None),
        "boolean" => ("bool", None),
        "integer" => ("int", None),
        "decimal" => ("FhirDecimal", Some("FhirDecimal")),
        "positiveInt" => ("FhirPositiveInt", Some("FhirPositiveInt")),
        "unsignedInt" => ("FhirUnsignedInt", Some("FhirUnsignedInt")),
        "date" => ("FhirDate", Some("FhirDate")),
        "dateTime" => ("FhirDateTime", Some("FhirDateTime")),
        "instant" => ("FhirInstant", Some("FhirInstant")),
        "time" => ("FhirTime", Some("FhirTime")),
        "uri" => ("FhirUri", Some("FhirUri")),
        "url" => ("FhirUrl", Some("FhirUrl")),
        "canonical" => ("FhirCanonical", Some("FhirCanonical")),
        "code" => ("FhirCode", Some("FhirCode")),
        "id" => ("FhirId", Some("FhirId")),
        "base64Binary" => ("FhirBase64Binary", Some("FhirBase64Binary")),
        "oid" => ("FhirOid", Some("FhirOid")),
        "uuid" => ("FhirUuid", Some("FhirUuid")),
        _ => return None,
    };
    Some(PrimitiveMapping {
        annotation,
        runtime_alias: alias,
    })
}

/// True when the code is a FHIR primitive that admits a `_x` companion.
pub(crate) fn is_primitive(code: &str) -> bool {
    primitive(code).is_some()
}

/// Static contents of the generated `_runtime.py` module.
pub(crate) const RUNTIME_PY: &str = include_str!("templates/runtime.py");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_core_primitives() {
        assert_eq!(primitive("string").unwrap().annotation, "str");
        assert_eq!(primitive("boolean").unwrap().annotation, "bool");
        assert_eq!(primitive("integer").unwrap().annotation, "int");
        assert_eq!(primitive("decimal").unwrap().annotation, "FhirDecimal");
        assert_eq!(primitive("dateTime").unwrap().annotation, "FhirDateTime");
        assert_eq!(primitive("integer64").unwrap().annotation, "str");
        assert!(primitive("HumanName").is_none());
    }

    #[test]
    fn runtime_module_defines_every_alias() {
        for code in [
            "decimal",
            "positiveInt",
            "unsignedInt",
            "date",
            "dateTime",
            "instant",
            "time",
            "uri",
            "url",
            "canonical",
            "code",
            "id",
            "base64Binary",
            "oid",
            "uuid",
        ] {
            let alias = primitive(code).unwrap().runtime_alias.unwrap();
            assert!(
                RUNTIME_PY.contains(&format!("{alias} = ")),
                "missing alias {alias}"
            );
        }
    }
}
