# Task: Workspace Bootstrap

## Summary
Establish the initial Rust workspace structure, shared tooling, and contributor documentation so future tasks can focus on feature delivery without reworking project scaffolding.

## Status
- [x] Workspace skeleton files
- [x] Crate bootstrapping
- [x] Repository hygiene updates
- [x] Developer tooling (`justfile`)
- [x] CI skeleton
- [x] Documentation pass
- [x] Local validation *(`just bootstrap`, `just test`, `just review`)*

## Progress Log
- 2025-11-06 — Added workspace members and placeholder crates (`cli`, `core`, `typescript`, `testing`).
- 2025-11-06 — Replaced legacy source files with fresh minimal stubs; removed legacy configs/tests to honour "from scratch" mandate.
- 2025-11-06 — Authored baseline `justfile`, CI workflow, README status note, and refreshed `.gitignore`.
- 2025-11-06 — Attempted `cargo check --all`; blocked due to restricted network preventing crates.io index resolution.
- 2025-11-06 — Enabled `indexmap` serde feature and completed `just bootstrap`, `just test`, `just review` locally.
- 2025-11-06 — Aligned README quick start and command reference with the stubbed workspace capabilities.

## Background
ADR 0001 and the coding agent plan expect a multi-crate workspace with strong developer ergonomics (`justfile`, CI hooks, baseline docs). The repo currently lacks those guardrails, so contributors have no reliable target for `cargo` commands or CI validation. This task provides that baseline.

## Context Snapshot
- Toolchain must use Rust stable (matching `rustup show` output in ADR assumptions); nightly features are off-limits.
- Workspace members will share lint/test configuration to guarantee consistent warnings-as-errors enforcement.
- The CI skeleton should mirror the `just review` command so local and remote checks stay aligned.

## Detailed Work Breakdown
1. **Workspace Skeleton**
   - Author root `Cargo.toml` with `[workspace]` members for `crates/inkgen-cli`, `crates/inkgen-core`, `crates/inkgen-typescript`, `crates/inkgen-testing`.
   - Include `[workspace.package]` metadata (version, authors, edition) and `[workspace.dependencies]` entries for shared crates (`anyhow`, `tracing`, etc.) to avoid duplication later.
   - Generate `Cargo.lock` using `cargo generate-lockfile`.
   - Pin the toolchain via `rust-toolchain.toml` (e.g., `channel = "stable"`).
2. **Crate Bootstrapping**
   - Create directories and minimal `Cargo.toml` for each crate, wiring dependencies only as needed to compile with zero warnings.
   - Implement placeholder `main.rs`/`lib.rs` that compile cleanly (e.g., `fn main() -> Result<(), anyhow::Error> { println!("inkgen-cli bootstrap"); Ok(()) }`).
   - Add crate-level documentation comments referencing ADR 0001 to keep the context visible.
3. **Repository Hygiene**
   - Add `.gitignore` covering `/target`, `/dist`, `.cargo/`, `*.snap.new`, and other build artifacts.
   - Configure `rustfmt.toml` if project-specific formatting decisions are required (otherwise document that defaults apply).
4. **Developer Tooling**
   - Author `justfile` with recipes: `bootstrap`, `fetch`, `generate`, `test`, `snap`, `review`, `clean`.
   - Ensure commands echo intent and fail fast (e.g., `set -euo pipefail`).
   - Document environment variables (e.g., `INKGEN_CACHE_DIR`) that upcoming tasks will rely on, even if stubs for now.
5. **CI Skeleton**
   - Add `.github/workflows/ci.yml` (or chosen platform) with jobs for `fmt`, `clippy`, `test`.
   - Cache `~/.cargo` and `target` directories for performance; document cache strategy in workflow comments.
   - Ensure workflow references `just review` to stay in sync with local commands.
6. **Documentation Pass**
   - Update `README.md` with Quick Start (clone, `just bootstrap`, `just test`), architecture overview referencing ADR 0001, and contribution expectations.
   - Link to `docs/adr/0001-inkgen-codegen-strategy.md` and `docs/tasks` from README.
   - If CONTRIBUTING.md is missing, add a stub directing readers to README and ADR for now.
7. **Validation**
   - Run `just bootstrap`, `just test`, `just review` locally; capture any prerequisites (install `just`, `rustup`) in README.
   - Optionally run the CI workflow via `act` or note manual verification steps.

## Acceptance Criteria
- `just bootstrap`, `just test`, and `just review` succeed on a clean checkout with no warnings.
- CI workflow definition mirrors local commands and is linted (GitHub Actions: `act -l` or manual review).
- README clearly documents prerequisites, quick start, and where to find architectural context.
- Workspace builds with zero `cargo` warnings; placeholder code avoids `todo!()` or unused warnings.
- All new files follow ASCII-only encoding and include SPDX/license headers if the repo policy requires them.

## Scope Boundaries
- No real CLI logic, IR definitions, or package fetching; those belong to later tasks.
- No release automation or doc site generation (handed off to TASK-0007).
- No network-dependent tests should run in CI at this stage.

## Dependencies
- None.

## Follow-up Notes
- Subsequent tasks populate real functionality. Keep stubs simple but production-friendly (return `Result<(), anyhow::Error>`).
- Document any deviations or open questions (e.g., alternative CI provider) for future ADRs.
