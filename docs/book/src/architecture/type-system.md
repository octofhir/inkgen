# Type System

InkGen's type system maps FHIR types to implementation-specific types.

## FHIR Primitive Types

| FHIR Type | JavaScript | Java | Notes |
|-----------|-----------|------|-------|
| string | string | String | Text data |
| boolean | boolean | Boolean | True/false |
| integer | number | int | Whole numbers |
| decimal | number | BigDecimal | Precise decimals |
| date | Date | LocalDate | Calendar date |
| dateTime | Date | ZonedDateTime | With timezone |
| code | string | String | Coded value |

## FHIR Complex Types

| FHIR Type | Description |
|-----------|-------------|
| CodeableConcept | Coding with text |
| Coding | Code system reference |
| Identifier | Business identifier |
| Quantity | Numeric value with units |
| Period | Start and end time |
| Ratio | Numeric ratio |

## Custom Types

Profiles can define custom types that extend or constrain base types:

```fsh
Profile: CustomQuantity
Parent: Quantity

* system 1..1 MS
* value 1..1 MS
```

## Type Constraints

Cardinality, patterns, and value set bindings form the type constraint system:

```fsh
* value[x] only Quantity or string
* code from MyValueSet (required)
```

For more on code generation, see [Language Backends](../backends/README.md).
