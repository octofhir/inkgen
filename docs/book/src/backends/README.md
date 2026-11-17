# Language Backends

InkGen's modular backend architecture allows code generation for multiple languages while sharing the core IR and resolution engine.

## Supported Languages

### TypeScript (Stable)

Modern TypeScript/JavaScript SDK generation with:
- Strongly-typed resource definitions
- Runtime validators (Zod or Valibot)
- Extension and profile support
- Discriminator unions for polymorphic types

See [TypeScript Backend](./typescript.md) for details.

### Python (Pydantic v2)

An importable `fhir` package of Pydantic v2 models:
- One module per resource/complex type, composite classes for backbone elements
- `snake_case` fields aliased to FHIR wire names; wire-faithful JSON serialization, `extra="forbid"`
- Closed `Literal` aliases for required/extensible ValueSet bindings
- `model_validator` runtime checks for `value[x]` choice groups and XOR-exists invariants (e.g. `ext-1`)
- Cross-module cycles resolved via `model_rebuild()` in `__init__.py`

Generated code requires Python >= 3.11 and `pydantic >= 2.11, < 3`. CI typechecks
the generated R4 package with `pyright` and runs runtime smoke tests. Run
`inkgen generate python` (default output `generated/python`).

### Rust (Reference)

`inkgen generate rust` emits compiling structs from the IR alone — a compact
proof that the `PackageIr` contract is language-neutral.

### Gleam (Reference)

`inkgen generate gleam` emits Gleam record types from the IR alone.

## Architecture

Each language backend implements the `Backend` trait from `inkgen-core` — a pure,
synchronous function of a resolved `&PackageIr`, with no provider, no async and no
FHIR resolution:

```rust
pub trait Backend {
    fn id(&self) -> &str;
    fn generate(&self, ir: &PackageIr) -> Result<GenerationOutput, BackendError>;
}
```

Backends:
1. Read the resolved IR and type information (core resolves it once)
2. Render templates (usually Tera or language-specific)
3. Return a `GenerationOutput` (relative path → contents); core writes the files

## Creating a New Backend

See [Extending Backends](./extending.md) for a step-by-step guide to building a new language backend.

## Backend Checklist

For production backends:

- [ ] Full resource type coverage
- [ ] Profile and extension support
- [ ] Value set integration
- [ ] Discriminator union handling
- [ ] Comprehensive test suite
- [ ] Documentation and examples
- [ ] CI/CD integration
- [ ] Performance benchmarks
