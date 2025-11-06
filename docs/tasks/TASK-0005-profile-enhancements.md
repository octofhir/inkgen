# Task: Profile Enhancements

## Summary
Enhance the TypeScript backend to support advanced profile features: complex extensions with typed accessors, discriminator-based unions, FHIRPath invariant validation, and profile helper functions for US Core and IPS implementation guides.

## Background
TASK-0004 establishes foundation for basic profile generation with constraints, mustSupport, and simple value set bindings. This task extends that foundation to cover advanced profile ergonomics demanded by real-world IGs: complex extension handling, discriminated unions for sliced elements, FHIRPath-based invariant enforcement, and helper functions for attaching/extracting profile data.

## Context Snapshot
- Basic profile generation, value set bindings, and mustSupport rendering established in TASK-0004
- Extensions need typed accessors and consistent naming to avoid manual patches by consumers
- Discriminators in profiles like `Observation.component` require discriminated unions for type-safety
- FHIRPath invariants should be validated at runtime where feasible
- Profile helper functions needed for attaching/extracting profile data from base resources
- All enhancements must continue honoring the manifest-specified packages; no assumptions about global registries

## Detailed Work Breakdown
1. **IR Enhancements** (extending TASK-0004 foundation)
   - Model complex extension metadata: URLs, cardinalities, complex value types, and context (resource/path)
   - Enhance discriminator representation within slicing info (type discriminators, value discriminators, rules)
   - Add FHIRPath expression storage for invariants with severity and human-readable text
   - Track extension definitions with their contexts and allowed value types

2. **Advanced Terminology Features**
   - Generate helper modules for complex code systems (e.g., `ObservationBindings`)
   - Implement `asCoding()` helper functions converting string literals to Coding objects
   - Support `preferred` binding strength with union + string fallback pattern
   - Generate Coding/CodeableConcept helpers for common value sets

3. **Complex Extension Support**
   - Generate named extension wrapper interfaces with typed accessor methods
   - Implement helper functions to attach/detach extensions from resources:
     - `attachExtension(resource, extensionData)` - add extension to resource
     - `extractExtension(resource, url)` - get extension data from resource
   - Generate extension definitions as first-class TypeScript types
   - For unknown extensions (not in manifest), emit typed placeholder interfaces with documentation
   - Handle modifier extensions separately with runtime validation

4. **Discriminator-Based Unions**
   - For sliced elements with discriminators, emit TypeScript discriminated unions
   - Support multiple discriminator types:
     - Type discriminators (discriminate by type.code)
     - Value discriminators (discriminate by fixed value)
     - Pattern discriminators (discriminate by pattern match)
     - Profile discriminators (discriminate by profile URL)
   - Generate builder helpers enforcing valid combinations at compile-time
   - Update structural guards to check discriminator values at runtime when enabled

5. **FHIRPath Invariant Validation**
   - Parse FHIRPath expressions from invariant constraints
   - Generate runtime validation functions for evaluable invariants
   - Emit helpful error messages referencing constraint keys and human text
   - Provide configuration toggle for invariant enforcement (default: warnings only)
   - Document limitations for complex FHIRPath features not yet supported

6. **Profile Helper Functions**
   - Generate `attach<ProfileName>To<ResourceName>()` functions
   - Generate `extract<ProfileName>From<ResourceName>()` functions
   - Generate `is<ProfileName>()` type guard functions checking profile URL in meta
   - Generate validation functions checking profile constraints are met

7. **Fixture Expansion**
   - Add US Core and IPS packages to fixtures (ensure licensing compliance and caching)
   - Update snapshots covering:
     - Observation with complex terminology bindings and helper functions
     - Patient with US Core extensions (race, ethnicity) with typed accessors
     - IPS profile showing discriminated slices and unions
     - Resources with FHIRPath invariants and validation
   - Write integration tests verifying `tsc --noEmit --strict` across new fixtures
   - Test structural guard toggles with discriminator validation
   - Assert that manifests referencing multiple packages (core + IG) propagate correctly

8. **Documentation & Developer Experience**
   - Update README/CONTRIBUTING with advanced configuration options
   - Document extension helper patterns and usage examples
   - Document discriminator union patterns
   - Document FHIRPath validation capabilities and limitations
   - Provide migration guide for users upgrading from TASK-0004 to TASK-0005 features
   - Add troubleshooting guide for common profile generation issues

## Acceptance Criteria
- Complex extension types generated with typed accessor interfaces
- Extension helper functions (attach/extract) work correctly and type-check
- Discriminated unions generated for sliced elements with discriminators
- FHIRPath invariants evaluated at runtime where feasible, with clear error messages
- Profile helper functions (attach/extract/isProfile/validate) generated and tested
- Snapshot tests cover all advanced features and pass `cargo insta test`
- Generated TypeScript compiles with `tsc --noEmit --strict` for US Core and IPS packages
- Extension handling works for both known extensions (in manifest) and unknown extensions
- Discriminator validation prevents invalid combinations at compile-time and runtime
- Configuration toggles for invariant enforcement tested and working
- IR changes maintain backward compatibility (add `#[non_exhaustive]` where appropriate)
- Workspace formatting, linting, and testing remain warning-free
- Generators honor only packages declared in `inkgen.toml`

## Scope Boundaries
- Basic profile generation and simple value set bindings established in TASK-0004
- Full FHIRPath evaluation beyond common patterns may be deferred; document gaps clearly
- Do not implement terminology service lookups (runtime); stick to compile-time bindings
- No additional languages are modified in this task (focus remains on TypeScript and core IR)
- Complex FHIR operations (search, batch) remain out of scope

## Dependencies
- `TASK-0004 TypeScript Backend MVP`

## Follow-up Notes
- Capture open questions on validation depth for future ADR updates if new invariants require design decisions.
- If runtime performance regresses, document findings for TASK-0006 (performance tooling).
