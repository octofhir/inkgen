# Performance Tuning

Optimize InkGen's performance for large projects.

## Build Performance

### Parallel Processing

Enable parallel processing in your configuration:

```json
{
  "parallel": true,
  "maxWorkers": 4
}
```

### Incremental Generation

Use incremental generation to only process changed files:

```bash
npx inkgen generate --incremental
```

## Generated Code Performance

### Code Splitting

Split generated code into multiple modules:

```json
{
  "codeSplitting": {
    "enabled": true,
    "moduleSize": 50000
  }
}
```

### Tree Shaking

Enable tree shaking in your bundler configuration to remove unused code:

```javascript
// webpack.config.js
export default {
  mode: 'production',
  optimization: {
    usedExports: true,
  }
};
```

## Caching

Enable caching to speed up subsequent builds:

```json
{
  "cache": {
    "enabled": true,
    "directory": "./.inkgen-cache"
  }
}
```

## Benchmarking

Profile your generation process:

```bash
npx inkgen generate --profile
```

This will output timing information for each phase of code generation.

## Tips and Tricks

1. **Use selectors** - Only generate code for profiles you need
2. **Lazy load** - Generate code on-demand rather than upfront
3. **Compress output** - Enable compression for generated files
4. **Monitor memory** - Use Node.js heap snapshots to identify leaks

For more information, refer to the [Architecture](../architecture/README.md) section.
