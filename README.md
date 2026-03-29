# selection-capture

[![Crates.io](https://img.shields.io/crates/v/selection-capture.svg)](https://crates.io/crates/selection-capture)
[![Documentation](https://docs.rs/selection-capture/badge.svg)](https://docs.rs/selection-capture)
[![CI](https://github.com/maemreyo/selection-capture/actions/workflows/ci.yml/badge.svg)](https://github.com/maemreyo/selection-capture/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`selection-capture` is a Rust library for selected-text capture with retry, cancellation,
and strategy fallbacks.

It is designed as a replacement foundation for earlier `get-selected-text` style usage,
with explicit capture status and trace metadata for app-level UX decisions.

## Features

- ✅ **Synchronous API** - Simple, blocking calls that are easy to integrate
- 🔄 **Retry Logic** - Automatic retry with configurable budgets and delays
- ⚡ **Multiple Strategies** - Falls back through different capture methods automatically
- 🎯 **App-Specific Profiles** - Customize behavior per application
- 🔍 **Detailed Tracing** - Full visibility into what happened during capture attempts
- ❌ **Cancellation Support** - Cooperative cancellation via `CancelSignal` trait
- 🧹 **Automatic Cleanup** - Clipboard cleanup after capture

## Platform Support

- **macOS**: Fully implemented (`MacOSPlatform`)
- **Windows**: `windows-beta` feature flag exposes a bounded MVP scaffold with compile-safe dispatch and fallback tests. Current attempts return `Unavailable` until backend implementations land.
- **Other platforms**: Portable API via `CapturePlatform` trait, implementations welcome!

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
selection-capture = "0.1"
```

Or use the latest from Git:

```toml
[dependencies]
selection-capture = { git = "https://github.com/maemreyo/selection-capture" }
```

Enable the Windows beta scaffold explicitly:

```toml
[dependencies]
selection-capture = { version = "0.1", features = ["windows-beta"] }
```

## Quick Start (macOS)

```rust
use selection_capture::{
    capture, AppAdapter, AppProfile, AppProfileStore, AppProfileUpdate, CaptureOptions,
    CaptureOutcome, MacOSPlatform, CancelSignal, ActiveApp,
};

// Implement required traits for your use case
struct NeverCancel;
impl CancelSignal for NeverCancel {
    fn is_cancelled(&self) -> bool { false }
}

struct NoopStore;
impl AppProfileStore for NoopStore {
    fn load(&self, _app: &ActiveApp) -> AppProfile { AppProfile::default() }
    fn merge_update(&self, _app: &ActiveApp, _update: AppProfileUpdate) {}
}

fn main() {
    let platform = MacOSPlatform::new();
    let store = NoopStore;
    let cancel = NeverCancel;
    let adapters: [&dyn AppAdapter; 0] = [];
    let options = CaptureOptions::default();

    match capture(&platform, &store, &cancel, &adapters, &options) {
        CaptureOutcome::Success(ok) => println!("Selected text: {}", ok.text),
        CaptureOutcome::Failure(err) => eprintln!("Capture failed: {:?}", err.status),
    }
}
```

## Core API

### Main Function

- `capture(...)` → `CaptureOutcome` - The primary capture function

### Configuration

- `CaptureOptions` - Configure timeouts, trace collection, and strategy overrides
- `CapturePlatform` - Trait for platform-specific implementations
- `CancelSignal` - Trait for cooperative cancellation
- `AppAdapter` - Trait for app-specific customizations
- `AppProfileStore` - Trait for persisting app profiles

### Return Types

- `CaptureOutcome::{Success, Failure}` - Result of capture attempt
- `CaptureStatus` - Detailed status codes for deterministic UX mapping
- `FailureKind` - Categorization of failure modes
- `CaptureTrace` - Complete trace of all attempts made

### Example with Custom Options

```rust
let options = CaptureOptions {
    max_retries: 3,
    retry_delay_ms: 100,
    collect_trace: true,
    strategy_override: None,
};
```

## Permission Notes (macOS)

Depending on the target application and capture strategy, users may need to grant:

1. **Accessibility Permission**  
   `System Settings → Privacy & Security → Accessibility`

2. **Automation Permission**  
   Required for AppleScript fallback in some application combinations

The library will gracefully degrade through available strategies based on permissions.

## Architecture

```
┌─────────────────┐
│  capture()      │
└────────┬────────┘
         │
    ┌────▼────┐
    │ Strategy │
    │ Manager  │
    └────┬────┘
         │
    ┌────▼─────────────────┐
    │ 1. Accessibility API │ ← Primary method
    ├──────────────────────┤
    │ 2. AppleScript       │ ← Fallback 1
    ├──────────────────────┤
    │ 3. Clipboard Monitor │ ← Fallback 2
    └──────────────────────┘
```

Each strategy is attempted in order, with automatic retry and detailed tracing.

## Contributing

Contributions are welcome! Please see our [Contributing Guide](CONTRIBUTING.md) for details on:

- Reporting bugs
- Suggesting features
- Submitting pull requests
- Code style and testing

### Development Setup

```bash
# Clone the repository
git clone https://github.com/maemreyo/selection-capture.git
cd selection-capture

# Build
cargo build

# Run tests
cargo test

# Run the Windows beta smoke path on any host
cargo test --features windows-beta --lib
cargo test --features windows-beta --test windows_smoke

# Check formatting
cargo fmt --check

# Run linter
cargo clippy --all-targets
```

## Documentation

- [API Documentation](https://docs.rs/selection-capture)
- [Specification](SPEC.md)
- [Changelog](CHANGELOG.md)
- [Windows Beta Notes](docs/technical/WINDOWS.md)

## Related Projects

This library was extracted from the [zmr-koe](https://github.com/maemreyo/zmr-koe) project, which provides a complete text capture solution.

## License

MIT License - See [LICENSE](LICENSE) file for details.

## Acknowledgments

Thanks to all contributors and the Rust community for making this possible!

---

Built with ❤️ by zamery and contributors
