# Template Overlays

Template overlays allow you to customize how InkGen generates code for specific FHIR resources or profiles.

## Overview

Overlays provide a way to:
- Customize generated code structure
- Add custom annotations
- Control naming conventions
- Extend generated types with additional properties

## Creating an Overlay

Create a `overlays.yaml` file in your project:

```yaml
overlays:
  Patient:
    custom_class_name: "PatientProfile"
    generate_interfaces: true
    add_validation: true

  Observation:
    custom_class_name: "ObservationResult"
    generate_builder_pattern: true
```

## Using Overlays

Reference the overlay file in your configuration:

```json
{
  "overlaysFile": "./overlays.yaml"
}
```

## Common Overlay Options

| Option | Type | Description |
|--------|------|-------------|
| `custom_class_name` | string | Override the generated class name |
| `generate_interfaces` | boolean | Generate interface definitions |
| `add_validation` | boolean | Add validation methods |
| `generate_builder_pattern` | boolean | Generate builder pattern |

## Examples

See [Profile Customization](./profiles.md) for more advanced customization examples.
