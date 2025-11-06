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

clean:
    @cargo clean
