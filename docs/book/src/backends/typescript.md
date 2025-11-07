# TypeScript Backend

Generate TypeScript interfaces and types from FHIR profiles.

## Overview

The TypeScript backend generates:
- TypeScript interfaces for each profile
- Type guards and validation functions
- Optional JSON schema generation
- Comprehensive JSDoc comments

## Basic Usage

```bash
npx inkgen generate MyProfile.fsh --backend typescript
```

## Generated Output

For a profile like:

```fsh
Profile: MyPatient
Parent: Patient

* name 1..* MS
* birthDate 1..1 MS
```

You get:

```typescript
export interface MyPatient extends Patient {
  name: HumanName[];
  birthDate: string;
}

export function isMyPatient(resource: unknown): resource is MyPatient {
  // validation logic
}
```

## Configuration

Control TypeScript output with options:

```json
{
  "backends": {
    "typescript": {
      "exportFormat": "esm",
      "generateValidation": true,
      "generateJsonSchema": false,
      "strictNullChecks": true
    }
  }
}
```

## Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `exportFormat` | string | esm | Export format (esm, cjs) |
| `generateValidation` | boolean | true | Generate validation functions |
| `generateJsonSchema` | boolean | false | Generate JSON Schema |
| `strictNullChecks` | boolean | true | Enable strict null checks |

## Advanced Features

### Validation

Generated validation functions check cardinality, types, and bindings:

```typescript
const result = validateMyPatient(data);
if (!result.valid) {
  console.error(result.errors);
}
```

### JSON Schema

Generate JSON Schema for use with schema validators:

```typescript
import { MyPatientSchema } from './schemas';
```

See [Extending Backends](./extending.md) for custom output.
