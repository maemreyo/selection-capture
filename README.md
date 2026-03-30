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
- ✅ **Optional Rich Content API** - Feature-gated `capture_rich(...)` / `try_capture_rich(...)` with clipboard HTML/RTF enrichment
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

Experimental monitoring support is scaffolded with a generic `MonitorPlatform` trait plus
`CaptureMonitor<P>`, and `CaptureMetrics` for aggregating capture-outcome success/latency
statistics from trace data. Polling monitor backends are available for macOS
(`MacOSSelectionMonitor`), Windows (`WindowsSelectionMonitor`, `windows-beta`), and Linux
(`LinuxSelectionMonitor`, `linux-alpha`) with per-backend de-duplication. Poll helpers now
include cancellation-aware loops and an optional coalescing mode for bursty event streams.

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

Enable rich-content capture explicitly:

```toml
[dependencies]
selection-capture = { version = "0.1", features = ["rich-content"] }
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
- `capture_rich(...)` → `CaptureRichOutcome` - Optional rich-content capture behind `rich-content`
- `try_capture_rich(...)` → `Result<CaptureRichOutcome, WouldBlock>` - Non-blocking rich variant behind `rich-content`

### Configuration

- `CaptureOptions` - Configure timeouts, trace collection, and strategy overrides
- `CapturePlatform` - Trait for platform-specific implementations
- `MonitorPlatform` - Experimental trait for platform-specific selection-change monitoring
- `CancelSignal` - Trait for cooperative cancellation
- `AppAdapter` - Trait for app-specific customizations
- `AppProfileStore` - Trait for persisting app profiles

### Experimental Monitoring

```rust
use selection_capture::{
    CancelSignal, CaptureMetrics, CaptureMonitor, MacOSMonitorBackend, MacOSSelectionMonitor,
    MacOSNativeEventSource, MacOSSelectionMonitorOptions, MonitorPlatform, MonitorSpamGuard,
};

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

struct StopImmediately;
impl CancelSignal for StopImmediately {
    fn is_cancelled(&self) -> bool { true }
}

let mac_monitor = CaptureMonitor::new(MacOSSelectionMonitor::default());
let _native_pref = MacOSSelectionMonitor::new_with_options(MacOSSelectionMonitorOptions {
    poll_interval: std::time::Duration::from_millis(120),
    backend: MacOSMonitorBackend::NativeObserverPreferred,
    native_queue_capacity: 256,
    native_event_pump: None, // defaults to AxObserverBridge drain pump when native observer activates
    active_pid_provider: None, // optional test/integration hook; defaults to active window PID
});
let _ = _native_pref.enqueue_native_selection_event("hello from observer");
let _ = _native_pref.ingest_native_observer_payload(
    MacOSNativeEventSource::AXObserverSelectionChanged,
    "hello from axobserver callback",
);
let cancel = StopImmediately;
let _processed = mac_monitor.poll_until_cancelled(
    std::time::Duration::from_millis(120),
    &cancel, // replace with your own cancellation signal
    |text| println!("live selection: {text}"),
);

let guard = MonitorSpamGuard {
    suppress_identical: true,
    min_emit_interval: std::time::Duration::ZERO,
    min_emit_interval_same_text: std::time::Duration::from_millis(200),
    normalize_whitespace: true,
    stable_polls_required: 2,
};
let _guarded = mac_monitor.poll_until_cancelled_guarded(
    std::time::Duration::from_millis(120),
    &cancel,
    &guard,
    |text| println!("de-spammed selection: {text}"),
);
let _native_stats = _native_pref.native_observer_stats();
let _stats = mac_monitor.poll_until_cancelled_guarded_with_stats(
    std::time::Duration::from_millis(120),
    &cancel,
    &guard,
    |_text| {},
);

let mut metrics = CaptureMetrics::default();
// metrics.record_outcome(&capture_outcome);
```

Current limitations:

- Native callback integration is fully wired on macOS; Windows/Linux expose native-event-preferred queue-and-pump scaffolds with polling fallback
- macOS `NativeObserverPreferred` uses AXObserver callback ingress with a native-event queue and safe polling fallback
- No async stream integration exists yet
- `None` from backend is treated as "no more events" by `run(...)` APIs
- For anti-spam behavior, prefer `poll_until_cancelled_guarded(...)` with `MonitorSpamGuard`

### Return Types

- `CaptureOutcome::{Success, Failure}` - Result of capture attempt
- `CaptureRichOutcome::{Success, Failure}` - Rich result with plain fallback semantics
- `CaptureStatus` - Detailed status codes for deterministic UX mapping
- `FailureKind` - Categorization of failure modes
- `CaptureTrace` - Complete trace of all attempts made

### Rich Content Capture (`rich-content` feature)

`capture_rich(...)` preserves backward compatibility by keeping the plain capture engine as baseline,
then optionally attaching clipboard rich payloads.

- Rich payload sources (current):
  - macOS direct AX RTF (`AXRTFForRange`) when capture method is accessibility-based
  - Windows direct UIA text wrapped to minimal RTF when capture method is accessibility-based (`windows-beta`)
  - Linux direct AT-SPI text wrapped to minimal RTF when capture method is accessibility-based (`linux-alpha`)
  - Clipboard HTML/RTF fallback
- Guardrail: rich payload is accepted only when clipboard plain text matches captured plain text
- Fallback: if guard fails (or payload unavailable/oversized), result degrades to plain content

`CaptureRichOptions::allow_direct_accessibility_rich` controls the direct AX path and defaults to
`true`.
`CaptureRichOptions::conversion = Some(RichConversion::Markdown)` enables markdown normalization and
populates `RichPayload.markdown` (powered by `quick_html2md` and `rtf-to-html`).

```rust
#[cfg(feature = "rich-content")]
{
    use selection_capture::{capture_rich, CaptureRichOptions, CaptureRichOutcome};

    let rich_options = CaptureRichOptions::default();
    match capture_rich(&platform, &store, &cancel, &adapters, &rich_options) {
        CaptureRichOutcome::Success(ok) => println!("Captured: {:?}", ok.content),
        CaptureRichOutcome::Failure(err) => eprintln!("Capture failed: {:?}", err.status),
    }
}
```

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
    interleave_method_retries: true,
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
