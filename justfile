# Inkgen Development Commands
# Run `just --list` to see all available commands

# Install development dependencies and setup toolchain
bootstrap:
    rustup component add clippy rustfmt
    cargo fetch
    cargo build --bin inkgen

# Run all workspace tests
test:
    cargo test --all

# Run snapshot tests with insta
snap:
    cargo insta test

# Code quality review: format, lint, and test
review:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    cargo test --all

# Clean build artifacts
clean:
    cargo clean
    rm -rf target/

# Format code
fmt:
    cargo fmt --all

# Run clippy linter
lint:
    cargo clippy --all-targets -- -D warnings

# Build all crates
build:
    cargo build --all

# Build release version
build-release:
    cargo build --all --release

# Check code without building
check:
    cargo check --all

# Update snapshot tests
snap-review:
    cargo insta review

# Run tests with coverage (requires cargo-tarpaulin)
test-coverage:
    cargo tarpaulin --all --out Html

# Install development tools
install-tools:
    cargo install cargo-insta
    cargo install just
    cargo install cargo-tarpaulin

# CLI Commands
# ===========

# Fetch a FHIR package using the CLI
fetch package version="latest":
    cargo run --bin inkgen -- fetch {{package}} --version {{version}}

# Fetch a FHIR package with force re-download
fetch-force package version="latest":
    cargo run --bin inkgen -- fetch {{package}} --version {{version}} --force

# Generate TypeScript code from configuration
generate-ts config="inkgen.toml" output="generated":
    cargo run --bin inkgen -- generate typescript --config {{config}} --output {{output}}

# Generate TypeScript code from a specific package
generate-ts-package package config="inkgen.toml" output="generated":
    cargo run --bin inkgen -- generate typescript --config {{config}} --output {{output}} --package {{package}}

# Initialize a new configuration file
config-init output="inkgen.toml":
    cargo run --bin inkgen -- config init --output {{output}}

# Initialize configuration with force overwrite
config-init-force output="inkgen.toml":
    cargo run --bin inkgen -- config init --output {{output}} --force

# CLI Testing Commands
# ===================

# Run CLI-specific tests
test-cli:
    cargo test --package inkgen-cli

# Run CLI integration tests
test-cli-integration:
    cargo test --package inkgen-cli --test integration_tests

# Run CLI tests with verbose output
test-cli-verbose:
    cargo test --package inkgen-cli -- --nocapture

# Test CLI help output
test-cli-help:
    cargo run --bin inkgen -- --help

# Test CLI version output
test-cli-version:
    cargo run --bin inkgen -- --version

# Common CLI Workflows
# ===================

# Quick start: fetch R4 core and generate TypeScript
quickstart:
    cargo run --bin inkgen -- config init
    cargo run --bin inkgen -- fetch hl7.fhir.r4.core --version 4.0.1
    cargo run --bin inkgen -- generate typescript

# Development workflow: fetch US Core and generate with custom config
dev-workflow:
    cargo run --bin inkgen -- fetch hl7.fhir.us.core --version 6.1.0
    cargo run --bin inkgen -- generate typescript --config dev-config.toml --output dev-generated