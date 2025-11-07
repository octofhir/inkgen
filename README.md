# InkGen

![logo](./inkgen-logo.png)

A Rust-based FHIR code generator that transforms canonical FHIR packages into SDKs for multiple programming languages, starting with TypeScript.

## Overview

InkGen is designed to bridge the gap between FHIR specifications and practical SDK development. It processes canonical FHIR packages and generates type-safe, idiomatic code for target programming languages, enabling developers to work with FHIR resources in a natural way within their preferred development environment.

## Project Status

> ✅ **TASK-0004: TypeScript Backend MVP** is COMPLETE (100% - Production Ready)

> ✅ **TASK-0006: Extensibility & Tooling** is COMPLETE (100% - All Phases Done)

### Completed Features

**TASK-0004 (TypeScript MVP)**
- ✅ CLI provides fetch/generate/config commands powered by the core services
- ✅ **TypeScript generation is fully functional and tested**: 74 tests passing
  - Generates interfaces, nested types, profiles, and structural guards
  - Multiple output modes (interface, class, class_with_builder)
  - Customizable naming conventions and output directory
  - Default output to `./generated` directory
- ⚠️ Value set integration requires additional infrastructure (planned for TASK-0005)

**TASK-0006 (Extensibility & Tooling - Complete)**
- ✅ **Extensible Backend Architecture**: Core `LanguageGenerator` trait enabling third-party backends
- ✅ **Template Overlay System**: Customize built-in templates without forking, with validation and merge strategies
- ✅ **Example Rust Backend**: Complete skeleton implementing programmatic code generation patterns
- ✅ **Performance Instrumentation**: Criterion benchmarks with `just bench` recipe for regression detection
- ✅ **Directory Diff Tooling**: Compare generated outputs with unified diff format and file filtering
- ✅ **Comprehensive Documentation**: README enhancements, CONTRIBUTING.md guide, and architecture documentation

### Upcoming Work
- 📋 TASK-0007: Hardening and Release Prep (release automation, documentation site, feedback channels)
- ❌ Template lint command for overlay validation (future optimization)
- ❌ Other languages (planned for TASK-0008+)

Follow the architecture roadmap in [`docs/adr`](docs/adr) and the work queue in [`docs/tasks`](docs/tasks) for upcoming milestones.

## New Features (TASK-0006)

### Extensible Backend Architecture

Define custom language backends by implementing the `LanguageGenerator` trait:

```rust
use inkgen_core::{LanguageGenerator, PackageDescriptor, StructureDefinitionProvider, StructureProviderConfig};

pub struct MyLanguageGenerator {
    config: MyGeneratorConfig,
}

#[async_trait]
impl<S> LanguageGenerator<S> for MyLanguageGenerator
where
    S: StructureDefinitionProvider + Sync + Send,
{
    async fn generate(
        &self,
        service: &S,
        descriptor: &PackageDescriptor,
        provider_config: &StructureProviderConfig,
    ) -> Result<()> {
        // Your generation logic here
        Ok(())
    }
}
```

### Template Overlay System

Customize TypeScript templates without forking:

```toml
[languages.typescript]
overlays = ["./templates/my-overlays"]
```

Overlay files are merged with built-in templates using the same filename. Validation ensures overlays are syntactically correct before generation.

### Directory Diff Tool

Compare generated outputs:

```bash
inkgen diff --old ./previous --new ./generated --extension .ts --context 5
```

Shows unified diff with statistics on added, removed, and modified files.

### Performance Benchmarks

Run regression benchmarks:

```bash
just bench
```

Benchmarks measure IR construction, profile resolution, template rendering, and code generation performance using Criterion.

## Workspace Layout

```
inkgen/
├── crates/
│   ├── inkgen-cli/           # CLI with fetch/generate/config/diff commands
│   ├── inkgen-core/          # Shared types, traits, and services
│   ├── inkgen-typescript/    # TypeScript backend with overlay support
│   ├── inkgen-rust/          # Example Rust backend (programmatic generation)
│   └── inkgen-testing/       # Shared testing helpers
├── justfile                  # Local automation entrypoints
└── .github/workflows/        # CI skeleton mirroring `just review`
```

## Prerequisites

- Rust stable toolchain (edition 2024 support)
- [`just`](https://github.com/casey/just) command runner

Install `just` via cargo:

```bash
cargo install just
```

## Quick Start

1. Clone the repository:
   ```bash
   git clone https://github.com/octofhir/inkgen.git
   cd inkgen
   ```
2. Bootstrap local tooling:
   ```bash
   just bootstrap
   ```
3. Run the default validation suite:
   ```bash
   just review
   ```
4. Explore the CLI:
```bash
cargo run -p inkgen-cli -- help
```

### Generate TypeScript SDK (MVP)

```bash
inkgen config init --force                  # create/overwrite inkgen.toml
inkgen fetch                                # download configured packages
inkgen generate typescript                  # emit TypeScript into target/inkgen/out/typescript
```

#### Supported TypeScript Generation Features

- **Resource Interfaces**: Type-safe TypeScript interfaces for all FHIR resources
- **Nested Types**: BackboneElement types generated as separate exported interfaces
- **Mode Selection**: Choose between `interface` (default), `class`, or `class_with_builder` output
- **Naming Conventions**: Support for PascalCase, camelCase, and snake_case field naming
- **Structural Guards**: Optional type guard functions for runtime validation
- **Profiles**: Generate profile interfaces that extend base resources with constraints
- **Deterministic Output**: Using IndexMap for consistent, sortable output

#### CLI Options

- `--dry-run` — Preview work without writing files
- `--offline` — Require all packages to be pre-cached
- `--output <dir>` — Redirect output directory (default: `./generated`)
- `--mode <mode>` — Choose generation mode: `interface`, `class`, or `class_with_builder`
- `--naming <convention>` — Choose naming: `pascal` (default), `camel`, or `snake`

#### Configuration

Update `inkgen.toml` to customize TypeScript generation:

```toml
[languages.typescript]
mode = "interface"                  # interface, class, or class_with_builder
naming_convention = "pascal"        # pascal, camel, or snake
structural_guards = true            # emit type guard functions
generate_profiles = true            # emit profile constraints
output_structure = "flat"           # flat or by_package
```

### Shell Completions

```bash
inkgen config completions bash --output completions/inkgen.bash
```

## Available `just` Commands

- `just bootstrap` — Ensure required Rust components (`rustfmt`, `clippy`) are installed.
- `just fetch PACKAGE=<name>` — Run the CLI fetch command (respects `--dry-run`/`--offline` via variables).
- `just generate lang=<backend> config=inkgen.toml` — Delegate to the CLI generator.
- `just test` — Run `cargo test --all`.
- `just snap` — Execute snapshot tests when `cargo-insta` is installed.
- `just bench` — Run Criterion benchmarks with `cargo bench`.
- `just review` — Run fmt, clippy (warnings as errors), and tests.
- `just clean` — Remove build artefacts with `cargo clean`.

Use `just --list` to see recipe parameters and defaults.

## CLI Commands

`inkgen-cli` provides the following commands:

### Package Management
- `inkgen config init` — create (or overwrite via `--force`) a starter `inkgen.toml`.
- `inkgen fetch [--package ...] [--offline] [--dry-run]` — download and cache FHIR packages declared in the manifest.
- `inkgen config validate` — verify manifest structure before running other commands.

### Code Generation
- `inkgen generate typescript [--output <dir>] [--dry-run]` — invoke the TypeScript generator after ensuring packages are available.

### Utilities
- `inkgen diff --old <dir> --new <dir> [--extension <ext>] [--context <lines>]` — compare two generated output directories with unified diff format.
- `inkgen config completions <shell> --output <path>` — emit shell completion scripts.

## Roadmap References

- Architecture decisions: [`docs/adr`](docs/adr)
- Task breakdown and progress logs: [`docs/tasks`](docs/tasks)

Contributions should reference the relevant ADRs and task files to keep context up to date.

## Contributing

We welcome contributions from the community! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines on:

- Setting up your development environment
- Submitting pull requests
- Creating new language backends
- Contributing to documentation

Before participating, please review our [Code of Conduct](CODE_OF_CONDUCT.md).

**Basic Workflow**:
- Use `just bootstrap`, `just test`, and `just review` to validate changes locally
- Keep documentation aligned with your changes
- The CI workflow mirrors `just review`, so a passing run locally should translate to green builds

**For Maintainers**: We use task tracking and ADRs for managing internal work. See [CONTRIBUTING.md](CONTRIBUTING.md) for optional internal practices.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support & Security

- **Issues**: Report bugs and request features via [GitHub Issues](https://github.com/octofhir/inkgen/issues)
- **Discussions**: Join conversations in [GitHub Discussions](https://github.com/octofhir/inkgen/discussions)
- **Documentation**: Full docs available at [InkGen Documentation](docs/book/src/) (mdBook format)
- **Security**: Please report vulnerabilities responsibly via [SECURITY.md](SECURITY.md)

## Roadmap

Active and upcoming milestones are tracked in [`docs/tasks`](docs/tasks); consult those files for the authoritative roadmap.
