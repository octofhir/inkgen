# Architecture

InkGen's modular architecture enables efficient FHIR code generation while maintaining extensibility for new languages and features.

## Directory Overview

- **overview.md** - High-level system design and data flow
- **concepts.md** - Core concepts: IR, genealogy, profiles, extensions
- **ir-design.md** - Internal Representation (IR) structure and evolution
- **type-system.md** - Type mapping and resolution across languages

## Key Principles

1. **Separation of Concerns**: FHIR parsing, IR construction, and code generation are independent
2. **Extensibility**: Add new language backends without modifying core
3. **Performance**: Caching, parallel processing, and lazy evaluation where possible
4. **Correctness**: Type safety and constraint validation throughout the pipeline

See [Architecture Decision Records](../../adr/) for design rationale on major decisions.
