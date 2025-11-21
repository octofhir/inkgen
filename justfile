set dotenv-load := false
set positional-arguments := true

default:
    @just --list

bootstrap:
    @echo "Installing Rust toolchain components..."
    @rustup show >/dev/null
    @rustup component add rustfmt clippy
    @echo "Bootstrap complete."

fetch PACKAGE='' config='inkgen.toml' offline='false' dry_run='false':
    @cmd="cargo run -p inkgen-cli -- fetch --config {{config}}"
    @if [ -n "{{PACKAGE}}" ]; then \
        cmd="$cmd --package {{PACKAGE}}"; \
    fi; \
    if [ "{{offline}}" = "true" ]; then \
        cmd="$cmd --offline"; \
    fi; \
    if [ "{{dry_run}}" = "true" ]; then \
        cmd="$cmd --dry-run"; \
    fi; \
    echo "$cmd"; \
    eval "$cmd"

generate lang='typescript' config='inkgen.toml' offline='false' dry_run='false' output='' package='':
    #!/usr/bin/env bash
    set -euo pipefail
    cmd="cargo run -p inkgen-cli -- generate {{lang}}"
    if [ -n "{{config}}" ]; then
        cmd="$cmd --config {{config}}"
    fi
    if [ "{{offline}}" = "true" ]; then
        cmd="$cmd --offline"
    fi
    if [ "{{dry_run}}" = "true" ]; then
        cmd="$cmd --dry-run"
    fi
    if [ -n "{{output}}" ]; then
        cmd="$cmd --output {{output}}"
    fi
    if [ -n "{{package}}" ]; then
        cmd="$cmd --package {{package}}"
    fi
    echo "$cmd"
    eval "$cmd"

test:
    @cargo test --all

snap:
    @if command -v cargo-insta >/dev/null 2>&1; then \
        cargo insta test; \
    else \
        echo "cargo-insta not installed; skipping snapshot tests"; \
    fi

review:
    @cargo fmt --all --check
    @cargo clippy --all-targets --all-features -- -D warnings
    @cargo test --all

bench:
    @echo "Running performance benchmarks..."
    @cargo bench --bench codegen
    @echo ""
    @echo "Benchmark results available above. Key metrics:"
    @echo "  - IR Construction: Time to parse FHIR structure definitions"
    @echo "  - Profile Resolution: Time to resolve inheritance chains"
    @echo "  - Template Rendering: Time to render Tera templates"
    @echo "  - Code Generation: End-to-end generation time"
    @echo ""
    @echo "For regression detection, compare against baseline:"
    @echo "  cargo bench --bench codegen -- --baseline main"

# Release automation commands
release-dry-run:
    @echo "Running release dry-run..."
    @cargo install cargo-release --locked 2>/dev/null || true
    @cargo release --workspace --dry-run --no-tag --no-push

release-check:
    @echo "Checking release readiness..."
    @echo "1. Validating Rust code quality..."
    @just review
    @echo "2. Checking workspace integrity..."
    @cargo check --all
    @echo "3. Verifying publish configuration..."
    @cargo metadata --format-version 1 | jq '.packages[] | select(.name | startswith("inkgen")) | {name, publish}'
    @echo "✓ Release check complete"

docs-build:
    @echo "Building documentation site..."
    @if command -v mdbook >/dev/null 2>&1; then \
        cd docs/book && mdbook build; \
        echo "✓ Documentation built successfully at docs/book/build/"; \
    else \
        echo "Error: mdbook not installed."; \
        echo "Install with: cargo install mdbook"; \
        exit 1; \
    fi

docs-serve port='3000':
    @echo "Serving documentation locally on port {{port}}..."
    @if command -v mdbook >/dev/null 2>&1; then \
        cd docs/book && mdbook serve --port {{port}} --open; \
    else \
        echo "Error: mdbook not installed."; \
        echo "Install with: cargo install mdbook"; \
        exit 1; \
    fi

# Development commands
fmt:
    @echo "Formatting code..."
    @cargo fmt --all

lint:
    @echo "Running clippy..."
    @cargo clippy --all-targets --all-features -- -D warnings

watch:
    @echo "Watching for changes and running tests..."
    @if command -v cargo-watch >/dev/null 2>&1; then \
        cargo watch -x "test --all"; \
    else \
        echo "Error: cargo-watch not installed."; \
        echo "Install with: cargo install cargo-watch"; \
        exit 1; \
    fi

dev:
    @echo "Running development checks..."
    @just fmt
    @just lint
    @just test

# Utility commands
clean:
    @cargo clean
