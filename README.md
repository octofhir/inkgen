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

### CLI Commands

InkGen provides a comprehensive command-line interface for FHIR package management and code generation.

#### Fetch FHIR Packages

```bash
# Fetch the latest version of a package
inkgen fetch hl7.fhir.r4.core

# Fetch a specific version
inkgen fetch hl7.fhir.r4.core --version 4.0.1

# Force re-download even if cached
inkgen fetch hl7.fhir.us.core --version 6.1.0 --force

# Shortened package names are supported
inkgen fetch r4.core --version 4.0.1  # Expands to hl7.fhir.r4.core
```

#### Generate TypeScript Code

```bash
# Generate using default configuration (inkgen.toml)
inkgen generate typescript

# Specify custom configuration and output directory
inkgen generate typescript --config my-config.toml --output ./sdk

# Generate from a specific package override
inkgen generate typescript --package hl7.fhir.r4.core --output ./r4-sdk
```

#### Configuration Management

```bash
# Create a new configuration file with defaults
inkgen config init

# Create configuration at a specific path
inkgen config init --output custom-config.toml

# Overwrite existing configuration
inkgen config init --force
```

#### Global Options

```bash
# Enable verbose logging
inkgen --verbose fetch r4.core

# Set custom log level
inkgen --log-level debug generate typescript

# Get help for any command
inkgen --help
inkgen fetch --help
inkgen generate typescript --help
```

### Configuration File Format

InkGen uses TOML configuration files to specify packages, generation options, and language-specific settings.

#### Basic Configuration Structure

```toml
# inkgen.toml - InkGen Configuration File

# List of FHIR packages to include in generation
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[[packages]]
name = "hl7.fhir.us.core"
version = "6.1.0"
# Package-specific inclusion rules
include = ["Patient", "Observation"]
# Package-specific exclusion rules  
exclude = ["Bundle", "OperationOutcome"]

# Tree-shaking configuration to control what gets generated
# By default, all resources from packages are generated
# Use tree-shaking to limit generation to specific resources
[tree_shaking]
# Explicit allowlist of resources to include (optional)
allowed_resources = ["Patient", "Observation", "Practitioner"]
# Explicit allowlist of profiles to include (optional)
allowed_profiles = ["us-core-patient", "us-core-observation"]

# Language-specific configuration
[languages.typescript]
# Generation mode: "interface" | "class" | "class_with_builder"
mode = "class_with_builder"
# Enable structural type guards
structural_guards = true
# Naming convention: "PascalCase" | "camelCase" | "snake_case"
naming_convention = "PascalCase"
# Output structure: "flat" | "nested" | "by_package"
output_structure = "nested"
```

#### Configuration Options Reference

**Package Specification**:
- `name`: Package identifier (supports shortened names like `r4.core`)
- `version`: Specific version or "latest" (optional)
- `include`: Array of resource/profile names to include (optional)
- `exclude`: Array of resource/profile names to exclude (optional)

**Tree-shaking Options** (all optional - by default all resources are generated):
- `allowed_resources`: Explicit resource type allowlist to limit generation
- `allowed_profiles`: Explicit profile allowlist to limit generation

**TypeScript Options**:
- `mode`: Code generation style
  - `"interface"`: TypeScript interfaces only
  - `"class"`: Classes with methods
  - `"class_with_builder"`: Classes with builder pattern
- `structural_guards`: Generate type guard functions
- `naming_convention`: Identifier naming style
- `output_structure`: File organization strategy

### Getting Started Guide

#### Quick Start (5 minutes)

1. **Install InkGen** (after building from source):
   ```bash
   git clone https://github.com/octofhir/inkgen.git
   cd inkgen
   just bootstrap
   ```

2. **Initialize Configuration**:
   ```bash
   # Create default configuration
   just config-init
   
   # Or use the CLI directly
   cargo run --bin inkgen -- config init
   ```

3. **Fetch FHIR Packages**:
   ```bash
   # Fetch R4 core package
   just fetch hl7.fhir.r4.core 4.0.1
   
   # Or use CLI directly
   cargo run --bin inkgen -- fetch hl7.fhir.r4.core --version 4.0.1
   ```

4. **Generate TypeScript SDK**:
   ```bash
   # Generate using configuration
   just generate-ts
   
   # Or use CLI directly
   cargo run --bin inkgen -- generate typescript
   ```

5. **Use Generated Code**:
   ```typescript
   import { Patient, Observation } from './generated';
   
   const patient = new Patient()
     .setId('patient-123')
     .setActive(true)
     .addName({
       family: 'Doe',
       given: ['John']
     });
   ```

#### Common Workflows

**US Core Development**:
```bash
# 1. Initialize project
just config-init

# 2. Fetch US Core package
just fetch hl7.fhir.us.core 6.1.0

# 3. Generate TypeScript with custom output
just generate-ts-package hl7.fhir.us.core inkgen.toml us-core-sdk

# 4. Use in your application
# Import from ./us-core-sdk/
```

**Multi-Package Project**:
```toml
# inkgen.toml - Generate all resources from multiple packages
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[[packages]]
name = "hl7.fhir.us.core"  
version = "6.1.0"

[[packages]]
name = "hl7.fhir.us.mcode"
version = "3.0.0"

# Optional: limit to specific resources only
[tree_shaking]
allowed_resources = ["Patient", "Observation", "Condition"]
```

**Custom Configuration Example**:
```toml
# custom-config.toml
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"
include = ["Patient", "Observation", "Practitioner", "Organization"]

# Generate only specific resources across all packages
[tree_shaking]
allowed_resources = ["Patient", "Observation", "Practitioner", "Organization"]

[languages.typescript]
mode = "class_with_builder"
structural_guards = true
naming_convention = "PascalCase"
output_structure = "by_package"
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

#### CLI Development Commands

```bash
# CLI-specific testing
just test-cli              # Run CLI-specific tests
just test-cli-integration  # Run CLI integration tests
just test-cli-verbose      # Run CLI tests with verbose output

# CLI functionality testing
just test-cli-help         # Test CLI help output
just test-cli-version      # Test CLI version output

# CLI workflow commands
just fetch <package> [version]           # Fetch FHIR package
just fetch-force <package> [version]     # Force re-fetch package
just generate-ts [config] [output]       # Generate TypeScript code
just generate-ts-package <package> [config] [output]  # Generate from specific package
just config-init [output]               # Initialize configuration
just config-init-force [output]         # Force initialize configuration

# Common workflows
just quickstart                         # Complete quick start workflow
just dev-workflow                       # Development workflow example
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

## Troubleshooting

### Common CLI Issues

#### Package Fetch Problems

**Issue**: `Failed to fetch package 'hl7.fhir.r4.core'`
```bash
# Solutions:
# 1. Check internet connection
# 2. Verify package name and version
just fetch hl7.fhir.r4.core 4.0.1

# 3. Try force re-download
just fetch-force hl7.fhir.r4.core 4.0.1

# 4. Check with verbose logging
cargo run --bin inkgen -- --verbose fetch hl7.fhir.r4.core --version 4.0.1
```

**Issue**: `Package version not found`
```bash
# Check available versions (manual verification needed)
# Use "latest" or omit version for most recent
just fetch hl7.fhir.r4.core latest
```

#### Configuration Issues

**Issue**: `Configuration file already exists`
```bash
# Use force flag to overwrite
just config-init-force

# Or specify different output path
just config-init my-custom-config.toml
```

**Issue**: `Invalid configuration: missing required field`
```bash
# Regenerate default configuration
just config-init-force

# Validate configuration format - ensure TOML syntax is correct
# Check that all required fields are present:
# - At least one [[packages]] entry with name field
```

**Issue**: `Configuration parsing failed`
```bash
# Common TOML syntax issues:
# 1. Missing quotes around strings
# 2. Incorrect array syntax
# 3. Invalid section headers

# Example of correct syntax:
[[packages]]
name = "hl7.fhir.r4.core"  # Quotes required
version = "4.0.1"          # Quotes required

[tree_shaking]
allowed_resources = ["Patient", "Observation"]  # Array with quotes
```

#### Code Generation Issues

**Issue**: `Code generation failed: no packages found`
```bash
# Ensure packages are fetched first
just fetch hl7.fhir.r4.core 4.0.1

# Check configuration has valid package entries
cat inkgen.toml

# Verify package cache
ls -la target/inkgen/packages/
```

**Issue**: `Output directory permission denied`
```bash
# Check directory permissions
ls -la ./generated/

# Create directory manually if needed
mkdir -p ./generated
chmod 755 ./generated

# Use different output directory
just generate-ts inkgen.toml ./my-output
```

**Issue**: `Generated code has compilation errors`
```bash
# Check TypeScript configuration in inkgen.toml
# Ensure mode is compatible with your use case:
[languages.typescript]
mode = "interface"  # Try simpler mode first

# Regenerate with verbose logging
cargo run --bin inkgen -- --verbose generate typescript
```

#### Build and Development Issues

**Issue**: `cargo build fails`
```bash
# Update Rust toolchain
rustup update

# Clean and rebuild
just clean
just bootstrap
just build
```

**Issue**: `Tests failing`
```bash
# Run specific test suites
just test-cli              # CLI tests only
just test                  # All tests

# Update snapshots if needed
just snap-review

# Check for dependency issues
cargo tree
```

#### Performance Issues

**Issue**: `Package fetch is very slow`
```bash
# Check network connection
# Large packages (like US Core) can take time

# Use verbose logging to see progress
cargo run --bin inkgen -- --verbose fetch hl7.fhir.us.core
```

**Issue**: `Code generation takes too long`
```bash
# Use tree-shaking to reduce scope
[tree_shaking]
allowed_resources = ["Patient", "Observation"]  # Limit resources

# Check package size
ls -lh target/inkgen/packages/
```

### Debug Mode

Enable debug logging for detailed troubleshooting:

```bash
# Set log level via CLI
cargo run --bin inkgen -- --log-level debug <command>

# Set via environment variable
INKGEN_LOG=debug cargo run --bin inkgen -- <command>

# Enable verbose mode
cargo run --bin inkgen -- --verbose <command>
```

### Getting Help

If you encounter issues not covered here:

1. **Check existing issues**: [GitHub Issues](https://github.com/octofhir/inkgen/issues)
2. **Enable debug logging**: Use `--verbose` or `--log-level debug`
3. **Provide context**: Include configuration file, command used, and full error output
4. **Create minimal reproduction**: Use `just quickstart` to test basic functionality

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