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

generate lang='typescript' config='inkgen.toml' offline='false' dry_run='false' output='' PACKAGE='':
    @cmd="cargo run -p inkgen-cli -- generate {{lang}} --config {{config}}"
    @if [ "{{offline}}" = "true" ]; then \
        cmd="$cmd --offline"; \
    fi; \
    if [ "{{dry_run}}" = "true" ]; then \
        cmd="$cmd --dry-run"; \
    fi; \
    if [ -n "{{output}}" ]; then \
        cmd="$cmd --output {{output}}"; \
    fi; \
    if [ -n "{{PACKAGE}}" ]; then \
        cmd="$cmd --package {{PACKAGE}}"; \
    fi; \
    echo "$cmd"; \
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
    @cargo clippy --all-targets -- -D warnings
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
    @echo "Note: Documentation site setup (mdBook) is planned"
    @echo "See: docs/tasks/TASK-0007-hardening-and-release-prep.md"

docs-serve:
    @echo "Serving documentation locally..."
    @echo "Note: Documentation site setup (mdBook) is planned"
    @echo "See: docs/tasks/TASK-0007-hardening-and-release-prep.md"

# Utility commands
clean:
    @cargo clean
