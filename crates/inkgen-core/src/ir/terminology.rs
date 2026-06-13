//! FHIR-neutral terminology naming helpers.
//!
//! Canonical-URL → type-name lowering and CodeSystem inference live here so
//! every backend (TypeScript, Rust, future Python) derives the same names from
//! the same URLs. Backends keep only language-specific casing/suffix choices.

/// Convert a canonical URL to a PascalCase name.
///
/// Takes the last path segment, splits on `-`/`_`, and PascalCases each word.
/// `fallback` is used only when the URL has no usable final segment.
///
/// # Examples
///
/// ```
/// # use inkgen_core::ir::url_segment_to_pascal_name;
/// assert_eq!(
///     url_segment_to_pascal_name("http://hl7.org/fhir/ValueSet/administrative-gender", "ValueSet"),
///     "AdministrativeGender"
/// );
/// assert_eq!(
///     url_segment_to_pascal_name("http://hl7.org/fhir/StructureDefinition/us-core-patient", "Profile"),
///     "UsCorePatient"
/// );
/// ```
pub fn url_segment_to_pascal_name(url: &str, fallback: &str) -> String {
    let segment = url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or(fallback);

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

/// Infer the CodeSystem canonical URL from a ValueSet canonical URL.
///
/// Uses the common FHIR pattern `…/ValueSet/name` → `…/name`. Returns `None`
/// when the URL does not contain a `/ValueSet/` segment.
///
/// # Examples
///
/// ```
/// # use inkgen_core::ir::infer_codesystem_url_from_valueset;
/// assert_eq!(
///     infer_codesystem_url_from_valueset("http://hl7.org/fhir/ValueSet/administrative-gender"),
///     Some("http://hl7.org/fhir/administrative-gender".to_string())
/// );
/// assert_eq!(infer_codesystem_url_from_valueset("http://example.org/cs"), None);
/// ```
pub fn infer_codesystem_url_from_valueset(valueset_url: &str) -> Option<String> {
    // Pattern: http://hl7.org/fhir/ValueSet/name → http://hl7.org/fhir/name
    if let Some(pos) = valueset_url.rfind("/ValueSet/") {
        let base = &valueset_url[..pos];
        let name = &valueset_url[pos + 10..]; // Skip "/ValueSet/"
        return Some(format!("{}/{}", base, name));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_name_from_valueset_url() {
        assert_eq!(
            url_segment_to_pascal_name(
                "http://hl7.org/fhir/ValueSet/administrative-gender",
                "ValueSet"
            ),
            "AdministrativeGender"
        );
        assert_eq!(
            url_segment_to_pascal_name(
                "http://hl7.org/fhir/ValueSet/contact-point-use",
                "ValueSet"
            ),
            "ContactPointUse"
        );
    }

    #[test]
    fn pascal_name_handles_snake_case() {
        assert_eq!(
            url_segment_to_pascal_name("http://example.org/fhir/ValueSet/my_value_set", "ValueSet"),
            "MyValueSet"
        );
    }

    #[test]
    fn pascal_name_from_profile_url() {
        assert_eq!(
            url_segment_to_pascal_name(
                "http://hl7.org/fhir/StructureDefinition/us-core-patient",
                "Profile"
            ),
            "UsCorePatient"
        );
        assert_eq!(
            url_segment_to_pascal_name(
                "http://example.org/fhir/StructureDefinition/my_profile",
                "Profile"
            ),
            "MyProfile"
        );
    }

    #[test]
    fn pascal_name_trims_trailing_slash() {
        assert_eq!(
            url_segment_to_pascal_name("http://hl7.org/fhir/ValueSet/my-codes/", "ValueSet"),
            "MyCodes"
        );
    }

    #[test]
    fn infer_codesystem_from_valueset_path() {
        assert_eq!(
            infer_codesystem_url_from_valueset(
                "http://hl7.org/fhir/ValueSet/administrative-gender"
            ),
            Some("http://hl7.org/fhir/administrative-gender".to_string())
        );
    }

    #[test]
    fn infer_codesystem_returns_none_without_valueset_segment() {
        assert_eq!(
            infer_codesystem_url_from_valueset("http://example.org/cs"),
            None
        );
    }
}
