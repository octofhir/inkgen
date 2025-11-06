# Task: Core Engine Foundations

## Summary
Implement the foundational `inkgen-core` capabilities: FHIR package resolution, IR data model for core StructureDefinitions, and deterministic inventory APIs that language backends can depend on.

## Background
ADR 0001 calls for a Rust-first core that consumes canonical packages using `canonical-manager-rs` and produces a stable intermediate representation (IR). Without this, the CLI and language backends cannot reason about StructureDefinitions, ValueSets, or profile constraints. This task operationalizes Phase 1 of the coding agent plan and establishes the contract that every generator relies on the packages declared in configuration—no ad hoc resources allowed.

## Context Snapshot
- Canonical packages (e.g., `hl7.fhir.r4.core`) must be fetched via `canonical-manager-rs` to avoid duplicating registry logic.
- The IR must fully represent base resources, complex types, and primitives from canonical packages; IG profiles will layer on once the base path is stable.
- Snapshot testing (`insta`) is mandated to keep IR output deterministic for review and downstream template rendering.

## Status
- [x] Package cache and base structure provider scoped to canonical packages.
- [x] IR data structures with snapshot-driven parsing for base StructureDefinitions.
- [x] Test harness (`inkgen-testing`) with shared workspace setup and integration coverage.
- [x] Snapshot fixtures for representative IR outputs (Patient, HumanName).
- [ ] Profile resolution (differential merge) and advanced tree-shaking diagnostics.

## Detailed Work Breakdown
1. **Canonical Package Integration**
   - Add `canonical-manager-rs` as a dependency in `inkgen-core`.
   - Design a `PackageCache` abstraction storing packages under `target/inkgen/packages/<vendor>/<package>@<version>`.
   - Implement network fetch with caching and offline guard rails (return cached copy when offline).
   - Expose configuration knobs (e.g., custom cache directory) for later CLI wiring.
   - Ensure all supported artifact types (StructureDefinition, ValueSet, CodeSystem, CapabilityStatement, etc.) from downloaded packages are enumerated and indexed for downstream consumption.
2. **Data Model & IR Definition**
   - Define IR structs/enums representing:
     - `ResourceDefinition`: metadata, base type, and snapshot element information.
     - `ElementDefinition`: cardinality, types (including choice types), slicing info, fixed/required values.
     - `BindingDefinition`: strength, value set references, preferred vs. required semantics.
     - `ExtensionDefinition`, `Invariant`, `ProfileLineage` for base/derived relationships.
   - Implement serde serialization with stable ordering (sorted maps) to support snapshot testing.
   - Document each struct with references to relevant sections of the FHIR specification.
3. **StructureDefinition Pipeline**
   - Parse StructureDefinitions from packages and produce normalized snapshots for base resources, complex types (e.g., `HumanName`), and primitives.
   - Capture enough lineage metadata so future profile support can layer on differential merges without reworking APIs.
   - Detect unsupported derivations or missing bases early and surface actionable diagnostics (profiles may return `Unsupported` for now, but log intent).
4. **Service APIs**
   - Implement `PackageResolver` for fetching/managing packages based on manifest entries.
   - Introduce a `StructureDefinitionProvider` trait that exposes:
     - `list_structures()` partitioned by resource/type/profile.
     - `load_structure(url)` returning the normalized IR for base definitions.
     - Inventory metadata for tree-shaking decisions.
   - Provide a `BaseStructureService` (or equivalent) that implements the trait using cached packages; profiles can add another implementation later.
   - Use `tracing` for debug instrumentation (log package downloads, merge steps, inventory scans).
5. **Tree-Shaking & Configuration Bridge**
   - Read tree-shaking preferences from `inkgen.toml` (`allowed_resources`, future profile lists) and apply them when exposing structures to callers.
   - Ensure config-driven filters are validated (warn if a requested resource is absent).
6. **Testing & Fixtures**
   - Extend `inkgen-testing` crate with helper utilities to load `hl7.fhir.r4.core` (use local fixture or canonical manager).
   - Add `insta` snapshots capturing IR JSON for representative base resources (Patient), complex types (HumanName), and primitives.
   - Write unit tests for:
     - Cache behavior (first call downloads, second call uses cache).
     - Error cases: missing package, invalid StructureDefinition elements, unsupported derivations.
     - Multiple artifact types per package to prove coverage of bundles beyond StructureDefinitions (ValueSet, CodeSystem, etc.).
7. **Documentation & Examples**
   - Add module-level documentation in `inkgen-core` explaining how to fetch packages and inspect the IR.
   - Update README task checklist to reflect progress and link to example IR snapshot files.
   - Provide quick sample snippets showing how future services or backends call the `StructureDefinitionProvider`.

- Snapshot fixtures for representative IR outputs (Patient, complex type) maintained via `insta`.

## Acceptance Criteria
- `cargo test -p inkgen-core` and `cargo test -p inkgen-testing` succeed without warnings. *(pass)*
- `cargo clippy --all-targets -- -D warnings` passes across the workspace.
- Fetching `hl7.fhir.r4.core` through public APIs caches files deterministically and respects offline behavior.
- IR serialization for selected resources is covered by `insta` snapshots; updating snapshots uses `cargo insta review`. *(pass — see `base_structures__patient_structure.snap` & `...human_name_structure.snap`)*
- Error cases (missing package, invalid profile, merge failure) surface actionable diagnostics including remediation tips.
- All StructureDefinitions, ValueSets, and CodeSystems present in configured packages are either represented in the IR or reported via explicit errors; no silent omissions.

## Scope Boundaries
- Do not generate language-specific artifacts; only build the IR and supporting services.
- Do not add US Core or IPS fixtures beyond what is required for coverage; deeper profile work is part of TASK-0005.
- Networking should rely on existing canonical manager capabilities; avoid bespoke HTTP clients.

## Dependencies
- `TASK-0001 Workspace Bootstrap`.

## Follow-up Notes
- Complex IG fixtures (US Core, IPS) can be stubbed as TODOs pointing to later tasks.
- Record unresolved questions (e.g., caching eviction strategy) for future ADR updates.

## Progress Log
- 2025-11-06 — Added `PackageCache` wrapper with canonical-manager integration, base IR structs, and initial `StructureDefinitionProvider`/`BaseStructureService`.
- 2025-11-06 — Introduced `InkgenConfig` loader and tree-shaking bridge, plus `inkgen-testing` helpers for shared workspace setup.
- 2025-11-06 — Added integration tests validating offline caching and StructureDefinition loading (`cargo test -p inkgen-core`, `cargo test -p inkgen-testing`).
- 2025-11-06 — Captured `insta` snapshots for base resource (`Patient`) and complex type (`HumanName`) IR serialization.
