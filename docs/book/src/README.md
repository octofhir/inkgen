# InkGen: FHIR Code Generation for Modern Languages

Welcome to the InkGen documentation! InkGen is a powerful, extensible code generator that transforms FHIR (Fast Healthcare Interoperability Resources) StructureDefinitions into type-safe, idiomatic code for multiple programming languages.

⚠️ **Development Stage**: InkGen is currently in active development. The TypeScript backend is functional and tested, but the project is evolving and APIs may change. We welcome early adopters and contributors!

## What is InkGen?

InkGen automates the generation of language-specific SDKs from FHIR specifications. Whether you're building a healthcare application in TypeScript or another language, InkGen ensures your code stays in sync with FHIR standards while maintaining flexibility for custom extensions and profiles.

## Key Features

- **Multi-Language Support**: Currently supporting TypeScript with extensible backend architecture for additional languages
- **FHIR Profile Support**: Full support for custom profiles, extensions, and constraints on FHIR resources
- **Type Safety**: Generate strongly-typed code that catches errors at compile time
- **Template Customization**: Override and extend code generation via template overlays
- **Performance Optimized**: Efficiently handles large FHIR packages with built-in caching and parallel processing
- **CI/CD Ready**: Seamless integration with your development pipeline

## Getting Started

If you're new to InkGen, start with the [Getting Started](./getting-started.md) guide.

For experienced developers, explore:
- [Architecture](./architecture/README.md) - Understand how InkGen works internally
- [Language Backends](./backends/README.md) - Learn about supported languages and extending support
- [Advanced Topics](./advanced/README.md) - Customize templates, profiles, and performance

## Architecture Overview

```
FHIR Specifications
       ↓
    Fetch & Cache
       ↓
IR Construction & Resolution
       ↓
Language-Specific Code Generation
       ↓
Formatted Code Output
```

InkGen operates in stages:

1. **Fetch**: Download and cache FHIR specification packages from canonical registries
2. **Parse**: Convert FHIR StructureDefinitions into an Internal Representation (IR)
3. **Resolve**: Build genealogy chains, resolve inheritance, and validate constraints
4. **Generate**: Render language-specific code from templates
5. **Output**: Format and write code to your project

## Quick Links

- **[CLI Reference](./getting-started.md#cli-reference)**: Complete command-line option documentation
- **[Configuration](./getting-started/configuration.md)**: How to configure InkGen for your project
- **[Template Overlays](./advanced/overlays.md)**: Customize code generation templates
- **[Contributing](./contributing.md)**: How to contribute new backends or features

## Support

- **[Troubleshooting Guide](./troubleshooting.md)**: Common issues and solutions
- **[GitHub Issues](https://github.com/octofhir/inkgen/issues)**: Report bugs or request features
- **[GitHub Discussions](https://github.com/octofhir/inkgen/discussions)**: Ask questions and discuss usage

## License

InkGen is licensed under the MIT License. See `LICENSE.md` for details.

---

**Ready to generate code?** → [Installation](./getting-started/installation.md)
