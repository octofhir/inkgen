# Task: CLI Scaffolding

## Summary
Build the `inkgen-cli` crate with user-facing commands that orchestrate package fetching, configuration management, and generator execution stubs.

## Background
Phase 2 of the coding agent plan establishes a contributor-friendly CLI wrapping core services. Although language backends are still under development, the CLI must feel production-ready so early adopters can script workflows and provide feedback.

## Status
- [x] Command surface defined (`fetch`, `generate`, `config init`) with logging flags and dry-run/offline toggles.
- [x] Manifest loading via `InkgenConfig`, including directory resolution and tree-shaking bridge.
- [x] Integration with `PackageResolver`, `PackageCache`, and `BaseStructureService` for fetch/generate paths.
- [x] Developer tooling updated (`justfile` recipes, README to follow).
- [x] CLI integration tests (assert_cmd) for config init, fetch, and generate dry runs.
- [x] Shell completions and initial manifest validation helpers.

## Context Snapshot
- CLI ergonomics should align with modern Rust CLIs (`clap`, `color-eyre`/`anyhow`, structured logging via `tracing`).
- `inkgen.toml` will become the primary manifest for projects; schema stability is important even if some values are not yet consumed.
- CLI commands must integrate with the `PackageResolver` and `StructureDefinitionProvider` created in TASK-0002 to avoid duplicate logic.

## Detailed Work Breakdown
1. **Command Surface Definition**
   - Design subcommands: `fetch`, `generate`, `config`.
   - Choose a command layout (e.g., `inkgen generate typescript`, `inkgen generate --lang typescript`); document reasoning in CLI help text.
   - Provide global options for logging (`--log-level`, `--json-logs`) and cache directory overrides.
2. **Configuration Handling**
   - Define `inkgen_config` module encapsulating manifest schema using `serde` + `figment` or `toml` crate.
     - Support fields:
       - `packages`: list of `{ name, version, registry? }`.
       - `tree_shaking`: toggles for `allowed_resources`, `allowed_profiles`, and future exclusions.
       - `languages.typescript`: structure with defaults for mode, output_dir, naming rules, structural guard flags.
   - Implement schema validation with helpful error messages (use `schemars` if JSON schema export is desired).
   - Enforce that generation always operates on packages declared in config; prevent fallback to implicit defaults.
3. **Subcommand Implementations**
   - `inkgen fetch`: iterate configured (and optionally filtered) packages, call `PackageResolver::ensure_packages`, report status, and summarize results. Support `--offline`/`--dry-run`.
   - `inkgen generate typescript`: load manifest, resolve packages, construct the core `StructureDefinitionProvider`, and call the TypeScript stub generator while honouring dry-run/output overrides.
   - Block generation if configured packages were not fetched or resolved successfully; instruct users to run `inkgen fetch`.
   - `inkgen config init`: generate sample `inkgen.toml` using curated defaults; optionally prompt before overwriting existing files.
4. **Error Handling & Logging**
   - Adopt `color-eyre` or custom error layer with `anyhow::Result`.
   - Integrate `tracing` subscriber with environment-based log level (default to `info`).
   - Ensure errors include remediation tips (e.g., "Run `inkgen fetch` before generating").
5. **Integration Tests**
   - Use `assert_cmd`, `tempfile`, and `predicates` to test CLI commands end-to-end.
   - Cover scenarios: successful fetch, missing manifest, invalid manifest schema, dry-run generation, config init overwriting.
   - Add golden output tests using `insta` or `trycmd` for help text to catch regressions.
6. **Developer Tooling Updates**
   - Update `justfile` to leverage the new CLI (`just fetch`, `just generate`).
   - Document CLI usage examples in README (copy/paste friendly).
   - Add shell completion generation command if time permits (`inkgen completions <shell>`).
7. **Validation**
   - Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all`.
   - Manually invoke CLI commands with sample manifests to confirm UX (capture output snippets for docs).

## Acceptance Criteria
- Running each CLI command locally succeeds with clear output and no panics. *(pass)*
- Integration tests run under `cargo test -p inkgen-cli` and pass without warnings. *(pass)*
- Configuration parsing rejects invalid manifests with actionable diagnostics referencing the failing field.
- CLI logs respect `RUST_LOG` or explicit flags and default to concise info-level output. *(pass)*
- Workspace `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` remain green. *(pass)*
- CLI refuses to generate when packages outside the manifest are requested, reinforcing config-driven workflows.

## Scope Boundaries
- The TypeScript generator can still emit stub output; full generation is handled in TASK-0004.
- No GUI or interactive prompts beyond confirmation for overwriting files.
- No network access beyond `PackageResolver` (which already implements caching).

## Dependencies
- `TASK-0001 Workspace Bootstrap`
- `TASK-0002 Core Engine Foundations`

## Follow-up Notes
- Ensure CLI telemetry or analytics hooks are considered later (if at all); document decision in ADR if needed.
- Capture open questions about manifest versioning for discussion before language plugins ship.

## Progress Log
- 2025-11-06 — Implemented structured CLI with `fetch`, `generate typescript`, and `config init` commands powered by `InkgenConfig`, `PackageResolver`, and `BaseStructureService`.
- 2025-11-06 — Added dry-run/offline toggles, package filtering, and updated `justfile` recipes to delegate to the new CLI.
- 2025-11-06 — Added `assert_cmd` integration tests covering config init, fetch, and generate workflows (dry-run).
- 2025-11-06 — Implemented manifest validation and shell completion generation (`config validate`, `config completions`).
