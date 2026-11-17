//! FHIRPath invariant validation code generation.
//!
//! This module handles:
//! - Classifying FHIRPath expressions by complexity
//! - Generating runtime validation functions for evaluable invariants
//! - Creating helpful error messages for constraint violations
//! - Tracking invariant metadata and severity levels

use inkgen_core::ir::{InvariantDefinition, ResourceDefinition};

/// A runtime check lowered from a FHIRPath invariant, emitted inside the
/// generated parse function's refinement block.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InvariantCheck {
    /// Invariant key (e.g. `ext-1`).
    pub key: String,
    /// Violation message, already escaped for a JS double-quoted string.
    pub message: String,
    /// JS boolean expression that is true when the invariant is VIOLATED.
    /// `__OBJ__` stands for the parsed value; the template substitutes the
    /// dialect's variable name.
    pub condition: String,
}

/// Lower error-severity invariants of the form `a.exists() != b.exists()`
/// (exclusive presence, e.g. Extension's `ext-1`: either `extension` or
/// `value[x]`, not both and not neither).
///
/// `choices` maps choice base names (`value`) to their emitted member fields;
/// `field_names`/`array_fields` describe the emitted scalar/collection fields.
/// Invariants referencing anything else, or any other expression shape, are
/// skipped — full FHIRPath evaluation is out of scope.
pub fn lower_xor_exists_checks(
    invariants: &[InvariantDefinition],
    choices: &indexmap::IndexMap<String, Vec<String>>,
    field_names: &std::collections::HashSet<String>,
    array_fields: &std::collections::HashSet<String>,
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
        let Some(lhs) = exists_target(lhs.trim(), choices, field_names, array_fields) else {
            continue;
        };
        let Some(rhs) = exists_target(rhs.trim(), choices, field_names, array_fields) else {
            continue;
        };
        let human = inv.human.as_deref().unwrap_or(expr);
        checks.push(InvariantCheck {
            key: inv.key.clone(),
            message: js_string_escape(&format!("{}: {}", inv.key, human)),
            // XOR is violated when both sides exist or neither does.
            condition: format!("({lhs}) === ({rhs})"),
        });
    }
    checks
}

/// Render one side of an `!=` invariant as a JS `exists()` expression, or
/// `None` when the side is not a plain `<name>.exists()` over a known field.
fn exists_target(
    side: &str,
    choices: &indexmap::IndexMap<String, Vec<String>>,
    field_names: &std::collections::HashSet<String>,
    array_fields: &std::collections::HashSet<String>,
) -> Option<String> {
    let name = side.strip_suffix(".exists()")?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    if let Some(members) = choices.get(name) {
        let listed = members
            .iter()
            .map(|m| format!("__OBJ__.{m}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!("[{listed}].some((x) => x !== undefined)"));
    }
    if !field_names.contains(name) {
        return None;
    }
    if array_fields.contains(name) {
        Some(format!(
            "Array.isArray(__OBJ__.{name}) && __OBJ__.{name}.length > 0"
        ))
    } else {
        Some(format!(
            "__OBJ__.{name} !== undefined && __OBJ__.{name} !== null"
        ))
    }
}

/// Escape a message for embedding in a JS double-quoted string literal.
fn js_string_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

/// Complexity classification for a FHIRPath expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionComplexity {
    /// Simple: Single property check (e.g., "exists(name)" or "birthDate.exists()")
    Simple,
    /// Moderate: Basic operators and boolean logic (e.g., "active and exists(address)")
    Moderate,
    /// Complex: Functions, recursion, or advanced features (e.g., "count() > 1" or "where(...)")
    Complex,
    /// Unknown: Cannot be classified or evaluated
    Unknown,
}

impl ExpressionComplexity {
    /// Lowercase label used in generated comments and metadata.
    pub fn to_string(&self) -> &'static str {
        match self {
            ExpressionComplexity::Simple => "simple",
            ExpressionComplexity::Moderate => "moderate",
            ExpressionComplexity::Complex => "complex",
            ExpressionComplexity::Unknown => "unknown",
        }
    }
}

/// Severity level of an invariant constraint violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvariantSeverity {
    /// Error: Invariant must be satisfied (default)
    Error,
    /// Warning: Invariant violation should be reported but not block processing
    Warning,
}

impl InvariantSeverity {
    /// Map a FHIR `constraint.severity` code, defaulting to `Error`.
    pub fn from_fhir(severity: &str) -> Self {
        match severity.to_lowercase().as_str() {
            "warning" => InvariantSeverity::Warning,
            _ => InvariantSeverity::Error,
        }
    }

    /// Lowercase label used in generated validation output.
    pub fn to_string(&self) -> &'static str {
        match self {
            InvariantSeverity::Error => "error",
            InvariantSeverity::Warning => "warning",
        }
    }
}

/// Metadata about a FHIRPath expression used in invariants.
#[derive(Debug, Clone)]
pub struct InvariantExpression {
    /// The FHIRPath expression text
    pub expression: String,
    /// Classified complexity level
    pub complexity: ExpressionComplexity,
    /// Whether this expression can be evaluated at runtime
    pub is_evaluable: bool,
    /// Supported evaluation context (currently: "Resource")
    pub evaluation_context: String,
}

/// Metadata for generating an invariant validation function.
#[derive(Debug, Clone)]
pub struct ValidationFunction {
    /// Function name (e.g., "validatePatientInd1")
    pub function_name: String,
    /// Invariant key (e.g., "pat-1")
    pub invariant_key: String,
    /// Parameter name (resource variable name)
    pub parameter_name: String,
    /// Parameter type (e.g., "Patient")
    pub parameter_type: String,
    /// Invariant expression metadata
    pub expression: InvariantExpression,
    /// Severity of constraint violation
    pub severity: InvariantSeverity,
    /// Human-readable description of what's being checked
    pub human_description: Option<String>,
}

/// Represents a complete invariant validation strategy.
#[derive(Debug, Clone)]
pub struct InvariantValidator {
    /// All invariants found in the resource
    pub invariants: Vec<InvariantDefinition>,
    /// Validation functions that will be generated
    pub validation_functions: Vec<ValidationFunction>,
    /// Invariants that cannot be evaluated at runtime
    pub unevaluable_invariants: Vec<(String, String)>, // (key, reason)
}

/// Analyze an invariant expression to determine complexity and evaluability.
pub fn analyze_expression(expr: &str) -> InvariantExpression {
    let complexity = classify_complexity(expr);
    let is_evaluable = is_evaluable_expression(expr, &complexity);

    InvariantExpression {
        expression: expr.to_string(),
        complexity,
        is_evaluable,
        evaluation_context: "Resource".to_string(),
    }
}

/// Classify FHIRPath expression complexity by pattern matching.
fn classify_complexity(expr: &str) -> ExpressionComplexity {
    let expr_lower = expr.to_lowercase();

    // Complex patterns: advanced functions, recursion, complex logic
    if expr_lower.contains("where(")
        || expr_lower.contains("select(")
        || expr_lower.contains("ofType(")
        || expr_lower.contains("descendents(")
        || expr_lower.contains("aggregate(")
        || expr_lower.contains("resolve(")
        || expr_lower.contains(".matches(")
    {
        return ExpressionComplexity::Complex;
    }

    // Moderate patterns: count, count comparisons, boolean operators
    if expr_lower.contains("count(")
        || expr_lower.contains(" and ")
        || expr_lower.contains(" or ")
        || expr_lower.contains(" xor ")
        || expr_lower.contains(" implies ")
    {
        return ExpressionComplexity::Moderate;
    }

    // Simple patterns: exists, type checks, property access
    if expr_lower.contains("exists(")
        || expr.contains("?.exists()")
        || expr.contains(".exists()")
        || expr_lower.contains("is(")
        || expr_lower.contains("as(")
    {
        return ExpressionComplexity::Simple;
    }

    ExpressionComplexity::Unknown
}

/// Determine if an expression can be evaluated at runtime.
fn is_evaluable_expression(expr: &str, complexity: &ExpressionComplexity) -> bool {
    // Complex expressions are generally not evaluable without a full FHIRPath engine
    if matches!(complexity, ExpressionComplexity::Complex) {
        return false;
    }

    // Empty or null expressions cannot be evaluated
    if expr.trim().is_empty() {
        return false;
    }

    // Currently we can evaluate simple and moderate expressions
    // This is a conservative estimate - actual evaluation depends on expression syntax
    matches!(
        complexity,
        ExpressionComplexity::Simple | ExpressionComplexity::Moderate
    )
}

/// Generate validation function metadata from an invariant definition.
pub fn create_validation_function(
    invariant: &InvariantDefinition,
    resource_type: &str,
    resource_id: &str,
) -> Option<ValidationFunction> {
    let expression_text = invariant.expression.as_ref()?;
    let expr_analysis = analyze_expression(expression_text);

    // Only create validation functions for evaluable expressions
    if !expr_analysis.is_evaluable {
        return None;
    }

    let severity = invariant
        .severity
        .as_deref()
        .map(InvariantSeverity::from_fhir)
        .unwrap_or(InvariantSeverity::Error);

    let function_name = format!(
        "validate{}{}",
        to_pascal_case(resource_id),
        to_pascal_case(&invariant.key)
    );

    Some(ValidationFunction {
        function_name,
        invariant_key: invariant.key.clone(),
        parameter_name: "resource".to_string(),
        parameter_type: to_pascal_case(resource_type),
        expression: expr_analysis,
        severity,
        human_description: invariant.human.clone(),
    })
}

/// Collect all invariants from a resource and generate validation functions.
pub fn collect_invariant_validators(resource: &ResourceDefinition) -> InvariantValidator {
    let mut validation_functions = Vec::new();
    let mut unevaluable_invariants = Vec::new();

    for invariant in &resource.invariants {
        match create_validation_function(invariant, &resource.id, &invariant.key) {
            Some(func) => validation_functions.push(func),
            None => {
                let reason = if let Some(expr) = invariant.expression.as_ref() {
                    let complexity = classify_complexity(expr);
                    if matches!(complexity, ExpressionComplexity::Complex) {
                        format!("Expression too complex: {}", complexity.to_string())
                    } else {
                        "Expression not evaluable at runtime".to_string()
                    }
                } else {
                    "No expression provided".to_string()
                };
                unevaluable_invariants.push((invariant.key.clone(), reason));
            }
        }
    }

    InvariantValidator {
        invariants: resource.invariants.clone(),
        validation_functions,
        unevaluable_invariants,
    }
}

/// Generate TypeScript code for a validation function condition.
pub fn generate_validation_condition(expr: &InvariantExpression) -> Option<String> {
    let expr_text = &expr.expression;

    // Map simple FHIRPath patterns to TypeScript
    // This is a simplified implementation - full FHIRPath parsing would be more comprehensive
    match expr.complexity {
        ExpressionComplexity::Simple => {
            if expr_text.contains("exists()") || expr_text.contains(".exists()") {
                // "name.exists()" → "resource.name !== undefined && resource.name !== null"
                let prop = expr_text
                    .replace(".exists()", "")
                    .replace("exists(", "")
                    .replace(")", "");
                let prop = prop.trim();
                Some(format!(
                    "resource.{} !== undefined && resource.{} !== null",
                    prop, prop
                ))
            } else {
                None
            }
        }
        ExpressionComplexity::Moderate => {
            // Could support more patterns like "exists(x) and exists(y)"
            None
        }
        _ => None,
    }
}

/// Convert a string to PascalCase for function/type names.
fn to_pascal_case(input: &str) -> String {
    input
        .split(['_', '-', '.'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_simple_exists() {
        let expr = "name.exists()";
        assert_eq!(classify_complexity(expr), ExpressionComplexity::Simple);
    }

    #[test]
    fn test_classify_moderate_and() {
        let expr = "active and birthDate.exists()";
        assert_eq!(classify_complexity(expr), ExpressionComplexity::Moderate);
    }

    #[test]
    fn test_classify_complex_where() {
        let expr = "contact.where(use='home').exists()";
        assert_eq!(classify_complexity(expr), ExpressionComplexity::Complex);
    }

    #[test]
    fn test_classify_complex_count() {
        let expr = "name.count() > 1";
        assert_eq!(classify_complexity(expr), ExpressionComplexity::Moderate);
    }

    #[test]
    fn test_evaluable_simple() {
        let expr = "name.exists()";
        let complexity = classify_complexity(expr);
        assert!(is_evaluable_expression(expr, &complexity));
    }

    #[test]
    fn test_not_evaluable_complex() {
        let expr = "contact.where(use='home').exists()";
        let complexity = classify_complexity(expr);
        assert!(!is_evaluable_expression(expr, &complexity));
    }

    #[test]
    fn test_severity_error_default() {
        assert_eq!(
            InvariantSeverity::from_fhir("unknown"),
            InvariantSeverity::Error
        );
    }

    #[test]
    fn test_severity_warning() {
        assert_eq!(
            InvariantSeverity::from_fhir("warning"),
            InvariantSeverity::Warning
        );
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("pat-1"), "Pat1");
        assert_eq!(to_pascal_case("patient_name"), "PatientName");
        assert_eq!(to_pascal_case("extension.url"), "ExtensionUrl");
    }

    #[test]
    fn test_analyze_expression_simple() {
        let expr = analyze_expression("name.exists()");
        assert_eq!(expr.complexity, ExpressionComplexity::Simple);
        assert!(expr.is_evaluable);
    }

    #[test]
    fn test_generate_validation_condition() {
        let expr = InvariantExpression {
            expression: "name.exists()".to_string(),
            complexity: ExpressionComplexity::Simple,
            is_evaluable: true,
            evaluation_context: "Resource".to_string(),
        };

        let condition = generate_validation_condition(&expr);
        assert!(condition.is_some());
        let cond = condition.unwrap();
        assert!(cond.contains("name"));
        assert!(cond.contains("undefined"));
    }

    fn invariant(key: &str, severity: &str, expression: &str) -> InvariantDefinition {
        InvariantDefinition {
            key: key.to_string(),
            severity: Some(severity.to_string()),
            human: Some("Must have either extensions or value[x], not both".to_string()),
            expression: Some(expression.to_string()),
            xpath: None,
            fhirpath: None,
            additional: indexmap::IndexMap::new(),
        }
    }

    fn xor_inputs() -> (
        indexmap::IndexMap<String, Vec<String>>,
        std::collections::HashSet<String>,
        std::collections::HashSet<String>,
    ) {
        let mut choices = indexmap::IndexMap::new();
        choices.insert(
            "value".to_string(),
            vec!["valueBoolean".to_string(), "valueString".to_string()],
        );
        let fields: std::collections::HashSet<String> =
            ["extension".to_string()].into_iter().collect();
        let arrays = fields.clone();
        (choices, fields, arrays)
    }

    #[test]
    fn xor_exists_lowers_ext1() {
        let (choices, fields, arrays) = xor_inputs();
        let checks = lower_xor_exists_checks(
            &[invariant(
                "ext-1",
                "error",
                "extension.exists() != value.exists()",
            )],
            &choices,
            &fields,
            &arrays,
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].key, "ext-1");
        assert_eq!(
            checks[0].condition,
            "(Array.isArray(__OBJ__.extension) && __OBJ__.extension.length > 0) === \
             ([__OBJ__.valueBoolean, __OBJ__.valueString].some((x) => x !== undefined))"
        );
        assert!(checks[0].message.starts_with("ext-1: "));
    }

    #[test]
    fn xor_exists_skips_unsupported() {
        let (choices, fields, arrays) = xor_inputs();
        let checks = lower_xor_exists_checks(
            &[
                // Not an `!=` expression.
                invariant(
                    "ele-1",
                    "error",
                    "hasValue() or (children().count() > id.count())",
                ),
                // Advisory severity is not enforced.
                invariant("ext-w", "warning", "extension.exists() != value.exists()"),
                // Unknown field name.
                invariant("x-1", "error", "nope.exists() != value.exists()"),
                // Not a plain exists() side.
                invariant("x-2", "error", "extension.empty() != value.exists()"),
            ],
            &choices,
            &fields,
            &arrays,
        );
        assert!(checks.is_empty());
    }

    #[test]
    fn xor_exists_scalar_field() {
        let choices = indexmap::IndexMap::new();
        let fields: std::collections::HashSet<String> =
            ["a".to_string(), "b".to_string()].into_iter().collect();
        let arrays = std::collections::HashSet::new();
        let checks = lower_xor_exists_checks(
            &[invariant("t-1", "error", "a.exists() != b.exists()")],
            &choices,
            &fields,
            &arrays,
        );
        assert_eq!(
            checks[0].condition,
            "(__OBJ__.a !== undefined && __OBJ__.a !== null) === \
             (__OBJ__.b !== undefined && __OBJ__.b !== null)"
        );
    }

    #[test]
    fn js_string_escape_quotes_and_backslashes() {
        assert_eq!(js_string_escape("a\"b\\c\nd"), "a\\\"b\\\\c d");
    }
}
