# Profile Customization

Learn how to customize FHIR profiles to meet your specific requirements.

## Profile Basics

A profile constrains a FHIR resource to add domain-specific requirements.

## Extending Resources

```fsh
Profile: CustomPatient
Parent: Patient
Title: "Custom Patient Profile"

// Add a new extension
* extension contains
    race 0..1 MS and
    ethnicity 0..1 MS

// Constrain cardinality
* name 1..* MS
* birthDate 1..1 MS
```

## Using Extensions

```fsh
Extension: PatientRace
Id: patient-race
Title: "Patient Race"
Description: "The patient's race"
* value[x] only CodeableConcept
```

## Slicing

Slice elements to distinguish between different types:

```fsh
Profile: ComplexObservation
Parent: Observation

* component ^slicing.discriminator.type = #pattern
* component ^slicing.discriminator.path = "code"
* component ^slicing.ordered = false
* component ^slicing.rules = #open

* component contains
    systolic 0..1 and
    diastolic 0..1

* component[systolic].code = $LOINC#8480-6
* component[diastolic].code = $LOINC#8462-4
```

## Best Practices

1. **Keep profiles focused** - Each profile should represent a single clinical concept
2. **Document constraints** - Explain why constraints exist
3. **Test thoroughly** - Validate profiles against sample data
4. **Version your profiles** - Use semantic versioning for profile changes

See [Performance Tuning](./performance.md) for optimization tips.
