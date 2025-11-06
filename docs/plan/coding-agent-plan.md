# Inkgen Coding Agent Plan

> Derived from ADR 0001: Inkgen FHIR Code Generation Strategy (2025) and focused on guiding day-to-day implementation work.

## 1. Guiding Principles

- Build everything in Rust; only the `inkgen` CLI will be published.
- Keep the architecture extensible (trait-based core + language backends) while accepting that all plugins live in-tree for now.
- Treat StructureDefinition profiles (including CCDA packages) as first-class citizens throughout the pipeline.
- Make snapshot-driven (`insta`) testing and reproducible outputs mandatory for any generator feature.
- Prioritize contributor ergonomics: clear module boundaries, descriptive logging (`tracing`), and documented workflows (`justfile`).

## 2. Phase Breakdown & Deliverables

### Phase 0 — Repository Bootstrap (Week 0-1)
- Create Rust workspace layout:
  - `crates/inkgen-cli`
  - `crates/inkgen-core`
  - `crates/inkgen-typescript`
  - `crates/inkgen-testing`
- Add baseline CI placeholders (format, lint, test) and a `justfile` skeleton with `bootstrap`, `test`, `snap`.
- Document prerequisites in `README.md` (Rust version, `just`, optional `cargo install` commands).

### Phase 1 — Core Engine Foundations (Week 1-3)
- Integrate `canonical-manager-rs` to download and cache packages chosen by the user.
- Design the IR module:
  - Resource metadata, element tree, bindings, slicing, invariants.
  - Profile resolution (base + differential merge) with unit tests.
- Expose `inkgen-core` APIs:
  - `PackageResolver` (manifest-aware, sanitizes package identifiers).
  - `ProfileService` (flattened profiles, must-support, terminology hooks).
- Add initial snapshot tests for IR serialization to ensure deterministic ordering.

### Phase 2 — CLI Scaffolding (Week 2-3 overlap)
- Implement `inkgen-cli` scaffolding with `clap`.
- Support commands:
  - `inkgen fetch` → delegates to canonical manager, caches under `target/inkgen/packages/`.
  - `inkgen generate typescript` → stub pipeline invoking core + TS backend.
  - `inkgen config init` → emits starter `inkgen.toml`.
- Parse `inkgen.toml`:
  - Package list (allow deduplicated names without `hl7.fhir` prefix).
  - Tree-shaking block (base resource families, explicit allowlists).
  - Language-specific configuration sections with defaults.
- Wire `tracing` subscribers and human-friendly error messages.

### Phase 3 — TypeScript Backend MVP (Week 4-8)
- Implement `LanguageGenerator` trait in `inkgen-typescript`.
- Define Tera template hierarchy:
  - Resource class template.
  - Profile class template (differential-aware).
  - Builder module template (supports interface-only / class-only / combined output).
  - Index + metadata templates.
- Add configuration-driven toggles:
  - `mode = "interface" | "class" | "class_with_builder"`.
  - Structural guard emission flag (basic shape checks only).
  - Naming convention (PascalCase default, overrideable).
- Ensure CCDA packages render correctly:
  - Test sanitized package directory names (drop `hl7.fhir` prefix).
  - Handle CCDA-specific extensions and choice types.
- Produce `insta` snapshots against:
  - Selected `hl7.fhir.r4.core` profiles (Patient, Observation, etc.).
  - CCDA example package (e.g., `hl7.fhir.us.ccda`).
- Update `justfile` with `generate` recipes utilizing CLI commands.

### Phase 4 — Profile Enhancements (Week 9-12)
- Extend IR & generator to emit:
  - Terminology binding helpers (literal unions + fallback types).
  - Extension wrappers (standard + named profiles).
  - Discriminated unions for hierarchies (profiles deriving from base resources).
- Add structural guard enrichments (must-support flags, fixed values).
- Expand snapshot suite with IGs: US Core, IPS.
- Provide CLI switches to include/exclude above features.

### Phase 5 — Extensibility & Tooling (Week 13-16)
- Harden trait interfaces for future languages; document extension points inline (`rustdoc` + `CONTRIBUTING.md`).
- Allow optional template overlays via manifest (non-default, requires tests).
- Implement performance profiling (feature-gated `rayon` parallelism).
- Draft skeleton crates for next language (e.g., Kotlin) to validate interfaces.
- Investigate diff tooling for generated outputs (prototype command, may remain experimental).

### Phase 6 — Hardening & Release Prep (Week 17+)
- Finalize release workflow (`cargo release`, changelog automation).
- Build documentation site structure (mdBook or Docusaurus) consuming ADRs and guides.
- Collect user feedback, iterate on template ergonomics and configuration UX.
- Evaluate integrations: VS Code snippets, OpenAPI cross-generation hooks.

## 3. Recurring Execution Checklist

1. **Story Kickoff**
   - Confirm scope aligns with ADR/plan.
   - Identify required `just` commands; add new ones if needed.
   - Define expected snapshots/fixtures up front.
2. **Implementation**
   - Update `inkgen.toml` schema or defaults if configuration changes.
   - Keep `LanguageGenerator` trait implementations backward compatible.
   - Ensure logging/tracing covers new code paths.
3. **Testing**
   - Run `just test` (unit + integration).
   - Run `just snap`; review via `cargo insta review` when snapshots change.
   - Add new fixtures for CCDA or IG cases as required.
4. **Documentation**
   - Update README / CONTRIBUTING / docs when user-facing behavior changes.
   - Note any new commands or toggles in the `justfile`.
5. **Pre-merge**
   - `just review` (fmt, clippy, tests).
   - Ensure ADR references remain accurate; add follow-up ADRs if decisions pivot.

## 4. Command Inventory (to be defined in `justfile`)

- `just bootstrap` — Install toolchain, run `rustup component add clippy rustfmt`, fetch deps.
- `just fetch PACKAGE=? VERSION=?` — Wrapper around `inkgen fetch`.
- `just generate lang=typescript config=inkgen.toml` — Invoke CLI with manifest.
- `just test` — `cargo test --all`.
- `just snap` — `cargo insta test`.
- `just review` — `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --all`.
- `just clean` (optional) — Remove `target/inkgen` artifacts when needed.

## 5. Risk Watchlist & Mitigations

- **Manifest complexity**: add schema validation and friendly diagnostics; include example configs in `docs/examples/`.
- **Snapshot churn**: group template changes per feature, communicate via PR summaries, and keep templates formatted consistently.
- **Profile merge failures**: log actionable errors, add regression tests for known tricky profiles (CCDA, US Core).
- **Performance regressions**: baseline large-package runs (US Core, IPS) and document expected timings in release notes.

## 6. Tracking & Future Considerations

- Maintain ADR updates as decisions evolve (e.g., when introducing dynamic plugins or validation engines).
- Assess WASM/plugin architecture once multiple in-repo generators exist and externalcontributors request runtime expansion.
- Revisit validation scope after structural guards prove stable; plan for optional deeper validators as a separate ADR.
