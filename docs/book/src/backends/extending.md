# Extending Backends

Create custom code generators by extending InkGen's backend architecture.

## Backend Structure

A backend is a module that implements the `Backend` interface:

```typescript
export interface Backend {
  name: string;
  generate(ir: IntermediateRepresentation): GeneratedCode[];
  validate(): ValidationResult;
}
```

## Creating a Custom Backend

### 1. Implement the Backend Interface

```typescript
import { Backend, IntermediateRepresentation } from '@octofhir/inkgen';

export class MyBackend implements Backend {
  name = 'mybackend';

  generate(ir: IntermediateRepresentation) {
    // Your generation logic
    return [];
  }

  validate() {
    return { valid: true };
  }
}
```

### 2. Register the Backend

```typescript
import { registerBackend } from '@octofhir/inkgen';
import { MyBackend } from './my-backend';

registerBackend(new MyBackend());
```

### 3. Use in Configuration

```json
{
  "backends": ["mybackend"]
}
```

## Example: Python Backend

Create a simple Python backend:

```typescript
export class PythonBackend implements Backend {
  name = 'python';

  generate(ir: IntermediateRepresentation) {
    return ir.profiles.map(profile => ({
      path: `${profile.name}.py`,
      content: `
class ${profile.name}:
    def __init__(self):
        pass
`
    }));
  }

  validate() {
    return { valid: true };
  }
}
```

## Best Practices

1. **Handle all FHIR types** - Support all types in the IR
2. **Generate idiomatic code** - Follow language conventions
3. **Include validation** - Add runtime type checking
4. **Document output** - Generate comments explaining types
5. **Test thoroughly** - Validate against sample data

For more details on the IR structure, see [IR Design](../architecture/ir-design.md).
