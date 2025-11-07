# Architecture Overview

InkGen is a FHIR code generation tool that transforms FHIR Shorthand (FSH) specifications into implementation-ready code.

## High-Level Architecture

```
FSH Input
   ↓
FSH Parser
   ↓
Semantic Analysis
   ↓
Intermediate Representation (IR)
   ↓
Backend Codegen
   ↓
Generated Code
```

## Key Components

### 1. FSH Parser
Parses FHIR Shorthand syntax into an abstract syntax tree (AST).

### 2. Semantic Analyzer
Validates profiles and resolves references to FHIR base resources and value sets.

### 3. Intermediate Representation
Generates a language-agnostic intermediate representation of the profile structure.

### 4. Backend Code Generators
Transforms the IR into implementation-specific code (TypeScript, Java, etc.).

## Design Principles

- **Modularity** - Each component has a single responsibility
- **Extensibility** - Easy to add new backends and analyzers
- **Performance** - Efficient processing of large profile sets
- **Correctness** - Rigorous validation at each step

See [Core Concepts](./concepts.md) for more details.
