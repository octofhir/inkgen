set dotenv-load := false
set positional-arguments := true

default:
    @just --list

bootstrap:
    @echo "Installing Rust toolchain components..."
    @rustup show >/dev/null
    @rustup component add rustfmt clippy
    @echo "Bootstrap complete."

fetch PACKAGE='':
    @if [ -z "{{PACKAGE}}" ]; then \
        echo "Usage: just fetch PACKAGE=<package>"; \
        exit 1; \
    fi
    @echo "Fetching package {{PACKAGE}} (stub command; real implementation arrives in TASK-0003)"

generate lang='typescript' config='inkgen.toml':
    @echo "Generating SDK for language {{lang}} using config {{config}}"
    @echo "Stub generation command executed (real implementation arrives in TASK-0004)"

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
