# Determinism Contract

InkGen treats deterministic output as a first-class guarantee, not a side effect.
Reproducible generation is what makes generated SDKs reviewable (clean diffs),
trustworthy in snapshots, and safe to commit and release.

## The guarantee

> Given the **same inputs**, the **same InkGen version**, and the **same
> configuration**, `inkgen generate` produces **byte-identical** output.

"Same inputs" means the same resolved FHIR packages (by name + version), including
transitive dependencies. "Same configuration" means the same `inkgen.toml`
(mode, naming, feature flags, output structure, package filters).

## What is stable

- **File set** — the exact set of generated files.
- **File contents** — byte-for-byte, including import ordering and member order.
- **Ordering** — elements, extensions, invariants, and the structure list are
  sorted by stable keys (canonical URL / id / path), backed by `IndexMap` and
  explicit `sort()` on the IR types. No iteration-order or hash-map randomness
  leaks into output.

## What is intentionally *not* part of the contract

- **Wall-clock timestamps** — InkGen does not embed generation timestamps in
  output. (The `--report` artifact records timing, but it is written to
  `.inkgen/debug/`, never into the generated SDK.)
- **Absolute paths** — appear only in logs (stderr), never in generated files.
- **Output across different InkGen versions** — a version bump may change output;
  that is a reviewable, intentional diff, not a determinism violation.
- **Output across different package versions** — pinning package versions in
  `inkgen.toml` is the caller's responsibility.

## How to verify it yourself

```bash
# Fails (non-zero exit) if regeneration would change the committed output.
# Generates into a temp dir and diffs against your output directory; it never
# modifies the output.
inkgen generate typescript --verify
```

Use this in CI or a pre-commit hook to guarantee your committed SDK matches what
InkGen produces from the current spec + config.

## How InkGen enforces it

- **CI gate** — the project's own CI generates `hl7.fhir.r4.core` twice and fails
  if the two runs differ (`diff -rq`).
- **Snapshot tests** — `insta` golden tests guard representative output.
- **`--verify`** — the same check is available to every consumer.

If you ever observe non-deterministic output for identical inputs/version/config,
that is a bug — please report it.
