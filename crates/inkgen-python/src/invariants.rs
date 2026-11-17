//! Lowering of FHIRPath XOR-exists invariants to Python runtime checks.
//!
//! Mirrors the TypeScript backend's semantics: only `severity=error`
//! invariants of the shape `a.exists() != b.exists()` over known fields are
//! lowered; everything else is skipped (full FHIRPath is out of scope).

use indexmap::IndexMap;
use inkgen_core::ir::InvariantDefinition;

use crate::naming::py_string_escape;

/// A runtime check rendered inside a `model_validator(mode="after")` body.
#[derive(Debug, Clone)]
pub(crate) struct InvariantCheck {
    /// Python boolean expression that is true when the constraint is VIOLATED.
    pub condition: String,
    /// Violation message as a quoted Python string literal (`"key: human"`).
    pub message: String,
}

/// Field lookup tables for one generated class, keyed by FHIR wire name.
#[derive(Debug, Default)]
pub(crate) struct FieldIndex {
    /// Choice base name (`value`) -> emitted attribute names of its variants.
    pub choices: IndexMap<String, Vec<String>>,
    /// Scalar field wire name -> attribute name.
    pub scalars: IndexMap<String, String>,
    /// List field wire name -> attribute name.
    pub arrays: IndexMap<String, String>,
}

impl FieldIndex {
    /// Render `name.exists()` as a Python presence expression, or `None` when
    /// the name is not a plain identifier over a known field.
    fn exists_expr(&self, name: &str) -> Option<String> {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        if let Some(members) = self.choices.get(name) {
            let parts: Vec<String> = members
                .iter()
                .map(|attr| format!("self.{attr} is not None"))
                .collect();
            return Some(format!("any(({}))", parts.join(", ")));
        }
        if let Some(attr) = self.scalars.get(name) {
            return Some(format!("self.{attr} is not None"));
        }
        if let Some(attr) = self.arrays.get(name) {
            return Some(format!("bool(self.{attr})"));
        }
        None
    }
}

/// Lower error-severity `a.exists() != b.exists()` invariants (exclusive
/// presence, e.g. Extension's `ext-1`). XOR is violated when both sides exist
/// or neither does.
pub(crate) fn lower_xor_exists_checks(
    invariants: &[InvariantDefinition],
    fields: &FieldIndex,
) -> Vec<InvariantCheck> {
    let mut checks = Vec::new();
    for inv in invariants {
        if inv.severity.as_deref() != Some("error") {
            continue;
        }
        let Some(expr) = &inv.expression else {
            continue;
        };
        let Some((lhs, rhs)) = expr.split_once("!=") else {
            continue;
        };
        let Some(lhs) = exists_side(lhs.trim(), fields) else {
            continue;
        };
        let Some(rhs) = exists_side(rhs.trim(), fields) else {
            continue;
        };
        let human = inv.human.as_deref().unwrap_or(expr);
        checks.push(InvariantCheck {
            condition: format!("({lhs}) == ({rhs})"),
            message: format!(
                "\"{}\"",
                py_string_escape(&format!("{}: {}", inv.key, human))
            ),
        });
    }
    checks
}

/// Render one side of an `!=` invariant, requiring a plain `<name>.exists()`.
fn exists_side(side: &str, fields: &FieldIndex) -> Option<String> {
    let name = side.strip_suffix(".exists()")?;
    fields.exists_expr(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invariant(key: &str, severity: &str, expression: &str) -> InvariantDefinition {
        InvariantDefinition {
            key: key.to_string(),
            severity: Some(severity.to_string()),
            human: Some("Must have either extensions or value[x], not both".to_string()),
            expression: Some(expression.to_string()),
            xpath: None,
            fhirpath: None,
            additional: IndexMap::new(),
        }
    }

    fn ext1_index() -> FieldIndex {
        let mut choices = IndexMap::new();
        choices.insert(
            "value".to_string(),
            vec!["value_boolean".to_string(), "value_string".to_string()],
        );
        let mut arrays = IndexMap::new();
        arrays.insert("extension".to_string(), "extension".to_string());
        FieldIndex {
            choices,
            scalars: IndexMap::new(),
            arrays,
        }
    }

    #[test]
    fn lowers_ext1() {
        let checks = lower_xor_exists_checks(
            &[invariant(
                "ext-1",
                "error",
                "extension.exists() != value.exists()",
            )],
            &ext1_index(),
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(
            checks[0].condition,
            "(bool(self.extension)) == (any((self.value_boolean is not None, self.value_string is not None)))"
        );
        assert!(checks[0].message.starts_with("\"ext-1: "));
    }

    #[test]
    fn skips_unsupported_shapes() {
        let checks = lower_xor_exists_checks(
            &[
                invariant(
                    "ele-1",
                    "error",
                    "hasValue() or (children().count() > id.count())",
                ),
                invariant("ext-w", "warning", "extension.exists() != value.exists()"),
                invariant("x-1", "error", "nope.exists() != value.exists()"),
                invariant("x-2", "error", "extension.empty() != value.exists()"),
            ],
            &ext1_index(),
        );
        assert!(checks.is_empty());
    }

    #[test]
    fn scalar_fields() {
        let mut scalars = IndexMap::new();
        scalars.insert("a".to_string(), "a".to_string());
        scalars.insert("b".to_string(), "b".to_string());
        let fields = FieldIndex {
            choices: IndexMap::new(),
            scalars,
            arrays: IndexMap::new(),
        };
        let checks = lower_xor_exists_checks(
            &[invariant("t-1", "error", "a.exists() != b.exists()")],
            &fields,
        );
        assert_eq!(
            checks[0].condition,
            "(self.a is not None) == (self.b is not None)"
        );
    }
}
