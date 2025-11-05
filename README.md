# InkGen

![logo](./inkgen-logo.png)

A Rust-based FHIR code generator that transforms canonical FHIR packages into SDKs for multiple programming languages, starting with TypeScript.

## Overview

InkGen is designed to bridge the gap between FHIR specifications and practical SDK development. It processes canonical FHIR packages and generates type-safe, idiomatic code for target programming languages, enabling developers to work with FHIR resources in a natural way within their preferred development environment.

### Key Features

- **Multi-language Support**: Generate SDKs for TypeScript (with more languages planned)
- **Type Safety**: Generate strongly-typed interfaces from FHIR profiles
- **Canonical Package Integration**: Direct integration with FHIR canonical packages
- **Extensible Architecture**: Plugin-based backend system for adding new target languages
- **Developer-Friendly**: Comprehensive CLI with intuitive commands

## Architecture

InkGen follows a modular workspace architecture with clear separation of concerns:

```
inkgen/
├── crates/
│   ├── inkgen-cli/           # Command line interface
│   ├── inkgen-core/          # FHIR processing engine
│   ├── inkgen-typescript/    # TypeScript code generator
│   └── inkgen-testing/       # Shared testing utilities
├── justfile                  # Development automation
└── .github/workflows/        # CI/CD configuration
```

### Component Overview

- **inkgen-cli**: User-facing command line tool with subcommands for fetching, generating, and configuring
- **inkgen-core**: Central engine handling FHIR resource parsing, validation, and intermediate representation
- **inkgen-typescript**: TypeScript-specific code generation backend with templating support
- **inkgen-testing**: Shared testing infrastructure including snapshot testing and fixture management

## Prerequisites

### Required Tools

- **Rust**: Version 1.70 or later (2024 edition support)
- **just**: Command runner for development automation
  ```bash
  # Install via cargo
  cargo install just
  
  # Or via package manager (macOS)
  brew install just
  ```

### Optional Development Tools

- **cargo-insta**: For snapshot testing
- **cargo-tarpaulin**: For test coverage reports

You can install these tools automatically by running:
```bash
just install-tools
```

## Installation

### From Source

1. Clone the repository:
   ```bash
   git clone https://github.com/octofhir/inkgen.git
   cd inkgen
   ```

2. Bootstrap the development environment:
   ```bash
   just bootstrap
   ```

3. Build the project:
   ```bash
   just build
   ```

4. Run tests to verify installation:
   ```bash
   just test
   ```

## Usage

### Basic Commands

```bash
# Generate TypeScript SDK from a FHIR package
inkgen generate --input fhir-package.tgz --output ./generated --language typescript

# Fetch a canonical FHIR package
inkgen fetch --package hl7.fhir.r4.core --version 4.0.1

# Configure default settings
inkgen config --set default-language typescript
```

### Development Workflow

The project uses `just` for development automation. Here are all available commands:

```bash
# Initial setup (run once)
just bootstrap     # Install development dependencies and setup toolchain

# Development cycle
just test          # Run all workspace tests
just fmt           # Format code
just lint          # Run clippy linter
just review        # Complete quality check (format + lint + test)

# Snapshot testing
just snap          # Run snapshot tests with insta
just snap-review   # Review and update snapshot tests

# Build commands
just build         # Build all crates (debug)
just build-release # Build all crates (release)
just check         # Check code without building

# Testing and coverage
just test-coverage # Run tests with coverage (requires cargo-tarpaulin)

# Development tools
just install-tools # Install additional development tools (insta, tarpaulin)

# Maintenance
just clean         # Clean build artifacts

# List all commands
just --list        # Show all available commands
```

### Running Tests

```bash
# Run all tests
just test

# Run tests with coverage
just test-coverage

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'
```

## Contributing

We welcome contributions! Please follow these guidelines to ensure a smooth development experience.

### Development Setup

1. **Fork and Clone**: Fork the repository and clone your fork locally
   ```bash
   git clone https://github.com/YOUR_USERNAME/inkgen.git
   cd inkgen
   ```

2. **Environment Setup**: Set up your development environment
   ```bash
   just bootstrap          # Install Rust components and fetch dependencies
   just install-tools      # Install optional development tools
   ```

3. **Verify Setup**: Ensure everything works correctly
   ```bash
   just review            # Run complete quality check
   ```

4. **Create Branch**: Create a feature branch for your work
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Development Workflow

1. **Make Changes**: Implement your feature or fix
2. **Test Locally**: Run tests frequently during development
   ```bash
   just test              # Run all tests
   just snap              # Run snapshot tests if applicable
   ```
3. **Code Quality**: Ensure code meets standards
   ```bash
   just fmt               # Format code
   just lint              # Check for linting issues
   ```
4. **Complete Review**: Before committing, run full review
   ```bash
   just review            # Format + lint + test
   ```

### Coding Standards

- **Formatting**: Use `cargo fmt` - enforced in CI
- **Linting**: Address all `cargo clippy` warnings - enforced in CI  
- **Testing**: Add comprehensive tests for new functionality
- **Documentation**: Document public APIs with rustdoc comments
- **Commit Messages**: Use clear, descriptive commit messages
- **Code Style**: Follow Rust community conventions

### Testing Guidelines

- **Unit Tests**: Add tests in `src/` modules using `#[cfg(test)]`
- **Integration Tests**: Add end-to-end tests in `tests/` directories
- **Snapshot Tests**: Use `cargo insta` for deterministic output validation
- **Test Coverage**: Aim for comprehensive coverage of new functionality

### Pull Request Process

1. **Pre-submission Checklist**:
   ```bash
   just review            # Ensure all quality checks pass
   ```

2. **Update Documentation**: Update README or docs if needed

3. **Test Coverage**: Ensure new code has appropriate tests

4. **Submit PR**: Create pull request with:
   - Clear title and description
   - Reference to related issues
   - Summary of changes made
   - Testing approach used

5. **Address Feedback**: Respond to review comments promptly

### Code Quality Checks

The `just review` command runs our complete quality pipeline:

- **Format Check**: `cargo fmt --all --check`
- **Linting**: `cargo clippy --all-targets -- -D warnings`  
- **Testing**: `cargo test --all`

All checks must pass for CI to succeed.

## Project Structure

### Workspace Organization

- **Binary Crate**: Only `inkgen-cli` is publishable and provides the user interface
- **Library Crates**: Internal workspace dependencies for modular development
- **Testing**: Shared testing utilities and snapshot testing infrastructure

### Key Files

- `Cargo.toml`: Workspace configuration and dependency management
- `justfile`: Development automation commands
- `.github/workflows/ci.yml`: Continuous integration configuration
- `crates/*/Cargo.toml`: Individual crate configurations

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

- **Issues**: Report bugs and request features via [GitHub Issues](https://github.com/octofhir/inkgen/issues)
- **Discussions**: Join conversations in [GitHub Discussions](https://github.com/octofhir/inkgen/discussions)
- **Documentation**: Additional documentation available in the `docs/` directory

## Roadmap

- [ ] TypeScript SDK generation (Phase 1)
- [ ] Python backend support
- [ ] JavaScript/Node.js backend support
- [ ] Advanced FHIR profile validation
- [ ] Plugin system for custom backends