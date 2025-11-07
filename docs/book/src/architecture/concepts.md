# Core Concepts

## FHIR Profiles

A FHIR profile constrains a base resource to apply domain-specific rules.

Example:
```fsh
Profile: MyPatient
Parent: Patient
Title: "My custom Patient profile"
```

## Elements and Cardinality

Elements are individual fields within a resource. Cardinality defines min..max occurrences:

```fsh
* name 1..* MS  // 1 or more mandatory
* birthDate 0..1 // 0 or 1 optional
```

## Extensions

Custom data elements that extend the standard FHIR resource:

```fsh
* extension contains race 0..1
```

## Slicing

Discriminating between different types of the same element:

```fsh
* component ^slicing.discriminator.type = #pattern
* component ^slicing.discriminator.path = "code"
```

## Value Sets and Codings

Constrain coded elements to specific value sets:

```fsh
* status from ObservationStatus (required)
```

For more information, see [IR Design](./ir-design.md).
