# Contributing to InkGen

Thank you for your interest in contributing to InkGen! This document provides guidance for developers working on the project, with a focus on contributing new language backends.

## Getting Started

### Prerequisites

- Rust stable toolchain (edition 2024)
- [`just`](https://github.com/casey/just) command runner
- [`cargo-insta`](https://insta.rs/) for snapshot testing (optional but recommended)

### Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/octofhir/inkgen.git
   cd inkgen
   ```

2. Bootstrap the workspace:
   ```bash
   just bootstrap
   ```

3. Run the validation suite:
   ```bash
   just review
   ```

## Project Structure

```
crates/
├── inkgen-cli/           # CLI application
├── inkgen-core/          # Shared types, traits, and services
├── inkgen-typescript/    # TypeScript backend implementation
├── inkgen-rust/          # Example Rust backend
└── inkgen-testing/       # Testing utilities
```

## Adding a New Language Backend

InkGen is designed to be extensible. This guide walks through adding a new backend.

### Step 1: Create a New Crate

Create a new crate in the `crates/` directory:

```bash
cargo new crates/inkgen-<language>
```

Update `Cargo.toml` to add workspace dependencies:

```toml
[package]
name = "inkgen-<language>"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
anyhow = { workspace = true }
async-trait = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
inkgen-core = { path = "../inkgen-core" }

[dev-dependencies]
tempfile = { workspace = true }
```

Add the crate to the workspace `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing crates ...
    "crates/inkgen-<language>",
]
```

### Step 2: Implement the LanguageGenerator Trait

Create `src/lib.rs` and implement the core generator:

```rust
use anyhow::Result;
use async_trait::async_trait;
use inkgen_core::{
    LanguageGenerator, PackageDescriptor, StructureDefinitionProvider, StructureProviderConfig,
};

pub struct <Language>Generator {
    config: <Language>GeneratorConfig,
}

#[derive(Debug, Clone)]
pub struct <Language>GeneratorConfig {
    pub output_dir: std::path::PathBuf,
}

impl <Language>Generator {
    pub fn new(config: <Language>GeneratorConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &<Language>GeneratorConfig {
        &self.config
    }
}

#[async_trait]
impl<S> LanguageGenerator<S> for <Language>Generator
where
    S: StructureDefinitionProvider + Sync + Send,
{
    async fn generate(
        &self,
        _service: &S,
        descriptor: &PackageDescriptor,
        _provider_config: &StructureProviderConfig,
    ) -> Result<()> {
        tracing::info!(
            "Starting <Language> generation for package: {} v{}",
            descriptor.id.name,
            descriptor.id.version
        );

        let structures = descriptor.structures();

        tracing::info!("Found {} structures to generate", structures.len());

        // Implement your generation logic here

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_initialization() {
        let config = <Language>GeneratorConfig {
            output_dir: std::path::PathBuf::from("./test"),
        };
        let generator = <Language>Generator::new(config);
        assert_eq!(generator.config().output_dir.to_str().unwrap(), "./test");
    }
}
```

### Step 3: Add Configuration Support

Create `src/config.rs` for manifest integration:

```rust
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct <Language>Config {
    pub output_dir: Option<PathBuf>,
    // ... other configuration fields ...
}

impl <Language>Config {
    pub fn from_manifest(config: Option<&<Language>Config>, default_output: PathBuf) -> Self {
        config.cloned().unwrap_or_else(|| Self {
            output_dir: Some(default_output),
        })
    }
}
```

### Step 4: Integrate with the CLI

Update `crates/inkgen-cli/src/main.rs` to support your backend:

1. Add imports:
```rust
use inkgen_<language>::<Language>Generator;
```

2. Add a subcommand variant:
```rust
#[derive(Subcommand, Debug)]
enum GenerateSubcommand {
    Typescript(GenerateTypescriptArgs),
    <Language>(Generate<Language>Args),
}
```

3. Create an argument struct:
```rust
#[derive(Args, Debug)]
struct Generate<Language>Args {
    #[command(flatten)]
    shared: SharedCacheArgs,
    /// Output directory
    #[arg(long)]
    output: Option<PathBuf>,
}
```

4. Add handler and main match arm.

### Step 5: Test Your Backend

Write comprehensive tests in `src/lib.rs` and integration tests in `tests/`:

```bash
cargo test -p inkgen-<language>
```

Run the full validation suite:

```bash
just review
```

## Development Workflow

### Testing

- **Unit tests**: Inline with implementation using `#[cfg(test)]` modules
- **Integration tests**: Create `tests/` directory for end-to-end tests
- **Snapshot tests**: Use `cargo-insta` for comparing generated output

Run tests:

```bash
cargo test --all                    # Run all tests
cargo test -p <crate> <filter>     # Run specific tests
just snap                           # Review snapshot changes
```

### Code Quality

The CI pipeline enforces:

- **Formatting**: `cargo fmt --all`
- **Linting**: `cargo clippy --all -- -D warnings`
- **Testing**: All tests must pass

Validate locally before pushing:

```bash
just review
```

### Performance

Benchmarks help detect performance regressions:

```bash
just bench
```

Add benchmarks to `benches/` directory and update the Criterion configuration.

## Template Overlays

If your backend uses Tera templates (like TypeScript), you can allow customization via overlays:

```toml
[languages.typescript]
overlays = ["./templates/custom"]
```

Overlay templates with the same filename override built-in templates. See `crates/inkgen-typescript/src/overlays.rs` for implementation details.

## Code Style

- Follow Rust conventions (use `rustfmt`)
- Use meaningful variable and function names
- Add documentation comments (`///`) to public APIs
- Keep functions focused and relatively short
- Use `Result<T>` for fallible operations

## Documentation

- Update relevant sections in `README.md`
- Add inline documentation for public APIs
- Include examples in doc comments for complex APIs
- Consider updating architecture documentation if your changes affect design

## Pull Request Guidelines

1. **Keep commits clean**: Use descriptive commit messages
2. **Test thoroughly**: Ensure `just review` passes locally
3. **Update documentation**: Keep README and other docs in sync with your changes
4. **Add tests**: New features must include tests
5. **Use templates**: Fill out the [PR template](.github/PULL_REQUEST_TEMPLATE.md)

### Optional (for maintainers)

These practices are used by the InkGen team for managing internal work:

- Reference task files from `docs/tasks/` when working on major features
- Create Architecture Decision Records (ADRs) in `docs/adr/` for significant design changes
- Update progress notes in task files as you work

## Release Process

### Release Cadence

InkGen follows semantic versioning:
- **Minor releases** (0.1.x → 0.2.x): New features, typically monthly
- **Patch releases** (0.1.0 → 0.1.1): Bug fixes, as needed
- **Major releases** (0.x.x → 1.x.x): Breaking changes, scheduled

### Release Checklist

Before releasing:

```bash
# Ensure all checks pass
just review

# Run release dry-run
just release-dry-run

# Update CHANGELOG.md with git-cliff
cargo install git-cliff
git-cliff --output CHANGELOG.md

# Tag and push
git tag v0.X.Y
git push origin v0.X.Y
```

The GitHub Actions `release-dry-run` workflow will:
1. Build CLI binaries for Linux, macOS, Windows
2. Generate changelog preview
3. Validate package publish settings
4. Upload artifacts

### Publishing

Only `inkgen-cli` is published to crates.io. Other crates have `publish = false`.

To publish:

```bash
cargo publish -p inkgen-cli
```

## Triage & Labeling

Issues are triaged using labels:

- `bug` - Something isn't working
- `enhancement` - New feature or improvement
- `documentation` - Documentation updates
- `help wanted` - Seeking community help
- `good first issue` - Good for newcomers
- `template-overlay` - Template customization requests
- `question` - User questions
- `blocked` - Waiting on external dependency
- `wontfix` - Won't be implemented

### Triage Responsibilities

- Review new issues within 48 hours
- Label with appropriate category
- Ask for clarification if needed
- Link to related issues
- Move to GitHub Project board for tracking

## Code of Conduct

We've adopted the [Contributor Covenant](CODE_OF_CONDUCT.md). Please review it before participating.

## Reporting Issues

Found a bug? Please report it via [GitHub Issues](https://github.com/octofhir/inkgen/issues) with:

- A clear description of the issue
- Steps to reproduce
- Expected vs. actual behavior
- Your environment (Rust version, OS, etc.)

## Architecture References

- [Architecture Decision Records](docs/adr/) — Design decisions and rationales
- [Task Tracking](docs/tasks/) — Feature work and progress notes
- [InkGen Core Documentation](crates/inkgen-core/src/lib.rs) — API documentation

## Questions?

- Check existing [GitHub Issues](https://github.com/octofhir/inkgen/issues)
- Start a discussion in [GitHub Discussions](https://github.com/octofhir/inkgen/discussions)
- Review the ADRs in `docs/adr/`

## License

Your contributions will be licensed under the MIT License, consistent with the project.
