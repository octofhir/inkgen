# InkGen Rust Backend

An example FHIR code generator backend for Rust that demonstrates how to implement the `LanguageGenerator` trait.

## Features

- Generates idiomatic Rust structs with `serde` support
- Programmatic code generation (no templates required)
- Module index file for easy imports
- Demonstrates backend architecture extensibility

## Building

```bash
cargo build -p inkgen-rust
```

## Testing

```bash
cargo test -p inkgen-rust --lib
```

## Architecture

This backend demonstrates key architectural patterns:

1. **Generator Implementation**: Implements `LanguageGenerator<S>` trait from `inkgen-core`
2. **Configuration**: Uses manifest-driven configuration via `RustLanguageConfig`
3. **Programmatic Emission**: Generates code directly without template engine
4. **Error Handling**: Proper error propagation with context

## Design Decisions

### No Templates
Unlike the TypeScript backend, this backend generates Rust code programmatically. This demonstrates that backends don't need to use Tera templates—they can use any code generation approach.

### Module Structure
- Each FHIR structure generates its own module file
- Central `mod.rs` provides index and re-exports
- Follows Rust module naming conventions

### Serialization
Generated structs use `serde` with explicit crate specification:
```rust
#[derive(Serialize, Deserialize)]
#[serde(crate = "serde")]
```

## Extension Points

Extend this backend by:

1. **Custom Builders**: Add builder patterns for complex types
2. **Validation Helpers**: Implement FHIR constraint validation
3. **Custom Derives**: Add domain-specific derive macros
4. **Output Organization**: Customize file/module structure

## Example Usage

From manifest:

```toml
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[languages.rust]
output_dir = "./src/generated"
```

## Testing Strategy

The backend includes unit tests for:
- Configuration creation and defaults
- Module index generation
- Generator initialization

## Future Enhancements

- [ ] Builder pattern generation
- [ ] Validation helper generation
- [ ] Custom trait derives
- [ ] Async client code generation
- [ ] Feature-gated serialization options
