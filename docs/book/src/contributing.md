# Contributing

We welcome contributions to InkGen! This guide will help you get started.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR-USERNAME/inkgen.git`
3. Create a feature branch: `git checkout -b feature/my-feature`
4. Follow the [Development Setup](#development-setup)

## Development Setup

### Prerequisites

- Node.js 18+
- Rust (for native components)
- npm or yarn

### Install Dependencies

```bash
git clone https://github.com/octofhir/inkgen.git
cd inkgen
npm install
```

### Build

```bash
npm run build
```

### Test

```bash
npm test
```

### Lint

```bash
npm run lint
```

## Making Changes

### Code Style

- Follow existing code conventions
- Use TypeScript for all new code
- Run `npm run format` to auto-format code
- Add unit tests for new features

### Commit Messages

Use conventional commits format:

```
feat: add new feature
fix: fix bug
docs: update documentation
test: add tests
```

### Pull Requests

1. Push your branch to your fork
2. Create a PR against `main`
3. Describe your changes clearly
4. Ensure all checks pass
5. Request review from maintainers

## Reporting Issues

Use GitHub Issues to report bugs or suggest features:

1. Check existing issues first
2. Provide a minimal reproduction case
3. Include environment details
4. Attach relevant files

## Code of Conduct

Please be respectful and constructive in all interactions.

## Questions?

- Ask in GitHub Discussions
- Check the [Documentation](./README.md)
- Review [Architecture](./architecture/README.md)

Thank you for contributing!
