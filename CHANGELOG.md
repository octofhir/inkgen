## [0.1.5] - 2026-11-04

- Update the private CLI diff engine to Similar 3; keep library dependency profiles and generated output unchanged.

## [0.1.4] - 2026-11-04

### Changed
- Add a core IR mode without canonical-manager, SQLite, HTTP, Tokio, Tera or the TOML parser (`default-features = false`). Default core features retain the resolver, template and TOML configuration APIs.
- Rust and Gleam backends consume the lightweight IR core and drop unused async/error dependencies.
- Add an optional resolver feature to TypeScript: the default preserves the existing cache configuration; disable defaults and use `TypescriptGeneratorConfig::from_package_ir` for generation from resolved IR without network/storage dependencies.
- Remove unused futures and once_cell dependencies, trim Tokio features, disable the unused canonical-manager CLI, and update compatible dependencies and private dirs to 7.

## [v0.1.3] - 2025-11-21

### Changes

- fix: cli name in tests
- ci: add missed binaries

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
