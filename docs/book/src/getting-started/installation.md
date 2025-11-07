# Installation

## Prerequisites

Before installing InkGen, ensure you have the following installed:

- **Node.js** (version 18 or higher)
- **npm** or **yarn** package manager
- **Rust** (for development; optional for end users)

## Installation Steps

### From npm

The easiest way to install InkGen is via npm:

```bash
npm install @octofhir/inkgen
```

Or with yarn:

```bash
yarn add @octofhir/inkgen
```

### From Source

To install InkGen from source for development:

```bash
git clone https://github.com/octofhir/inkgen.git
cd inkgen
npm install
npm run build
```

## Verification

To verify the installation was successful:

```bash
npx inkgen --version
```

You should see the version number of InkGen.

## Next Steps

After installation, check out the [Quick Start](./quickstart.md) guide to get started with your first FHIR profile.
