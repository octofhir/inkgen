# TypeScript Backend

Generate TypeScript code from FHIR StructureDefinitions.

## Overview

The TypeScript backend generates type-safe code from FHIR specifications:
- TypeScript interfaces for FHIR resources
- Zod schemas for runtime validation
- Support for FHIR profiles and extensions
- BackboneElement types as separate interfaces
- Structural type guards

## Basic Usage

```bash
inkgen generate typescript
```

## Generated Output

For a FHIR Patient resource, you get:

```typescript
export interface Patient {
  resourceType: 'Patient';
  id?: string;
  meta?: Meta;
  name?: HumanName[];
  birthDate?: string;
  gender?: 'male' | 'female' | 'other' | 'unknown';
  // ... other fields
}

// Zod schema for validation
export const PatientSchema = z.object({
  resourceType: z.literal('Patient'),
  id: z.string().optional(),
  name: z.array(HumanNameSchema).optional(),
  birthDate: z.string().regex(/^\d{4}(-\d{2}(-\d{2})?)?$/).optional(),
  // ... other fields
});

// Validation function
export function parsePatient(input: unknown): PatientValidated | false {
  const result = PatientSchema.safeParse(input);
  return result.success ? result.data : false;
}
```

## Configuration

Configure TypeScript output in `inkgen.toml`:

```toml
[languages.typescript]
output_dir = "./generated"
mode = "interface"                  # interface, class, or class_with_builder
naming_convention = "pascal"        # pascal, camel, or snake
structural_guards = true            # generate type guard functions
generate_profiles = true            # generate profile types
output_structure = "flat"           # flat or by_package
```

## Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `output_dir` | string | "./generated" | Output directory for generated files |
| `mode` | string | "interface" | Generation mode (interface, class, class_with_builder) |
| `naming_convention` | string | "pascal" | Field naming style (pascal, camel, snake) |
| `structural_guards` | boolean | true | Generate type guard functions |
| `generate_profiles` | boolean | true | Generate profile types |
| `output_structure` | string | "flat" | Output structure (flat, by_package) |

## CLI Options

Override configuration via command-line:

```bash
# Use a different output directory
inkgen generate typescript --output ./src/fhir

# Change generation mode
inkgen generate typescript --mode class

# Change naming convention
inkgen generate typescript --naming camel

# Dry run (preview without writing files)
inkgen generate typescript --dry-run
```

## Advanced Features

### FHIR Profiles

InkGen generates TypeScript interfaces for FHIR profiles:

```typescript
// Base Patient resource
export interface Patient { /* ... */ }

// US Core Patient profile
export interface USCorePatient extends Patient {
  // Profile-specific constraints
}
```

### Template Overlays

Customize generated code by providing template overlays:

```toml
[languages.typescript]
overlays = ["./my-templates"]
```

Create a file at `./my-templates/structure.ts.tera` to override the default structure template.

See [Template Overlays](../advanced/overlays.md) for more details.

### Runtime Validation with Zod

Use the generated Zod schemas to validate data at runtime:

```typescript
import { parsePatient, PatientSchema } from './generated/patient';

// Option 1: Use the parse function
const patient = parsePatient(unknownData);
if (patient) {
  // patient is validated and typed
  console.log(patient.name);
} else {
  console.error('Invalid patient data');
}

// Option 2: Use Zod directly for detailed errors
const result = PatientSchema.safeParse(unknownData);
if (result.success) {
  console.log(result.data);
} else {
  console.error(result.error.issues);
}
```

## Output Modes

### Interface Mode (Default)

Generates plain TypeScript interfaces with Zod schemas:

```typescript
export interface Patient {
  resourceType: 'Patient';
  // ... fields
}
```

### Class Mode

Generates ES6 classes:

```typescript
export class Patient {
  resourceType: 'Patient';
  // ... fields
}
```

### Class with Builder Mode

Generates classes with a fluent builder API:

```typescript
export class Patient {
  resourceType: 'Patient';
  // ... fields

  static builder(): PatientBuilder {
    return new PatientBuilder();
  }
}

const patient = Patient.builder()
  .withName({ family: 'Smith', given: ['John'] })
  .withBirthDate('1980-01-01')
  .build();
```

## Next Steps

- [Extending Backends](./extending.md) - Create custom language backends
- [Template Overlays](../advanced/overlays.md) - Customize generated code
- [Profiles](../advanced/profiles.md) - Working with FHIR profiles
