# Quick Start

Get started with InkGen in just a few minutes.

## Your First Profile

### 1. Create a Project

```bash
mkdir my-fhir-project
cd my-fhir-project
npm init -y
npm install @octofhir/inkgen
```

### 2. Create a FHIR Shorthand File

Create a file named `MyPatient.fsh`:

```fsh
Profile: MyPatient
Parent: Patient
Title: "My Patient Profile"
Description: "A custom Patient profile for my application"

* name 1..* MS
* birthDate 0..1 MS
* gender 1..1 MS
```

### 3. Generate Code

```bash
npx inkgen generate MyPatient.fsh --output ./generated
```

### 4. Use the Generated Code

The generated code will be available in the `./generated` directory, ready to be integrated into your application.

## Learn More

- [Configuration](./configuration.md) - Customize InkGen behavior
- [Architecture](../architecture/README.md) - Understand how InkGen works
- [Advanced Topics](../advanced/README.md) - Explore advanced features
