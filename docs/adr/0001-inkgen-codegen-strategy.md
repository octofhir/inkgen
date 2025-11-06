# ADR 0001: Inkgen FHIR Code Generation Strategy

- Status: Proposed
- Date: 2025-01-??
- Deciders: Inkgen maintainers
- Tags: architecture, fhir, codegen, rust, typescript, sdk

## 1. Context

Inkgen should become a best-in-class, extensible code generator that turns canonical FHIR packages (StructureDefinition, ValueSet, CodeSystem, etc.) into SDKs for multiple languages, starting with TypeScript. The tool must be approachable for contributors of varying seniority, offer predictable output through snapshot testing, and support profile-aware generation so that implementation guides can produce ergonomic SDKs.

### 1.1 Landscape (2025 overview)

- **Microsoft FHIR CodeGen**: ships multi-language support (C#, TypeScript, Java, Kotlin, Python). It couples generation logic tightly to language-specific emitters and has limited extensibility for bespoke profile authoring.
- **HL7 `fhir-codegen` / MITRE projects**: provide reference generators but lack polished developer ergonomics, modern testing workflows, or flexible templating.
- **Firely SDK tooling**: focused on .NET with strong conformance, but not designed for cross-language expansion or external contributors.
- **SUSHI / GoFSH**: excel at authoring StructureDefinitions but do not emit SDKs.
- **Community SDKs (e.g., `fhir-kit`, `@types/fhir` bundles)**: offer hand-crafted TypeScript bindings without an automated path from StructureDefinitions.
- **Canonical package management**: the `octofhir/canonical-manager-rs` crate already solves download, caching, and version resolution for `hl7.fhir.*` and IG package registries.
- **Atomic FHIR Codegen (`atomic-ehr/codegen`)**: Bun-based CLI + fluent API with a TypeSchema IR, template overrides, and multi-package support; strong TypeScript ergonomics but JavaScript runtime dependency and limited Rust extensibility.

Gaps we address:

1. Rust-first architecture with a stable intermediate representation (IR) that can target many languages.
2. Profile-aware generation that respects slicing, constraints, terminology bindings, and extensions.
3. First-class testing (snapshot-driven) and templating ergonomics for teams evolving their SDKs.
4. Contributor-friendly CLI and automation (via `justfile`, clear module boundaries, documented workflows).

### 1.2 Goals

1. Deliver a Rust-based CLI (backed by internal crates) that transforms canonical FHIR resources into language SDKs, starting with TypeScript, while only distributing the CLI binary publicly.
2. Provide an extensible plugin system enabling new language backends without modifying core logic.
3. Support StructureDefinition profiles (including differential constraints) so that generated SDKs mirror IG expectations, including challenging domains such as CCDA conversions.
4. Bake in `insta` snapshot testing for deterministic output validation.
5. Document developer workflows through a `justfile` and clear contribution guidelines.
6. Reuse `canonical-manager-rs` for package acquisition to avoid duplicating registry logic and allow users to declare the exact FHIR/IG packages to install before generation.
7. Offer tree-shaking configuration so teams can pick only the necessary base resource families (R4/R5/R6) and profile subsets for generation.

### 1.3 Non-goals (initial scope)

- Implementing a hosted web UI.
- Replacing existing FHIR authoring tools (SUSHI, Forge). Inkgen consumes packages they produce.
- Shipping production-ready generators for every language in the first release; only TypeScript is targeted initially.
- Offering runtime validation or full FHIR server capabilities (may integrate with other OctoFHIR tools later).

### 1.4 Constraints & Principles

- **Language**: All orchestration and plugins implemented in Rust (stable toolchain).
- **Quality**: Deterministic output, `insta`-based regressions, and robust unit/integration tests per crate.
- **Extensibility**: Clean separation between IR construction and language emitters; minimize tightly coupled code.
- **Transparency**: Generated code should be understandable and idiomatic for the target language community.
- **Sustainability**: Prefer well-maintained crates (e.g., `serde`, `tera`, `clap`, `tracing`); keep dependency surface focused.

## 2. Decision

### 2.1 High-level architecture

```
          ┌────────────────────┐
          │   CLI / Workspace  │
          └─────────┬──────────┘
                    │
            (orchestration)
                    │
          ┌─────────▼──────────┐
          │ Core Engine Crate  │
          │ (inkgen-core)      │
          ├────────────────────┤
          │ Canonical package  │
          │ ingestion & cache  │◄─── uses canonical-manager-rs
          │ FHIR resource IR   │
          │ profile resolver   │
          └─────────┬──────────┘
                    │
      ┌─────────────┴─────────────┐
      │ Language plugin interface │
      └──────┬───────────┬────────┘
             │           │
   ┌─────────▼───┐ ┌─────▼────────┐
   │ TS backend   │ │ Future langs │ ...
   │ (inkgen-ts)  │ │ (Rust crates)│
   └──────────────┘ └──────────────┘
```

Key elements:

- **CLI crate (`inkgen-cli`)**: Provides commands for fetching packages, selecting resource subsets, generating code, managing templates, and running smoke tests. Uses `clap` for argument parsing and `tracing` for diagnostics. Public releases focus on this CLI binary; internal crates stay unpublished.
- **Core crate (`inkgen-core`)**: Responsible for loading canonical packages (via dependency on `canonical-manager-rs`), normalizing resources into an internal IR, resolving profile differentials, and exposing traits for code generation.
- **Package selection**: CLI will accept manifest files or repeated flags to declare all required packages (e.g., FHIR base, CCDA, IG-specific artifacts) and pass resolved resources to generators.
- **IR module**: Defines language-neutral models for StructureDefinitions, value sets, bindings, slicing rules, invariants, and derived artifacts (e.g., flattened element trees).
- **Template runtime**: Adopt `tera` for powerful macro support, filters, and readable templates. Expose helper functions (registered filters/functions) to simplify templating complex FHIR constructs.
- **Language plugins**: Each backend lives in its own crate implementing a `LanguageGenerator` trait. They register templates, configuration defaults, and metadata (language name, SDK type). Initial focus is `inkgen-typescript`, with new languages contributed directly in-repo at compile time; future expansion (e.g., dynamic WASM adapters) can build on this trait system.
- **Snapshot testing**: Shared testing utilities live in `inkgen-testing` to streamline `insta` usage. Each backend has golden fixtures deriving from real FHIR packages (e.g., `hl7.fhir.r4.core` subset plus sample IGs).
- **Profile handling**: The core resolves profiles by merging base definitions, applying differentials, and exposing metadata (cardinality, value set bindings, must support flags). Generators can query this enriched IR to emit type-safe constructs (e.g., TypeScript literal unions for fixed values, discriminated unions for choice types, builder helpers for must-support elements).
- **Configuration handling**: Support a workspace manifest (`inkgen.toml`) where users list packages, specify FHIR versions, configure tree-shaking (resource/profile inclusion), and tweak language-level options. Permit dropping registry prefixes (`hl7.fhir`) when deriving output directories to simplify multi-package outputs.

### 2.2 Developer workflows (`justfile`)

We will ship a repository-level `justfile` capturing repeatable commands:

- `just bootstrap` – install Rust toolchain components, fetch git submodules if any.
- `just fetch PACKAGE=hl7.fhir.r4.core VERSION=4.0.1` – wrapper around CLI package download (delegates to canonical manager).
- `just generate PACKAGE_PATH=... LANG=typescript` – run codegen against a given package or IG.
- `just test` – run unit tests across crates.
- `just snap` – execute `cargo insta test`.
- `just review` – run formatting (`cargo fmt`), linting (`cargo clippy`), and tests.

Document defaults (e.g., working directories under `target/inkgen/`), environment variables, and how to update snapshots (`cargo insta review`).

### 2.3 TypeScript backend strategy (phase 1)

- **Generated artifacts**:
  - Class-based representations for resources and profiles with generated builders (configurable: interface-only, class-only, or class + builder modes).
  - Type declarations (e.g., `type PatientProfile` unions for constrained views) alongside class definitions.
  - Lightweight structural guards (shape validation) to ensure generated instances match the minimum required FHIR resource structure; deep semantic validation is explicitly out of scope.
  - Index barrel files and package metadata (package.json scaffolding optional in phase 1).
- **Templating**:
  - Use `tera` templates stored per backend (`templates/ts/`).
  - Provide shared partials for common patterns (resource header, imports, docstrings).
- **Configuration**:
  - CLI options for selecting subset of resources/profiles, toggling builder generation and structural guards, controlling naming conventions.
  - Tree-shaking controls for base resource families (R4, R5, R6) and explicit resource/profile allowlists per package.
  - Allow injection of custom template overlays (user-supplied directories merged with defaults).
- **Profile fidelity**:
  - Flatten differential constraints to produce specialized TypeScript types.
  - Generate discriminated unions for profile hierarchies (e.g., `ObservationBloodPressure` extends `Observation`).
  - Represent terminology bindings as literal unions plus string fallback when binding strength < required.
  - Ensure CCDA package peculiarities (naming, extensions, choice types) are faithfully reflected in emitted classes and builders, with sanitized package names (e.g., dropping `hl7.fhir` prefix) used for output directories.

### 2.4 Extensibility for future languages

- Encapsulate backend-specific logic in trait implementations with metadata:
  ```rust
  pub trait LanguageGenerator {
      fn language(&self) -> &'static str;
      fn register_templates(&self, tera: &mut Tera) -> Result<()>;
      fn generate(&self, ctx: &GenerationContext) -> Result<Vec<EmittedFile>>;
  }
  ```
- Provide utilities for filesystem layout, formatting hooks (e.g., running `deno fmt` or `rustfmt` post-generation).
- Keep IR additions backward compatible; use feature flags if a language needs extra metadata.
- Contributions for new languages land directly in the repository (no dynamic loading yet); architect the trait boundaries so we can later experiment with sandboxed backends (e.g., WASM) without disrupting existing code.

### 2.5 Testing strategy

- Unit tests per module (`inkgen-core`, IR transformations).
- `insta`-based golden tests for end-to-end outputs:
  - Store snapshots under `crates/<backend>/tests/snapshots/`.
  - Use curated fixture packages: minimal core resources + sample IG derived from `StructureDefinition` examples.
  - Provide helper command `just snap` to refresh.
- Smoke tests as CLI integration tests (e.g., `cargo test -p inkgen-cli --test generate_typescript`) covering tree-shaken runs and CCDA packages.
- Static analysis via `cargo fmt --check`, `cargo clippy --all-targets`, `cargo deny` (optional in later phase).

## 3. Consequences

### Positive

- Clear separation of concerns enables parallel work (core vs. TypeScript vs. future backends).
- Snapshot tests give deterministic review diffs, easing contributor onboarding.
- Templating with `tera` offers a balance between readability and power (filters, macros).
- Reusing `canonical-manager-rs` avoids maintaining registry logic and ensures compatibility with HL7 packaging.
- Profile-aware IR unlocks high-value SDK features (must-support enforcement, pre-bound terminology).

### Negative / Trade-offs

- Managing template overlays increases complexity; need documentation and caching strategy.
- `tera` introduces runtime template parsing; must precompile or cache for performance-sensitive workflows.
- Snapshot tests can churn if formatting changes; require contributor education.
- Multi-crate workspace adds maintenance overhead (version bumps, cross-crate dependencies).
- Tree-shaking configuration and manifest parsing add CLI complexity that must be carefully validated.

### Risks & Mitigations

- **FHIR specification updates**: Mitigate by versioned IR modules and tests spanning R4/R4B/R5 packages.
- **Complex profile merges**: Implement validation pipeline and log warnings when differential merge fails; include reference tests from well-known IGs.
- **Template drift**: Provide CLI command to lint templates and run contract tests (render sample contexts).
- **Performance on large IGs**: Profile generation; add caching for resolved profiles and parallelize language emitters via `rayon`.

## 4. Roadmap (2025)

1. **Foundation (Weeks 1-3)**
   - Initialize Rust workspace (`inkgen-cli`, `inkgen-core`, `inkgen-typescript`, `inkgen-testing`).
   - Integrate `canonical-manager-rs` for package fetching and caching.
   - Define IR data structures (resources, elements, bindings, slicing, constraints).
   - Implement differential-to-snapshot resolution with unit tests.
   - Scaffold `justfile` and CI workflows (lint, test, snapshot).

2. **TypeScript MVP (Weeks 4-8)**
   - Create TypeScript template set and generator pipeline.
   - Generate classes + builders (configurable modes) and supporting type aliases for base resources and profiles, including doc comments.
   - Produce initial snapshot fixtures (core resources + sample IG).
   - Implement tree-shaking configuration (base family toggles, explicit allowlists) and sanitized package naming for output directories.
   - Expose CLI command `inkgen generate typescript --package ...`.
   - Document usage in README and contribution guide.

3. **Profile Enhancements (Weeks 9-12)**
   - Add terminology binding exports (literal unions, helper enums).
   - Handle extensions (standard + named profiles).
   - Support derived profile hierarchies and discriminators.
   - Introduce validation helper generation (optional Zod schemas).
   - Expand test fixtures with complex IGs (US Core, IPS).

4. **Extensibility & Tooling (Weeks 13-16)**
   - Stabilize language plugin trait and publish crate-level docs.
   - Allow custom template overlays and configuration files (`inkgen.toml`).
   - Provide sample template repository for new languages.
   - Add performance benchmarks and caching improvements.
   - Prepare for next language (e.g., Kotlin or Python) by drafting backend skeleton.
   - Explore diff tooling for generated outputs (stretch goal beyond MVP).

5. **Hardening (Weeks 17+)**
   - Establish release process (`cargo release`, changelog).
   - Set up documentation site (mdBook or Docusaurus) referencing ADRs.
   - Collect feedback from early adopters; refine template ergonomics.
   - Evaluate additional integrations (VS Code snippets, openapi generation).

## 5. Open Questions

1. Which template customization interface will best balance safety and flexibility (e.g., limited WASM filters vs. fully user-defined Rust extensions)?
2. Should we generate runtime validators by default or gate behind feature flags to avoid heavy dependencies?
3. How should we version generated SDKs relative to FHIR package versions (semantic version mapping strategy)?
4. Do we need built-in diffing tools to compare generated SDKs across IG updates?
5. What is the right mechanism for supporting third-party language plugins once external contributions are needed (e.g., WASM adapters vs. statically linked crates)?

## 6. Next Actions

1. Create repository workspace structure and initial crates per roadmap foundation.
2. Author the `justfile`, `README`, and contribution guide referencing this ADR.
3. Start implementing IR ingestion pipeline with tests against `hl7.fhir.r4.core`.
4. Draft TypeScript templates for a simple profile (e.g., Patient) and establish first snapshots.
