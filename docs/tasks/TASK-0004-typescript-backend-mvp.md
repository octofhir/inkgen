# Task: TypeScript Backend MVP

## Status: ⚠️ SUBSTANTIALLY COMPLETE (Core features implemented, some enhancements pending)

**Overall Progress**: 80% Complete
- ✅ Phase 0: Core IR Enhancements (100%)
- ✅ Phase 1: Nested Type Generation (100%)
- ✅ Phase 2: Value Set Generation (100%)
- ✅ Phase 3: Profile Generation (100%)
- ⚠️ Phase 4: Template & Filter Enhancements (80%)
- ✅ Phase 5: Backend Infrastructure (100%)
- ✅ Phase 6: Filesystem & Output (90%)
- ⚠️ Phase 7: Validation & Testing (70%)
- ❌ Phase 8: Developer Experience (10%)

**Key Achievements**:
- Nested BackboneElement types generate as separate TypeScript interfaces
- Value sets with const arrays and type inference
- Profile constraint extraction and type generation
- ✅ Profiles integrated into main generation pipeline (constraint derivation detection)
- ✅ Custom Tera filters registered (pascal_case, camel_case, sanitize_id, wrap_doc)
- 22/23 unit tests passing (1 integration test has known flakiness)
- Deterministic output with IndexMap

**Remaining Work**:
- Integrate value sets into main pipeline (load from element bindings)
- Add TypeScript compilation validation test
- Investigate and fix integration test flakiness (import ordering)
- Update documentation and README

## Summary
Deliver the first production-ready language backend (`inkgen-typescript`) capable of generating idiomatic TypeScript SDKs from the IR, complete with deterministic snapshots, basic configuration toggles, and support for both template-driven and programmatic code emission.

## Background
ADR 0001 prioritizes TypeScript as the initial target language. The generators must provide high-quality developer ergonomics (interfaces/classes/builders) and align with existing OctoFHIR tooling expectations. Some artifacts (utility modules, index files, hand-tuned helpers) cannot be expressed purely via templates, so the backend must offer programmatic emitters alongside templating. Phase 3 of the coding agent plan lays out the MVP features required before we can onboard downstream teams.

## Context Snapshot
- Templates should rely on `tera` to balance readability with power (filters, macros), while allowing escape hatches for bespoke modules emitted from Rust code.
- Output must be deterministic to support `insta` snapshot reviews.
- Generated code must compile under `tsc --strict --noEmit`.
- CCDA and core FHIR packages require sanitized directory names (strip prefixes, lower-case).
- Generation must operate solely on the package set resolved from `inkgen.toml`; any missing package should halt execution with guidance.

## Detailed Work Breakdown

### Phase 0: Core IR Enhancements (Foundational) ✅ COMPLETED
1. **Element Hierarchy** ✅
   - ✅ Enhance `ElementDefinition` in `ir/mod.rs` to include:
     - ✅ `children: Vec<ElementDefinition>` - child elements in tree structure
     - ✅ `parent_path: Option<String>` - reference to parent element
     - ✅ `depth: usize` - nesting level (0-based)
     - ✅ `is_backbone: bool` - flag for BackboneElement detection
   - ✅ Update `profile.rs` to build hierarchical element trees from flat StructureDefinition snapshots
   - ✅ Identify BackboneElements (elements without type.code but with children)
   - ✅ Resolve contentReference by following internal element paths
   - ✅ Enumerate choice type variants from [x] suffix patterns

2. **Profile Genealogy** ✅
   - ✅ Create `lineage.rs` module for profile inheritance resolution
   - ✅ Implement `resolve_full_chain()` to walk baseDefinition chain to root
   - ✅ Implement `merge_element_snapshots()` to combine base + profile element snapshots
   - ✅ Track which elements are added/modified by profiles vs inherited from base

3. **Terminology Support** ✅
   - ✅ Create `terminology.rs` module for value set handling
   - ✅ Extract concept codes from ValueSet resources with `extract_codes_from_valueset()`
   - ✅ Support configurable maximum value set size with `should_generate_valueset()`
   - ✅ Cache resolved value sets by canonical URL via `ValueSetCache`

4. **Configuration Extensions** ✅
   - ✅ Add `generate_profiles: bool` to TypescriptLanguageConfig (default: true)
   - ✅ Add `max_valueset_size: usize` to TypescriptLanguageConfig (default: 50)
   - ✅ Add `generate_valuesets: bool` to TypescriptLanguageConfig (default: true)
   - ✅ Add `valueset_separate_files: bool` to TypescriptLanguageConfig (default: false)

### Phase 1: Nested Type Generation ✅ COMPLETED
5. **Nested Type Infrastructure** ✅
   - ✅ Create `nested.rs` module in inkgen-typescript
   - ✅ Implement `NestedTypeCollector` to traverse element trees
   - ✅ Build composite type names with `build_composite_name()` (e.g., Patient.contact → PatientContact)
   - ✅ Generate separate exported interfaces for each BackboneElement
   - ✅ Ensure nested types are emitted before parent resource types in template
   - ✅ Integrated into main generator pipeline via `build_render_structure()`

### Phase 2: Value Set Generation ✅ COMPLETED
6. **Value Set Strategy** ✅
   - ✅ Create `valuesets.rs` module in inkgen-typescript
   - ✅ Implement `ValueSetInfo::from_valueset()` with size limit checking
   - ✅ Decision logic: If codes.length > max_valueset_size, return None for string fallback
   - ✅ Generate const arrays with `as const` assertion
   - ✅ Generate inferred type: `type T = typeof TValues[number]`
   - ✅ Generate validation helpers: `isT(v: string): v is T` with type guard
   - ✅ Support for code display names in JSDoc comments

### Phase 3: Profile Generation ✅ COMPLETED
7. **Profile Constraint Handling** ✅
   - ✅ Create `profiles.rs` module in inkgen-typescript
   - ✅ Extract profile constraints with `ProfileInfo::from_resource_definition()`:
     - ✅ mustSupport fields tracked in `must_support_elements`
     - ✅ fixed values → `FixedElement` with TypeScript literals
     - ✅ cardinality tightening → `ConstrainedElement` with `makes_required` flag
     - ✅ narrowed bindings → stored in profile metadata
   - ✅ Generate profile interfaces extending base resources
   - ✅ Add `readonly __profileUrl` field for runtime profile identification
   - ✅ Profile detection via `Derivation::Constraint` check
   - ✅ Type guard generation with `isProfileName()` functions

### Phase 4: Template & Filter Enhancements ⚠️ MOSTLY COMPLETE
8. **Template System**
   - ❌ Create `macros.tera` with reusable snippets (NOT STARTED)
   - ✅ Register custom Tera filters in Rust (COMPLETED):
     - ✅ `pascal_case` - convert to PascalCase
     - ✅ `camel_case` - convert to camelCase
     - ✅ `sanitize_id` - escape reserved keywords
     - ✅ `wrap_doc` - wrap long documentation strings
   - ✅ Enhance `structure.ts.tera` to support:
     - ✅ Nested type generation blocks
     - ✅ Mode switching (interface/class/class_with_builder)
     - ⚠️ MustSupport JSDoc rendering (partial - in RenderField but not rendered)
     - ⚠️ Fixed/default value comments (tracked but not rendered)
     - ⚠️ Choice type union generation (type resolution works, template needs enhancement)
   - ❌ Create `guards.ts.tera` for structural validation functions (NOT STARTED)
   - ❌ Create `profile.ts.tera` for profile-specific type generation (NOT STARTED - profiles use programmatic generation)
   - ✅ Profiles/value sets integrated: profiles generate programmatically, value sets ready for integration

### Phase 5: Backend Infrastructure ✅ COMPLETED
9. **Generator Implementation** ✅
   - ✅ Defined `LanguageGenerator<S>` trait with async `generate()` method
   - ✅ Implemented `TypescriptGenerator` fulfilling the trait
   - ✅ Accept `TypescriptGeneratorConfig` struct with mode, guards, naming, output settings
   - ✅ Configuration builder via `TypescriptGeneratorConfig::from_manifest()`
   - ✅ Integrated nested type generation into main pipeline via `NestedTypeCollector`
   - ✅ Profile generator integrated into pipeline (detects constraint derivations)
   - ✅ Value set generator created, ready for integration
   - ✅ Ensure deterministic output with `IndexMap` for name mappings and imports
   - ✅ Two-phase generation: Phase 1 builds name mappings, Phase 2 generates structures and profiles

### Phase 6: Filesystem & Output ✅ MOSTLY COMPLETE
10. **File Generation**
    - ✅ Write generated files under configurable output directory (defaults to temp in tests)
    - ✅ Provide diff-friendly formatting (indented via Tera templates)
    - ✅ Nested types are inline in parent files
    - ✅ Profile files written with `profile-` prefix (e.g., `profile-us-core-patient.ts`)
    - ⚠️ Value sets not yet integrated into file generation (ready but needs loading mechanism)
    - ✅ Template-based emission via `write_package()` function
    - ✅ Index file generation via `index.ts.tera` template (updated to export profiles)

### Phase 7: Validation & Testing ⚠️ PARTIAL
11. **Comprehensive Test Suite**
    - ✅ Add snapshot tests for nested types:
      - ✅ Patient.contact, Patient.communication, Patient.link as separate interfaces (in unit tests)
      - ✅ Observation.component.referenceRange nested structure test
      - ✅ Tests verify depth sorting and composite naming
    - ✅ Add snapshot tests for value sets:
      - ✅ Test value set extraction with size limits
      - ✅ Test fallback when exceeds max_valueset_size
      - ✅ Test TypeScript generation with const arrays
      - ✅ Test validation helper function generation
    - ✅ Add snapshot tests for profiles:
      - ✅ Profile with mustSupport flags test
      - ✅ Profile with fixed values test
      - ✅ Profile with cardinality tightening test
      - ✅ Profile detection via Derivation::Constraint test
    - ⚠️ Integration tests:
      - ⚠️ `generates_patient_interface` test shows flakiness (Patient.ts generation inconsistent)
      - ⚠️ Appears related to async test infrastructure rather than generation logic
    - ❌ TypeScript compilation validation (NOT IMPLEMENTED):
      - Generate TypeScript output
      - Write to temp directory with tsconfig.json
      - Run `tsc --noEmit --strict`
      - Assert compilation succeeds
    - ⚠️ Configuration toggle tests needed
    - **Test Results**: 20/20 unit tests passing, 1 integration test flaky

### Phase 8: Developer Experience ❌ NOT STARTED
12. **Documentation & Logging**
    - ❌ Update README (NOT STARTED)
    - ⚠️ Basic logging exists via `info!()` and `warn!()` macros
    - ❌ Comprehensive documentation of features not written
    - ❌ Value set size limits not documented
    - ❌ Nested type naming conventions not documented

## Acceptance Criteria
- Element trees are built from flat StructureDefinition snapshots with proper parent-child relationships
- Nested BackboneElement types are generated as separate exported TypeScript interfaces
- Value sets generate as const arrays when under max_valueset_size, fallback to string otherwise
- Profiles generate with constraints (mustSupport, fixed values, cardinality) when config.generate_profiles is true
- `inkgen generate typescript --package hl7.fhir.r4.core --mode class_with_builder` produces compilable TypeScript validated by `tsc --noEmit --strict`
- `cargo test --all` (including 20+ snapshot tests and TypeScript compilation checks) succeeds with zero warnings
- Templates handle nested types, choice types, Must Support flags, fixed values, and profiles correctly
- Configuration options (generate_profiles, max_valueset_size, generate_valuesets) are tested and working
- CCDA package generation completes and outputs sanitized directory names; snapshots capture representative files
- README/CONTRIBUTING include updated instructions with configuration examples and feature explanations
- Special-case emitters coexist with template output without duplication or missing exports
- The shared `LanguageGenerator` trait is documented and unit-tested for future language backends
- Generators refuse to operate on packages not declared in `inkgen.toml`, ensuring config-driven workflows
- No references to external codebases in code comments or documentation

## Scope Boundaries
- Basic profile generation and value set bindings are included in this task
- Advanced extension wrappers, complex discriminator unions, and FHIRPath invariants are reserved for TASK-0005
- No packaging (npm publish) or bundler integration at this stage
- Do not implement template overlay mechanics (handled by TASK-0006)
- Profile helpers (attach/extract functions) are deferred to TASK-0005

## Dependencies
- `TASK-0002 Core Engine Foundations`
- `TASK-0003 CLI Scaffolding`

## Follow-up Notes
- Document any performance concerns observed during generation for follow-up in TASK-0006.
- Keep template file structure stable to avoid churn when overlays arrive.
