# Language Backends

InkGen's modular backend architecture allows code generation for multiple languages while sharing the core IR and resolution engine.

## Supported Languages

### TypeScript (Stable)

Modern TypeScript/JavaScript SDK generation with:
- Strongly-typed resource definitions
- Runtime validators
- Extension and profile support
- Discriminator unions for polymorphic types

See [TypeScript Backend](./typescript.md) for details.

## Architecture

Each language backend implements the `LanguageGenerator` trait:

```rust
pub trait LanguageGenerator<S: StructureDefinitionProvider> {
    async fn generate(
        &self,
        service: &S,
        descriptor: &PackageDescriptor,
        config: &StructureProviderConfig,
    ) -> Result<()>;
}
```

Backends:
1. Receive the parsed IR and type information
2. Render templates (usually Tera or language-specific)
3. Output formatted code

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
