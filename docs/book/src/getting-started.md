# Getting Started with InkGen

This guide walks you through installing and using InkGen for the first time.

## Installation

### Prerequisites

- Rust stable toolchain (edition 2024 support)
- [`just`](https://github.com/casey/just) command runner (recommended)

### From GitHub (Source)

InkGen is currently available via GitHub. Clone and build the repository:

```bash
git clone https://github.com/octofhir/inkgen.git
cd inkgen
cargo build --release -p inkgen-cli
./target/release/inkgen --version
```

For development workflows, install `just` and bootstrap:

```bash
cargo install just
just bootstrap
```

## First Steps

### 1. Create a Configuration File

Create `inkgen.toml` in your project root:

```toml
[packages]
hl7-fhir-us-core = "5.0.1"

[languages.typescript]
output_dir = "./generated"
```

### 2. Fetch FHIR Specifications

Download the FHIR packages referenced in your configuration:

```bash
inkgen fetch --config inkgen.toml
```

This downloads and caches FHIR specifications locally. Check `~/.cache/inkgen/` for cached packages.

### 3. Generate Code

Generate TypeScript code:

```bash
inkgen generate typescript --config inkgen.toml
```

Your generated code appears in `./generated/`.

## Configuration

### Package Selection

Specify which FHIR packages to use:

```toml
[packages]
hl7-fhir-us-core = "5.0.1"
hl7-fhir-r4-core = "4.0.1"
```

### Language-Specific Settings

#### TypeScript

```toml
[languages.typescript]
output_dir = "./generated"

# Optional: customize templates
overlays = ["./my-templates"]

# Optional: configure output
package_name = "fhir-us-core"
```

### Caching

InkGen caches downloaded FHIR packages to avoid re-downloading. The cache location is `~/.cache/inkgen/`.

To work offline:

```bash
inkgen fetch --offline
inkgen generate typescript --offline
```

To clear the cache:

```bash
rm -rf ~/.cache/inkgen/
```

## Project Structure

After generation, your TypeScript project might look like:

```
project/
├── generated/
│   ├── Patient.ts           # Resource definitions
│   ├── Observation.ts
│   ├── index.ts             # Barrel export
│   ├── types/               # Type definitions
│   └── validators/          # Validation helpers
├── src/
│   └── my-code.ts           # Your code
├── inkgen.toml              # InkGen config
└── package.json             # npm config
```

## Next Steps

- **[Configuration Guide](./getting-started/configuration.md)**: Learn all configuration options
- **[TypeScript Backend](../backends/typescript.md)**: Details on TypeScript code generation
- **[Template Overlays](../advanced/overlays.md)**: Customize generated code
- **[Architecture](../architecture/README.md)**: Understand how InkGen works

## Troubleshooting

**Q: "No packages found" error**

A: Verify your `inkgen.toml` references valid FHIR packages. Run `inkgen fetch --dry-run` to test.

**Q: "Resource not found" warnings**

A: Some dependencies might not be available in your package version. Check the FHIR specification for the package version you're using.

**Q: Generated code has syntax errors**

A: Ensure you're using a supported version of TypeScript (3.9+). Check [Troubleshooting](../troubleshooting.md) for more help.

See [Troubleshooting](../troubleshooting.md) for more common issues and solutions.
