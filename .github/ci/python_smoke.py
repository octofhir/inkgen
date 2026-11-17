"""Runtime smoke tests for the generated Python SDK (Pydantic v2).

Usage: python python_smoke.py <generated-output-dir>

The directory must contain the generated `fhir` package (hl7.fhir.r4.core).
"""

import sys

sys.path.insert(0, sys.argv[1])

from pydantic import ValidationError  # noqa: E402

from fhir import Extension, Patient  # noqa: E402


def expect_invalid(label, fn, message_part=None):
    try:
        fn()
    except ValidationError as err:
        if message_part is not None:
            assert message_part in str(err), (
                f"{label}: expected {message_part!r} in error, got: {err}"
            )
    else:
        raise SystemExit(f"{label}: expected ValidationError, but validation passed")


# A valid Patient passes and round-trips through wire-format JSON.
patient = Patient(gender="female", active=True)
assert patient.gender == "female"
wire = patient.model_dump_json()
assert '"gender":"female"' in wire
assert "null" not in wire, f"absent fields must be omitted, got: {wire}"
restored = Patient.model_validate_json(wire)
assert restored.gender == "female"
assert restored.active is True

# An out-of-valueset gender is rejected.
expect_invalid("invalid gender", lambda: Patient(gender="not-a-gender"))

# ext-1: an Extension with both `extension` and `value[x]` is rejected.
nested = Extension(url="http://example.org/fhir/StructureDefinition/nested", value_string="v")
expect_invalid(
    "ext-1 both",
    lambda: Extension(
        url="http://example.org/fhir/StructureDefinition/outer",
        extension=[nested],
        value_string="v",
    ),
    "ext-1",
)

# ext-1: an Extension with neither is rejected too.
expect_invalid(
    "ext-1 neither",
    lambda: Extension(url="http://example.org/fhir/StructureDefinition/empty"),
    "ext-1",
)

# Value-only and extension-only Extensions are accepted.
Extension(url="http://example.org/fhir/StructureDefinition/value-only", value_string="v")
Extension(url="http://example.org/fhir/StructureDefinition/ext-only", extension=[nested])

print("python smoke: OK")