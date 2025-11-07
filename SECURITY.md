# Security Policy

## Reporting a Vulnerability

The InkGen team takes security seriously. If you discover a security vulnerability in InkGen, please report it responsibly to us.

### Do Not Open Public Issues

**Please do not open public GitHub issues for security vulnerabilities.** This gives malicious actors time to exploit the vulnerability before a fix is available.

### Reporting Process

1. **Email us at**: funyloony@gmail.com with the subject line `[SECURITY] InkGen Vulnerability`
2. **Include details**:
   - Description of the vulnerability
   - Steps to reproduce (if applicable)
   - Affected versions
   - Suggested fix (if you have one)
3. **Please allow time** for us to respond (typically 48-72 hours) and prepare a fix

### What to Expect

- We will confirm receipt of your report
- We will work with you to understand the full impact
- We will create a patch and test it
- We will coordinate disclosure and release timing
- We will credit you publicly (unless you prefer anonymity)

## Supported Versions

InkGen follows semantic versioning. Security updates are provided for:

| Version | Status |
| --- | --- |
| 1.x | Supported (when released) |
| 0.1.x | Current active development |
| < 0.1.0 | Unsupported (pre-release) |

## Security Considerations

### Generated Code

InkGen-generated code targets production healthcare applications. The generated code:

- **Type Safety**: Leverages language type systems to catch errors at compile time
- **Validation**: Includes runtime validators for profile constraints and cardinalities
- **No Dynamic Execution**: Generated code does not use `eval()` or equivalent
- **No Secrets**: Generated code does not embed API keys or credentials

### User Input Handling

- Generated validators sanitize and validate FHIR data
- Template overlays are loaded from the local filesystem only
- Configuration files are parsed securely (no arbitrary code execution)

### Dependencies

InkGen's dependencies are regularly audited:

```bash
cargo audit
```

We aim to keep dependencies up-to-date and pin transitive dependency versions where critical.

## Best Practices for Users

1. **Keep InkGen Updated**: Run `cargo install --upgrade inkgen-cli` regularly
2. **Validate Generated Code**: Review generated code changes in version control
3. **Secure Credentials**: Never hardcode API keys or credentials in InkGen config files; use environment variables
4. **Code Review**: Treat generated code like any other code; include it in your review process
5. **Test Templates**: If using custom template overlays, test them thoroughly

## Dependency Security

InkGen uses a minimal set of production dependencies. See [Cargo.toml](Cargo.toml) for the complete dependency tree.

**Key security-relevant dependencies**:
- `serde`/`serde_json` - JSON parsing (widely audited)
- `tera` - Template engine (no arbitrary code execution)
- `tokio` - Async runtime (widely used in Rust ecosystem)

## Cryptography

InkGen does not implement cryptographic functionality directly. If you need to secure FHIR data:

- Use TLS/HTTPS for API communication
- Consider integrating with healthcare-specific security libraries (e.g., for PHI encryption)
- Follow HIPAA/GDPR guidelines for data handling

## Scope

This security policy covers:
- The InkGen CLI and core library (`inkgen-cli`, `inkgen-core`)
- Official language backends (`inkgen-typescript`, `inkgen-rust`)
- Official plugins and extensions

This policy does NOT cover:
- Third-party language backends
- Custom template overlays (user responsibility)
- Generated code (user responsibility for their specific deployment)

## Security Advisories

Critical security advisories will be published in:
- [GitHub Security Advisories](https://github.com/octofhir/inkgen/security/advisories)
- Our [Documentation Site](https://docs.octofhir.org)
- Pinned issues in GitHub

## Questions?

For non-security questions, please use:
- [GitHub Issues](https://github.com/octofhir/inkgen/issues) for bugs/features
- [GitHub Discussions](https://github.com/octofhir/inkgen/discussions) for questions

---

Thank you for helping keep InkGen and the healthcare ecosystem secure!
