# Contributing to selection-capture

Thank you for your interest in contributing to `selection-capture`! This document provides guidelines and instructions for contributing.

## Code of Conduct

Please note that this project is released with a [Code of Conduct](CODE_OF_CONDUCT.md). By participating in this project you agree to abide by its terms.

## How to Contribute

### Reporting Bugs

Before creating bug reports, please check the existing issues as you might find out that you don't need to create one. When you are creating a bug report, please include as many details as possible:

* Use a clear and descriptive title
* Describe the exact steps to reproduce the problem
* Provide specific examples to demonstrate the steps
* Describe the behavior you observed and what behavior you expected
* Include any relevant logs, error messages, or screenshots

### Suggesting Features

Feature suggestions are tracked as GitHub issues. When creating a feature suggestion:

* Use a clear and descriptive title
* Provide a detailed description of the suggested feature
* Explain why this feature would be useful
* List some examples of how this feature would be used

### Pull Requests

* Fill in the required template
* Follow the Rust code style (see [Code Style](#code-style))
* Include appropriate tests
* Update documentation as needed
* Add an entry to the [CHANGELOG.md](CHANGELOG.md) for significant changes

## Development Setup

### Prerequisites

- Rust 1.75 or later
- macOS (for testing, as the library currently only supports macOS)

### Building the Project

```bash
# Clone the repository
git clone https://github.com/maemreyo/selection-capture.git
cd selection-capture

# Build the project
cargo build

# Run tests
cargo test

# Check code style
cargo fmt --check

# Run clippy for linting
cargo clippy --all-targets
```

## Code Style

This project follows standard Rust formatting conventions and uses rustfmt.

* Format your code with `cargo fmt` before submitting
* Run `cargo clippy` to catch common mistakes and improve code quality
* Write clear, self-documenting code with comments for complex logic
* Keep functions focused and small (preferably under 50 lines)

## Testing

* Write tests for new functionality
* Ensure all existing tests pass
* Include both unit tests and integration tests where appropriate
* Test on different macOS versions if possible

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## Documentation

* Update README.md for user-facing changes
* Add doc comments for public APIs
* Keep examples up-to-date
* Document any breaking changes clearly

## Commit Messages

Follow conventional commit message format:

```
feat: add support for XYZ app
fix: handle edge case in ABC strategy
docs: update installation instructions
test: add tests for capture engine
chore: update dependencies
```

## Release Process

Releases follow semantic versioning (MAJOR.MINOR.PATCH):

* **MAJOR** version for incompatible API changes
* **MINOR** version for backwards-compatible functionality additions
* **PATCH** version for backwards-compatible bug fixes

## Questions?

Feel free to open an issue with the "question" label if you have any questions about contributing.

Thank you for contributing to `selection-capture`! 🎉
