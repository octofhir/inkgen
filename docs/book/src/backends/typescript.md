# TypeScript Backend

Generate TypeScript code from FHIR StructureDefinitions.

## Overview

The TypeScript backend generates type-safe code from FHIR specifications:
- TypeScript interfaces for FHIR resources
- **ValueSet types**: Const arrays, union types, and type guards
- **Profile classes**: Extension accessors, serialization, and validation
- Zod schemas for runtime validation
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
generate_valuesets = true           # generate valueset types
max_valueset_codes = 100            # skip large valuesets (optional)
output_structure = "flat"           # flat or by_package

# Profile method configuration
[languages.typescript.profile_methods]
extension_accessors = true          # generate extension getter/setter methods
extension_style = "both"            # "typed", "raw", or "both"
serialization = true                # generate toJson/toObject methods
validation = true                   # generate fromJson/fromObject methods
generate_zod_schemas = true         # generate Zod schemas for profiles
```

## Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `output_dir` | string | "./generated" | Output directory for generated files |
| `mode` | string | "interface" | Generation mode (interface, class, class_with_builder) |
| `naming_convention` | string | "pascal" | Field naming style (pascal, camel, snake) |
| `structural_guards` | boolean | true | Generate type guard functions |
| `generate_profiles` | boolean | true | Generate profile types |
| `generate_valuesets` | boolean | true | Generate valueset types |
| `max_valueset_codes` | number | none | Skip valuesets with more codes than this limit |
| `output_structure` | string | "flat" | Output structure (flat, by_package) |

### Profile Method Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `extension_accessors` | boolean | true | Generate extension getter/setter methods |
| `extension_style` | string | "both" | Extension accessor style: "typed", "raw", or "both" |
| `serialization` | boolean | true | Generate toJson/toObject methods |
| `validation` | boolean | true | Generate fromJson/fromObject methods |
| `generate_zod_schemas` | boolean | true | Generate Zod schemas for runtime validation |

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

## ValueSets

ValueSets are generated as type-safe const arrays with union types and type guards.

### Generated ValueSet Code

For a FHIR ValueSet like AdministrativeGender:

```typescript
/**
 * Administrative Gender
 *
 * The gender of a person used for administrative purposes
 * @see http://hl7.org/fhir/ValueSet/administrative-gender
 */
export const AdministrativeGenderValues = [
  "male", "female", "other", "unknown"
] as const;

// Type alias from const array
export type AdministrativeGender = typeof AdministrativeGenderValues[number];

// Type guard function
export function isAdministrativeGender(value: string): value is AdministrativeGender {
  return AdministrativeGenderValues.includes(value as any);
}

// Code definitions with display names and descriptions
export const AdministrativeGenderDefinitions = {
  "male": {
    code: "male",
    display: "Male",
  },
  "female": {
    code: "female",
    display: "Female",
  },
  "other": {
    code: "other",
    display: "Other",
  },
  "unknown": {
    code: "unknown",
    display: "Unknown",
  },
} as const;
```

### Binding Strength

ValueSets respect FHIR binding strength:

**Required/Extensible Bindings** - Closed types (only specified codes):
```typescript
export type AccountStatus = typeof AccountStatusValues[number];
// = "active" | "inactive" | "entered-in-error"
```

**Preferred/Example Bindings** - Open types (allow custom codes):
```typescript
// Open valueset - allows custom codes
export type ConditionSeverity =
  typeof ConditionSeverityValues[number] | (string & {});
// = "mild" | "moderate" | "severe" | (string & {})
```

The `| (string & {})` pattern allows any string while maintaining autocomplete for known values.

### Using ValueSets

```typescript
import {
  AdministrativeGender,
  isAdministrativeGender,
  AdministrativeGenderDefinitions
} from './generated/valuesets';

// Type-safe usage
const gender: AdministrativeGender = "male"; // ✓
const invalid: AdministrativeGender = "invalid"; // ✗ Type error

// Runtime validation
function validateGender(input: string): AdministrativeGender | null {
  return isAdministrativeGender(input) ? input : null;
}

// Access metadata
const maleInfo = AdministrativeGenderDefinitions.male;
console.log(maleInfo.display); // "Male"
```

### ValueSet Size Limits

Configure `max_valueset_codes` to skip large valuesets:

```toml
[languages.typescript]
max_valueset_codes = 100  # Skip valuesets with >100 codes
```

This prevents generating extremely large union types that can slow down TypeScript compilation.

## FHIR Profiles

InkGen generates TypeScript classes for FHIR profiles with extension accessors, serialization, and validation.

### Generated Profile Class

For a US Core Patient profile:

```typescript
import type { Coding, Extension } from "./types";
import { z } from 'zod';
import { PatientSchema } from "./Patient";

/**
 * US Core Patient Profile
 *
 * Defines constraints and extensions on the Patient resource for US Core
 * @profile http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient
 */
export class USCorePatient extends Patient {
  /** Profile URL for runtime validation */
  readonly __profile = 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient';

  // Fixed value constraints
  /** Fixed value: true */
  declare active: true;

  // Must-support elements (required by profile)
  /** Required by profile: Patient.identifier */
  declare identifier: NonNullable<Patient['identifier']>;
  /** Required by profile: Patient.name */
  declare name: NonNullable<Patient['name']>;

  // Extension accessors - Typed value access
  /**
   * Get the race extension value
   * @see http://hl7.org/fhir/us/core/StructureDefinition/us-core-race
   */
  get uSCoreRace(): Coding | undefined {
    const ext = this.extension?.find(
      e => e.url === 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-race'
    );
    return ext?.valueCoding;
  }

  /**
   * Set the race extension value
   */
  set uSCoreRace(value: Coding | undefined) {
    if (!this.extension) {
      this.extension = [];
    }
    const existingIndex = this.extension.findIndex(
      e => e.url === 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-race'
    );
    const newExt = value !== undefined
      ? { url: 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-race', valueCoding: value }
      : undefined;

    if (existingIndex >= 0) {
      if (newExt) {
        this.extension[existingIndex] = newExt;
      } else {
        this.extension.splice(existingIndex, 1);
      }
    } else if (newExt) {
      this.extension.push(newExt);
    }
  }

  // Extension accessors - Raw Extension access
  /**
   * Get the raw race Extension object
   */
  get uSCoreRaceExtension(): Extension | undefined {
    return this.extension?.find(
      e => e.url === 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-race'
    );
  }

  /**
   * Set the raw race Extension object
   */
  set uSCoreRaceExtension(value: Extension | undefined) {
    // ... implementation
  }

  // Serialization methods
  /**
   * Serialize to JSON string
   * @param pretty Whether to pretty-print the JSON
   */
  toJson(pretty?: boolean): string {
    return pretty ? JSON.stringify(this, null, 2) : JSON.stringify(this);
  }

  /**
   * Convert to plain object (strips __profile metadata)
   */
  toObject(): Patient {
    const { __profile, ...rest } = this;
    return rest as Patient;
  }

  // Validation methods
  /**
   * Create instance from JSON string with validation
   * @throws {z.ZodError} if validation fails
   */
  static fromJson(json: string): USCorePatient {
    const parsed = JSON.parse(json);
    return USCorePatient.fromObject(parsed);
  }

  /**
   * Create instance from object with validation
   * @throws {z.ZodError} if validation fails
   */
  static fromObject(obj: unknown): USCorePatient {
    const validated = USCorePatientSchema.parse(obj);
    return Object.assign(new USCorePatient(), validated);
  }
}

/**
 * Zod schema for USCorePatient
 */
export const USCorePatientSchema = PatientSchema.extend({
  identifier: z.array(z.unknown()).min(1),
  name: z.array(z.unknown()).min(1),
});

/**
 * Type inferred from Zod schema
 */
export type USCorePatientValidated = z.infer<typeof USCorePatientSchema>;

/**
 * Type guard to check if a value is a USCorePatient
 */
export function isUSCorePatient(value: Patient): value is USCorePatient {
  return '__profile' in value &&
    value.__profile === 'http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient';
}
```

### Extension Accessor Styles

Configure how extension accessors are generated:

**Typed Style** (`extension_style = "typed"`):
```typescript
get uSCoreRace(): Coding | undefined { /* ... */ }
set uSCoreRace(value: Coding | undefined) { /* ... */ }
```

**Raw Style** (`extension_style = "raw"`):
```typescript
get uSCoreRaceExtension(): Extension | undefined { /* ... */ }
set uSCoreRaceExtension(value: Extension | undefined) { /* ... */ }
```

**Both Style** (`extension_style = "both"`) - Default:
Generates both typed and raw accessors for maximum flexibility.

### Using Profile Classes

```typescript
import { USCorePatient } from './generated/profiles/USCorePatient';

// Create instance
const patient = new USCorePatient();
patient.identifier = [{ system: "http://hospital.example.org", value: "12345" }];
patient.name = [{ family: "Smith", given: ["John"] }];
patient.active = true;

// Use typed extension accessor
patient.uSCoreRace = {
  system: "urn:oid:2.16.840.1.113883.6.238",
  code: "2106-3",
  display: "White"
};

// Serialize
const json = patient.toJson(true);
const plainObject = patient.toObject();

// Deserialize with validation
try {
  const validated = USCorePatient.fromJson(jsonString);
  console.log(validated.name);
} catch (error) {
  console.error("Validation failed:", error);
}

// Runtime type checking
if (isUSCorePatient(somePatient)) {
  // TypeScript knows this is USCorePatient
  console.log(somePatient.uSCoreRace);
}
```

### Profile Method Configuration

Control which methods are generated:

```toml
[languages.typescript.profile_methods]
extension_accessors = false     # Disable extension accessors
serialization = false           # Disable toJson/toObject
validation = false              # Disable fromJson/fromObject
generate_zod_schemas = false    # Disable Zod schema generation
```

**Minimal Configuration** (only type constraints):
```typescript
export class USCorePatient extends Patient {
  readonly __profile = '...';
  declare identifier: NonNullable<Patient['identifier']>;
  declare name: NonNullable<Patient['name']>;
}
```

This provides pure type safety without runtime overhead.

## Template Overlays

Customize generated code by providing template overlays:

```toml
[languages.typescript]
overlays = ["./my-templates"]
```

Create a file at `./my-templates/structure.ts.tera` to override the default structure template.

See [Template Overlays](../advanced/overlays.md) for more details.

## Runtime Validation with Zod

Use the generated Zod schemas to validate data at runtime:

```typescript
import { parsePatient, PatientSchema } from './generated/patient';
import { USCorePatientSchema } from './generated/profiles/USCorePatient';

// Option 1: Use the parse function
const patient = parsePatient(unknownData);
if (patient) {
  // patient is validated and typed
  console.log(patient.name);
} else {
  console.error('Invalid patient data');
}

// Option 2: Use Zod directly for detailed errors
const result = USCorePatientSchema.safeParse(unknownData);
if (result.success) {
  console.log(result.data);
} else {
  console.error(result.error.issues);
}

// Option 3: Use profile validation method
try {
  const validated = USCorePatient.fromObject(unknownData);
  console.log("Valid profile:", validated);
} catch (error) {
  console.error("Profile validation failed:", error);
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

## Best Practices

### Type Safety

- Use generated type guards for runtime validation
- Leverage TypeScript's discriminated unions for resource types
- Use Zod schemas for external data validation

### Performance

- Set `max_valueset_codes` to avoid extremely large union types
- Use `output_structure = "by_package"` for large codebases to improve IDE performance
- Disable unused features (e.g., `generate_profiles = false` if not using profiles)

### Extension Handling

- Use **typed accessors** for simple value-based extensions
- Use **raw accessors** for complex extensions with nested structure
- Use **both** when you need flexibility

### Validation Strategy

- Use Zod schemas for user input and external APIs
- Use type guards for narrowing types at runtime
- Consider disabling Zod for internal-only code to reduce bundle size

## Next Steps

- [Extending Backends](./extending.md) - Create custom language backends
- [Template Overlays](../advanced/overlays.md) - Customize generated code
- [Profiles](../advanced/profiles.md) - Working with FHIR profiles
