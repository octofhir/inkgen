# InkGen

![logo](./inkgen-logo.png)

A Rust-based FHIR code generator that transforms canonical FHIR packages into SDKs for multiple programming languages, starting with TypeScript.

## Overview

InkGen is designed to bridge the gap between FHIR specifications and practical SDK development. It processes canonical FHIR packages and generates type-safe, idiomatic code for target programming languages, enabling developers to work with FHIR resources in a natural way within their preferred development environment.

## Project Status

> ✅ Workspace bootstrap baseline is complete (see [`docs/tasks/TASK-0001-workspace-bootstrap.md`](docs/tasks/TASK-0001-workspace-bootstrap.md)).

- CLI now provides fetch/generate/config commands powered by the core services.
- TypeScript generation remains a stub until TASK-0004; other languages are future work.
- Follow the architecture roadmap in [`docs/adr`](docs/adr) and the work queue in [`docs/tasks`](docs/tasks) for upcoming milestones.

## Workspace Layout

```
inkgen/
├── crates/
│   ├── inkgen-cli/           # Minimal CLI placeholder
│   ├── inkgen-core/          # Shared types and traits
│   ├── inkgen-typescript/    # TypeScript backend scaffolding
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

## Available `just` Commands

- `just bootstrap` — Ensure required Rust components (`rustfmt`, `clippy`) are installed.
- `just fetch PACKAGE=<name>` — Run the CLI fetch command (respects `--dry-run`/`--offline` via variables).
- `just generate lang=<backend> config=inkgen.toml` — Delegate to the CLI generator.
- `just test` — Run `cargo test --all`.
- `just snap` — Execute snapshot tests when `cargo-insta` is installed.
- `just review` — Run fmt, clippy (warnings as errors), and tests.
- `just clean` — Remove build artefacts with `cargo clean`.

Use `just --list` to see recipe parameters and defaults.

## CLI Placeholder

`inkgen-cli` drives the workspace automation pipeline:

- `inkgen config init` — create (or overwrite via `--force`) a starter `inkgen.toml`.
- `inkgen fetch [--package ...] [--offline] [--dry-run]` — download and cache FHIR packages declared in the manifest.
- `inkgen generate typescript [--output <dir>] [--dry-run]` — invoke the TypeScript generator (stub) after ensuring packages are available.
- `inkgen config validate` — verify manifest structure before running other commands.
- `inkgen config completions <shell> --output <path>` — emit shell completion scripts.

## Roadmap References

- Architecture decisions: [`docs/adr`](docs/adr)
- Task breakdown and progress logs: [`docs/tasks`](docs/tasks)

Contributions should reference the relevant ADRs and task files to keep context up to date.

## Contributing

Until the feature work in subsequent tasks lands, please coordinate changes via the task documents:

- Start from an open task in [`docs/tasks`](docs/tasks) and record progress notes there.
- Use `just bootstrap`, `just test`, and `just review` to validate changes locally.
- Keep documentation aligned with the current stubbed capabilities.

The CI workflow mirrors `just review`, so a passing run locally should translate to green builds.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

- **Issues**: Report bugs and request features via [GitHub Issues](https://github.com/octofhir/inkgen/issues)
- **Discussions**: Join conversations in [GitHub Discussions](https://github.com/octofhir/inkgen/discussions)
- **Documentation**: Additional documentation available in the `docs/` directory

## Roadmap

Active and upcoming milestones are tracked in [`docs/tasks`](docs/tasks); consult those files for the authoritative roadmap.
