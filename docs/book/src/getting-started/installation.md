# Installation

## Prerequisites

Before installing InkGen, ensure you have the following installed:

- **Rust** stable toolchain (edition 2024 support)
- **[just](https://github.com/casey/just)** command runner (recommended for development)

## Installation Steps

### From GitHub (Source)

InkGen is currently available via GitHub. Clone and build the repository:

```bash
git clone https://github.com/octofhir/inkgen.git
cd inkgen
cargo build --release -p inkgen-cli
```

The binary will be at `./target/release/inkgen`.

### Install just (Recommended)

For development workflows, install the `just` command runner:

```bash
cargo install just
```

Then bootstrap the project:

```bash
just bootstrap
```

## Verification

To verify the installation was successful:

```bash
./target/release/inkgen --version
```

Or if you added the binary to your PATH:

```bash
inkgen --version
```

You should see the version number of InkGen.

## Next Steps

After installation, check out the [Quick Start](./quickstart.md) guide to get started with your first FHIR code generation.
