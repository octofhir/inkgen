# Task: Hardening and Release Prep

## Summary
Finalize operational readiness for releasing Inkgen: documentation site, release automation, user feedback loop, and ecosystem integrations.

## Background
Phase 6 of the coding agent plan emphasizes long-term sustainability once the core generator is feature-complete. By this stage, the CLI, core engine with element trees and genealogy resolution, and TypeScript backend with nested types, value sets, profiles, and extensions should be stable; the focus shifts to publishing, documentation, and closing the loop with users.

## Context Snapshot
- Release automation must prevent publishing crates unintentionally (we ship only the CLI) and coordinate workspace version bumps.
- Documentation should surface ADRs, task backlog, and practical how-to guides in a single location.
- Feedback channels (GitHub templates, surveys) need to capture issues around template customization, performance, and future language support.
- Integrations like VS Code snippets or OpenAPI bridges improve adoption but should not destabilize the core.

## Detailed Work Breakdown
1. **Release Automation**
   - Configure `cargo release` (or `release-plz`) with workspace-aware settings: only `inkgen-cli` publish=true, others publish=false.
   - Define steps for changelog generation (e.g., `git-cliff`, `cliff.toml`) and update instructions in CONTRIBUTING.
   - Add GitHub Actions workflow to run release dry-run on tags, ensuring signatures and binaries are produced (if distributing binary artifacts).
2. **Documentation Site**
   - Choose tooling (`mdBook`, `Docusaurus`, `VitePress`) consistent with team preferences.
   - Structure docs: Introduction, Getting Started, Architecture (link to ADR 0001), Task Backlog, Extending Inkgen, Troubleshooting.
   - Automate doc builds via CI (preview deploy to GitHub Pages/Netlify) and provide `just docs:serve` command for local preview.
3. **Feedback Channels**
   - Create GitHub issue templates (bug, feature request, template overlay support) and pull request checklist referencing tests/snapshots.
   - Set up discussion board or use GitHub Discussions; document how to submit feedback.
   - Aggregate early adopter findings into `docs/feedback/` or ADR addendum.
4. **Template Ergonomics Polish**
   - Review logs, errors, and CLI UX for improvements; ensure overlays and config errors are clearly messaged.
   - Add `inkgen doctor` (optional) command verifying environment prerequisites (Rust version, TypeScript availability, cache permissions).
5. **Ecosystem Integrations**
   - Prototype VS Code snippet pack or extension that references generated code patterns; document installation steps.
   - Explore OpenAPI generation bridge: produce proof-of-concept command or note future work if infeasible.
   - Document integration status and roadmap (table summarizing ready, in progress, future).
6. **Governance & Support**
   - Update CONTRIBUTING with release cadence, triage expectations, labeling scheme.
   - Add CODE_OF_CONDUCT if missing and link from README & docs.
   - Provide security policy (SECURITY.md) describing how to report vulnerabilities.

## Acceptance Criteria
- Running release dry-run (`cargo release --workspace --dry-run`) completes without errors and produces expected changelog/version updates.
- Documentation site builds locally (`just docs:serve`, `just docs:build`) and via CI with no broken links or lint failures.
- Issue/PR templates and discussion channels are live; README and docs instruct contributors where to report feedback.
- Optional integrations compile/test if included; otherwise, explicit TODOs reference follow-up tasks or ADR proposals.
- Workspace continues to pass formatting, linting, test, and snapshot checks with zero warnings.

## Scope Boundaries
- Do not publish crates or binaries to crates.io/npm as part of this task; focus on dry-runs and automation readiness.
- Deep new feature work (e.g., new language backend) remains out of scope, though documentation should reference roadmap.
- Avoid locking in paid services for docs/feedback without separate approval.

## Dependencies
- `TASK-0006 Extensibility and Tooling`

## Follow-up Notes
- Retrospective findings should spawn new ADRs or tasks; ensure backlog grooming practices are documented.
- Once release automation is validated, schedule a live release dry-run with the team to practice cutovers.
