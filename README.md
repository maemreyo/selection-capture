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
- ✅ **Optional Async API** - Feature-gated `capture_async(...)` for Tokio-based applications
- 🧪 **Experimental Monitoring Scaffold** - Backend-agnostic monitoring API surface for future selection change streams
- 🔄 **Retry Logic** - Automatic retry with configurable budgets and delays
- ⚡ **Multiple Strategies** - Falls back through different capture methods automatically
- 🎯 **App-Specific Profiles** - Customize behavior per application
- 🔍 **Detailed Tracing** - Full visibility into what happened during capture attempts
- ❌ **Cancellation Support** - Cooperative cancellation via `CancelSignal` trait
- 🧹 **Automatic Cleanup** - Clipboard cleanup after capture

## Platform Support

- **macOS**: Fully implemented (`MacOSPlatform`)
- **Windows**: `windows-beta` includes real clipboard reads, foreground active-app lookup, and initial UIA + legacy IAccessible focused-element capture paths
- **Linux**: `linux-alpha` includes shell-backed clipboard/primary-selection reads, active-app lookup, and AT-SPI focused-descendant capture
- **Other platforms**: Portable API via `CapturePlatform` trait, implementations welcome!

Experimental monitoring support is currently backend-agnostic only. The crate exposes a generic
`MonitorPlatform` trait plus `CaptureMonitor<P>`, and `CaptureMetrics` for aggregating
capture-outcome success/latency statistics from trace data. No OS-specific monitor backend is
wired yet, so event production depends entirely on user-supplied implementations.

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

Enable the optional async wrapper explicitly:

```toml
[dependencies]
selection-capture = { version = "0.1", features = ["async"] }
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
    fn load(&self, app: &ActiveApp) -> AppProfile { AppProfile::unknown(app.bundle_id.clone()) }
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
- `try_capture(...)` → `Result<CaptureOutcome, WouldBlock>` - Non-blocking single-pass variant
- `capture_async(...).await` → `CaptureOutcome` - Optional Tokio-backed wrapper behind the `async` feature

### Configuration

- `CaptureOptions` - Configure timeouts, trace collection, and strategy overrides
- `CapturePlatform` - Trait for platform-specific implementations
- `MonitorPlatform` - Experimental trait for platform-specific selection-change monitoring
- `CancelSignal` - Trait for cooperative cancellation
- `AppAdapter` - Trait for app-specific customizations
- `AppProfileStore` - Trait for persisting app profiles

### Experimental Monitoring

```rust
use selection_capture::{CaptureMetrics, CaptureMonitor, MonitorPlatform};

struct StubMonitor;

impl MonitorPlatform for StubMonitor {
    fn next_selection_change(&self) -> Option<String> {
        Some("example selection".to_string())
    }
}

let monitor = CaptureMonitor::new(StubMonitor);
assert_eq!(monitor.next_event(), Some("example selection".to_string()));
let processed = monitor.run_with_limit(10, |text| println!("selection: {text}"));
assert_eq!(processed, 1);

let mut metrics = CaptureMetrics::default();
// metrics.record_outcome(&capture_outcome);
```

Current limitations:

- No built-in macOS, Windows, or Linux monitoring backend exists yet
- No async stream integration exists yet
- No OS event subscription, lifecycle, or permission handling is implemented yet
- `None` from backend is treated as "no more events" by `run(...)` APIs

### Return Types

- `CaptureOutcome::{Success, Failure}` - Result of capture attempt
- `CaptureStatus` - Detailed status codes for deterministic UX mapping
- `FailureKind` - Categorization of failure modes
- `CaptureTrace` - Complete trace of all attempts made

### Example with Custom Options

```rust
use selection_capture::{CaptureOptions, RetryPolicy};
use std::time::Duration;

let options = CaptureOptions {
    retry_policy: RetryPolicy {
        primary_accessibility: vec![Duration::from_millis(0), Duration::from_millis(80)],
        range_accessibility: vec![Duration::from_millis(0)],
        clipboard: vec![Duration::from_millis(120)],
        poll_interval: Duration::from_millis(20),
    },
    collect_trace: true,
    allow_clipboard_borrow: true,
    overall_timeout: Duration::from_millis(500),
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

Contributions are welcome! Please see our [Contributing Guide](docs/contributing/CONTRIBUTING.md) for details on:

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

# Run the async wrapper tests
cargo test --features async --lib
cargo test --features async --test async_capture

# Check formatting
cargo fmt --check

# Run linter
cargo clippy --all-targets
```

## Documentation

- [API Documentation](https://docs.rs/selection-capture)
- [Specification](docs/technical/SPEC.md)
- [Changelog](CHANGELOG.md)
- [Windows Beta Notes](docs/technical/WINDOWS.md)
- [Monitoring Notes](docs/technical/MONITORING.md)

## Related Projects

This library was extracted from the [zmr-koe](https://github.com/maemreyo/zmr-koe) project, which provides a complete text capture solution.

## License

MIT License - See [LICENSE](LICENSE) file for details.

## Acknowledgments

Thanks to all contributors and the Rust community for making this possible!

---

Built with ❤️ by zamery and contributors
