# Task: Extensibility and Tooling

## Summary
Stabilize extension points for additional language backends, enable template overlays, and introduce performance tooling to keep the generator responsive on large implementation guides.

## Background
Phase 5 of the coding agent plan focuses on preparing the architecture for multiple languages and strengthening developer tooling. After TypeScript with nested types, value sets, and profiles is in place (TASK-0004 and TASK-0005), we need to ensure third parties can build new backends without touching core crates, and that generation remains performant on real-world IGs. We also need to validate that non-template code paths (like Rust generators using specialized libraries) integrate seamlessly with shared traits.

## Context Snapshot
- Plugin ergonomics must remain Rust-first (in-tree crates) while leaving room for future dynamic loading.
- Template overlays allow teams to customize output without forking; validation must prevent runtime surprises.
- Performance instrumentation must catch regressions early, especially for large packages like US Core and IPS.
- Deterministic output remains non-negotiable even with parallelism enabled.

## Detailed Work Breakdown
1. **Generator Trait Hardening**
   - Review and finalize `LanguageGenerator` trait API (constructor parameters, lifecycle hooks).
   - Provide `#[doc = "..."]` examples showing how to implement a new backend, including hybrids (template + manually authored modules).
   - Introduce reusable base traits or helper structs (`TemplateEmitter`, `CustomModuleEmitter`) so programmatic emitters (e.g., Rust backend leveraging a domain-specific library) can be registered consistently.
   - Add versioned feature flags or semantic version guidance to break ties between core changes and backend updates.
2. **Template Overlay Support**
   - Extend manifest to allow `languages.<lang>.overlays = ["path/to/templates"]`.
   - Implement loader merging base templates with overlays; raise descriptive errors when overlays are missing or produce invalid output.
   - Add CLI command `inkgen template lint` (optional) that renders sample IR contexts to validate templates.
3. **New Backend Skeleton**
   - Scaffold `crates/inkgen-rust` (or another target) to prove a backend that relies on a specialized Rust library (e.g., builders, validation helpers) can plug into the base traits without templates.
   - Optionally add `crates/inkgen-kotlin` stub to confirm multiplatform compatibility.
   - Provide tests ensuring stub backends wire into CLI `generate` command behind feature flags and can coexist with TypeScript generator.
   - Document how to add new backend crates (naming conventions, manifest entries, enabling/disabling via features).
4. **Performance Instrumentation**
   - Integrate benchmarking harness (Criterion) measuring:
     - IR construction time for large packages.
     - TypeScript generation time with and without overlays.
   - Add `just bench` recipe and README documentation summarizing expected baseline metrics.
   - Evaluate `rayon` or parallel iterators for generation; guard behind feature flag (`parallel-generation`) and prove determinism by comparing snapshots with and without the flag.
5. **Diff Tooling**
   - Implement CLI command `inkgen diff <old-dir> <new-dir>` to compare generated outputs, highlighting structural differences.
   - Optionally leverage `similar` or `difftastic` libraries for richer output.
   - Add integration tests demonstrating diff command usage.
6. **Documentation & Guides**
   - Expand CONTRIBUTING with "Creating a new backend" walkthrough referencing overlays, benchmarks, and diff tooling.
   - Update README to advertise overlays, benchmarking, and diff capabilities.
   - Provide example overlay repository or inline sample templates.

## Acceptance Criteria
- Example plugin documentation walks through creating a new backend using provided traits and overlays, and passes `cargo doc --document-private-items`.
- Benchmark suite runs via `cargo bench` (or dedicated command) and captures metrics without panics; README documents how to interpret results.
- Feature-flagged parallelism can be toggled without breaking deterministic snapshots (CI runs both modes).
- Template overlay tests ensure missing/invalid templates yield clear errors before generation proceeds.
- Workspace passes formatting, linting, and tests with zero warnings, including new benchmark and diff modules.
- At least one backend example demonstrates mixing template-based and programmatic emission (e.g., Rust backend calling a specialized library) while still respecting manifest-declared packages.

## Scope Boundaries
- Do not ship production-ready Kotlin (or other) backend; only scaffolding to validate extension points.
- Avoid introducing runtime plugin loading yet; keep compile-time crates only.
- No heavy UI/UX for diff tooling—CLI output suffices.

## Dependencies
- `TASK-0004 TypeScript Backend MVP`
- `TASK-0005 Profile Enhancements`

## Follow-up Notes
- Capture insights for future ADR on plugin distribution (dynamic loading vs. compiled crates).
- If benchmarking reveals hotspots, file targeted tasks for optimization (link back here).
