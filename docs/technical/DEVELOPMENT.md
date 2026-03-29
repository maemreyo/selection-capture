# selection-capture Development Guide

This guide provides information for developers who want to contribute to `selection-capture`.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Development Workflow](#development-workflow)
- [Testing Strategy](#testing-strategy)
- [Code Style](#code-style)
- [Platform-Specific Development](#platform-specific-development)

## Architecture Overview

### Core Components

```
selection-capture/
├── src/
│   ├── lib.rs           # Library entry point and re-exports
│   ├── engine.rs        # Core capture engine
│   ├── types.rs         # Public type definitions
│   ├── traits.rs        # Core traits (CapturePlatform, CancelSignal, etc.)
│   ├── macos.rs         # macOS platform implementation
│   └── profile.rs       # App profile management
```

### Key Design Decisions

1. **Synchronous API**: The core engine is synchronous to keep integration simple
2. **Trait-based design**: Platform-specific code behind traits for portability
3. **Strategy pattern**: Multiple capture strategies with automatic fallback
4. **Cooperative cancellation**: Via `CancelSignal` trait
5. **Merge-based updates**: App profiles use merge semantics

## Development Workflow

### 1. Setup

```bash
git clone https://github.com/maemreyo/selection-capture.git
cd selection-capture
rustup install stable
rustup default stable
```

### 2. Make Changes

Create a feature branch:

```bash
git checkout -b feature/your-feature-name
```

### 3. Test Locally

Run the full test suite:

```bash
make ci
# or manually:
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose
```

### 4. Update Documentation

If you've changed public APIs:

```bash
cargo doc --no-deps --document-private-items
```

Update README.md if the changes affect usage.

### 5. Commit

Follow conventional commits:

```bash
git commit -m "feat: add your feature description"
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`

### 6. Push and Create PR

```bash
git push origin feature/your-feature-name
```

Then create a pull request on GitHub.

## Testing Strategy

### Unit Tests

Test individual functions and methods in isolation. Place tests in the same file as the code they test, at the bottom in a `#[cfg(test)]` module.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Arrange
        // Act
        // Assert
    }
}
```

### Integration Tests

For testing the full capture flow. These would go in `tests/` directory (not yet created).

### Manual Testing

Since this library interacts with macOS accessibility APIs, manual testing on real macOS systems is important. Test on:

- Different macOS versions (14.x, 15.x, etc.)
- Different applications (Safari, Chrome, TextEdit, etc.)
- Different permission states

## Code Style

### Formatting

All code must be formatted with `rustfmt`:

```bash
cargo fmt --all
```

### Linting

All code must pass `clippy` with no warnings:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Naming Conventions

- Types: PascalCase (`CaptureOutcome`, `AppProfile`)
- Functions: snake_case (`capture`, `merge_update`)
- Constants: UPPER_SNAKE_CASE (`DEFAULT_TIMEOUT_MS`)
- Traits: PascalCase with descriptive names (`CancelSignal`, `AppAdapter`)

### Documentation

All public items must have doc comments:

```rust
/// Brief description of what this function does.
///
/// More detailed explanation if needed.
///
/// # Arguments
///
/// * `param_name` - Description of parameter
///
/// # Returns
///
/// Description of return value
///
/// # Example
///
/// ```rust
/// // Example code here
/// ```
pub fn example_function(param_name: Type) -> ReturnType {
    // ...
}
```

## Platform-Specific Development

### Adding a New Platform

To add support for a new platform (e.g., Windows, Linux):

1. Create a new file `src/windows.rs` or `src/linux.rs`
2. Implement the `CapturePlatform` trait
3. Add conditional compilation in `lib.rs`:

```rust
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform;
```

4. Update documentation
5. Add platform-specific CI job

### macOS Development Notes

The macOS implementation uses:

- **Accessibility APIs** (`accessibility-ng`, `accessibility-sys-ng`)
- **Active window detection** (`active-win-pos-rs`)
- **Core Foundation** (`core-foundation`)

#### Debugging Accessibility Issues

1. Check permissions:
   ```bash
   tccutil reset Accessibility com.example.app
   ```

2. Monitor accessibility events:
   ```bash
   log show --predicate 'process == "YourApp"' --info
   ```

3. Test with different apps - some have custom text rendering

## Performance Considerations

- Capture should complete within 1-2 seconds typically
- Retry delays are intentionally short (50-100ms) to avoid user-perceptible lag
- Clipboard operations should be minimized
- Trace collection has minimal overhead when disabled

## Common Issues

### Issue: Tests fail on macOS

**Solution**: Ensure you have necessary permissions and run tests from Terminal with appropriate privileges.

### Issue: Clippy complains about complexity

**Solution**: Break down complex functions into smaller, focused functions. Use early returns.

### Issue: Build fails after rustup update

**Solution**: Run `cargo clean` and rebuild. Check MSRV (Minimum Supported Rust Version) compatibility.

## Release Checklist

Before releasing a new version:

- [ ] All tests pass
- [ ] Clippy is happy
- [ ] Documentation builds without warnings
- [ ] CHANGELOG.md is updated
- [ ] Version number updated in Cargo.toml
- [ ] Examples work correctly
- [ ] No breaking changes (or properly documented)

## Getting Help

- Open an issue on GitHub for bugs or questions
- Check existing issues and PRs
- Read the SPEC.md for design rationale

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

Thank you for contributing to `selection-capture`! 🚀
