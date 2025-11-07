# Troubleshooting

Common issues and solutions when using InkGen.

## Build Errors

### "Profile not found" Error

**Problem:** InkGen reports that a profile or resource cannot be found.

**Solution:**
1. Check that the profile file exists and is in the input directory
2. Verify the spelling of the profile name
3. Ensure all imports are correct: `import { Profile } from ...`
4. Check FHIR version compatibility

### "Invalid cardinality" Error

**Problem:** InkGen rejects the cardinality specification.

**Solution:**
- Use correct format: `min..max`
- Examples: `0..1`, `1..*`, `0..*`
- Ensure min ≤ max

### Memory Issues

**Problem:** InkGen runs out of memory on large profile sets.

**Solution:**
1. Increase Node.js heap: `NODE_OPTIONS="--max-old-space-size=4096" npm run build`
2. Split profiles into smaller batches
3. Use incremental generation: `--incremental`

## TypeScript Generation Issues

### Type Not Found

**Problem:** Generated code references undefined types.

**Solution:**
1. Ensure FHIR type definitions are installed
2. Check `tsconfig.json` includes: `@types/fhir`
3. Verify imports in generated files

### Validation Function Errors

**Problem:** Validation functions fail at runtime.

**Solution:**
1. Update FHIR libraries to latest version
2. Check that input data matches profile constraints
3. Enable debug logging: `INKGEN_DEBUG=true`

## Performance Issues

### Slow Generation

**Problem:** Code generation takes too long.

**Solution:**
1. Enable parallelization: `"parallel": true` in config
2. Use incremental builds
3. Profile the build: `--profile`
4. Check disk I/O performance

## Getting Help

- Check the [Documentation](./README.md)
- Review [Examples](../architecture/README.md)
- Open an issue on GitHub
- Check the [Roadmap](./roadmap.md) for planned features

## Common Patterns

See [Advanced Topics](./advanced/README.md) for solutions to common problems.
