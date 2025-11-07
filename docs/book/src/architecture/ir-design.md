# IR Design

The Intermediate Representation (IR) is InkGen's internal model for representing FHIR profiles.

## IR Structure

The IR provides:
- **Resource Definition** - Base resource and applied profile
- **Elements** - Fields with types, cardinality, and constraints
- **Extensions** - Custom data elements
- **Slices** - Discriminated element variants
- **Bindings** - Value set bindings to coded elements

## IR to Code Generation

The IR is designed to be:
- **Language-agnostic** - Can generate code for any language
- **Complete** - Contains all information needed for validation
- **Efficient** - Optimized for traversal and code generation
- **Extensible** - Can be extended with custom information

## Example IR

```json
{
  "name": "MyPatient",
  "parent": "Patient",
  "elements": [
    {
      "path": "name",
      "type": "HumanName",
      "min": 1,
      "max": "*",
      "isMustSupport": true
    },
    {
      "path": "birthDate",
      "type": "date",
      "min": 0,
      "max": 1
    }
  ]
}
```

See [Type System](./type-system.md) for details on type representation.
