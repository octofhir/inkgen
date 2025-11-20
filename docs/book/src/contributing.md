# Contributing

We welcome contributions to InkGen! This guide will help you get started.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR-USERNAME/inkgen.git`
3. Create a feature branch: `git checkout -b feature/my-feature`
4. Follow the [Development Setup](#development-setup)

## Development Setup

### Prerequisites

- Rust stable toolchain (edition 2024)
- [`just`](https://github.com/casey/just) command runner
- [`cargo-insta`](https://insta.rs/) for snapshot testing (optional)

### Install Dependencies

```bash
git clone https://github.com/octofhir/inkgen.git
cd inkgen
just bootstrap
```

### Build

```bash
cargo build --all
```

### Test

```bash
just test
```

Or run tests for a specific crate:

```bash
cargo test -p inkgen-typescript
```

### Lint and Format

```bash
just review
```

This runs formatting, clippy linting, and all tests.

## Making Changes

### Code Style

- Follow Rust conventions (use `rustfmt`)
- Use meaningful variable and function names
- Add documentation comments (`///`) to public APIs
- Keep functions focused and relatively short
- Add unit tests for new features

### Commit Messages

Use conventional commits format:

```
feat: add new feature
fix: fix bug
docs: update documentation
test: add tests
chore: update dependencies
```

### Pull Requests

1. Ensure `just review` passes locally
2. Push your branch to your fork
3. Create a PR against `main`
4. Describe your changes clearly
5. Fill out the PR template
6. Ensure all CI checks pass
7. Request review from maintainers

For detailed guidelines on adding language backends, see the main [CONTRIBUTING.md](../../../CONTRIBUTING.md) in the repository root.

## Reporting Issues

Use GitHub Issues to report bugs or suggest features:

1. Check existing issues first
2. Provide a minimal reproduction case
3. Include environment details
4. Attach relevant files

## Code of Conduct

Please be respectful and constructive in all interactions.

## Adding a New Language Backend

InkGen is designed to be extensible. To add a new language backend:

1. Create a new crate in `crates/inkgen-<language>`
2. Implement the `LanguageGenerator` trait from `inkgen-core`
3. Add configuration support
4. Integrate with the CLI
5. Write comprehensive tests

For detailed step-by-step instructions, see the [CONTRIBUTING.md](../../../CONTRIBUTING.md) in the repository root.

## Questions?

- Ask in [GitHub Discussions](https://github.com/octofhir/inkgen/discussions)
- Check the [Documentation](./README.md)
- Review [Architecture](./architecture/README.md)
- See the main [CONTRIBUTING.md](../../../CONTRIBUTING.md) for detailed guidelines

Thank you for contributing!
