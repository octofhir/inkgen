# Inkgen Development Commands
# Run `just --list` to see all available commands

# Install development dependencies and setup toolchain
bootstrap:
    rustup component add clippy rustfmt
    cargo fetch

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