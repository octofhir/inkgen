# Configuration

InkGen can be configured through configuration files and command-line options.

## Configuration File

Create an `inkgen.config.json` file in your project root:

```json
{
  "input": "./profiles",
  "output": "./generated",
  "backends": ["typescript"],
  "fhirVersion": "r4",
  "strict": true
}
```

### Configuration Options

| Option | Type | Description |
|--------|------|-------------|
| `input` | string | Directory containing FHIR Shorthand files |
| `output` | string | Directory for generated code |
| `backends` | array | List of code backends to use |
| `fhirVersion` | string | FHIR version (r4, r5, etc.) |
| `strict` | boolean | Enable strict validation |

## Command-Line Options

```bash
npx inkgen generate [options]

Options:
  --input, -i      Input directory (default: ./profiles)
  --output, -o     Output directory (default: ./generated)
  --backend, -b    Code backend to use (default: typescript)
  --config, -c     Path to config file
  --help, -h       Show help
  --version, -v    Show version
```

## Environment Variables

Set these environment variables to configure InkGen:

- `INKGEN_INPUT` - Input directory
- `INKGEN_OUTPUT` - Output directory
- `INKGEN_BACKEND` - Default backend
- `FHIR_VERSION` - FHIR version

## Next Steps

Check out the [Architecture](../architecture/README.md) guide to understand how InkGen processes your profiles.
