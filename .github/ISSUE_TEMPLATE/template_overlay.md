---
name: Template Overlay Support
about: Request or discuss template customization and overlay support
title: '[OVERLAY] '
labels: template-overlay
assignees: ''

---

## Description

Describe how you want to customize code generation via template overlays.

## Current Templates

Which templates are you looking to customize?

- [ ] Resource type definitions
- [ ] Validators
- [ ] Discriminator unions
- [ ] Extensions
- [ ] Value sets
- [ ] Other: ___________

## Use Case

What specific customization do you need? Why can't the default templates meet your needs?

## Proposed Overlay

If applicable, share your template overlay:

```tera
{# Your template here #}
```

## Configuration

How would you configure this in `inkgen.toml`?

```toml
[languages.typescript]
overlays = ["./my-templates"]
```

## Impact

Does this require changes to the base template system, or just custom overlays?

## Related Documentation

- [Template Overlays Guide](../../docs/book/src/advanced/overlays.md)
- [CONTRIBUTING](../../CONTRIBUTING.md)
