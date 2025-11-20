# Quick Start

Get started with InkGen in just a few minutes!

## Your First TypeScript SDK

### 1. Install InkGen

Install InkGen via Cargo:

```bash
cargo install inkgen-cli
```

Or build from source:

```bash
git clone https://github.com/octofhir/inkgen.git
cd inkgen
cargo build --release -p inkgen-cli
```

### 2. Initialize Your Project

Create a configuration file:

```bash
inkgen config init
```

This creates an `inkgen.toml` file. Edit it to specify which FHIR packages you want:

```toml
[packages]
hl7-fhir-r4-core = "4.0.1"

[languages.typescript]
output_dir = "./generated"
mode = "interface"
```

### 3. Fetch FHIR Specifications

Download the FHIR packages:

```bash
inkgen fetch
```

### 4. Generate Code

Generate TypeScript code from the FHIR specifications:

```bash
inkgen generate typescript
```

### 5. Use the Generated Code

Your TypeScript code will be in the `./generated` directory:

```typescript
import { Patient, Observation } from './generated';

const patient: Patient = {
  resourceType: 'Patient',
  name: [{
    family: 'Smith',
    given: ['John']
  }],
  birthDate: '1980-01-01'
};
```

## Learn More

- [Configuration](./configuration.md) - Customize InkGen behavior
- [Architecture](../architecture/README.md) - Understand how InkGen works
- [Advanced Topics](../advanced/README.md) - Explore advanced features
