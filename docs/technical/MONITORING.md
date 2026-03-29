# Monitoring API Scaffold

## Status

Monitoring support is currently an experimental, backend-agnostic scaffold.

What exists today:

- `MonitorPlatform` defines the minimal backend contract for selection-change polling.
- `CaptureMonitor<P>` wraps a backend and exposes `next_event()`, `run()`, `run_with_limit()`,
  `collect_events()`, `poll_until()`, `poll_until_cancelled()`, and
  `poll_until_cancelled_coalesced()`, and `poll_until_cancelled_guarded()` helpers for
  synchronous processing loops.
- `MacOSSelectionMonitor` provides a first-party macOS monitor backend (polling + de-duplication
  via AX selected-text reads), and exposes a native-observer scaffold mode
  (`MacOSMonitorBackend::NativeObserverPreferred`) that currently uses a bounded native-event
  queue (tail de-dup + drop counter) with safe fallback to polling, plus callback ingress API
  (`ingest_native_observer_payload(...)`) and an optional native pump hook
  (`native_event_pump`).
- `AxObserverBridge` (macOS-only scaffold) provides a process-local bridge with active-state
  gating, bounded queueing, tail de-duplication, and drop metrics; monitor integration can use
  `ax_observer_drain_events_for_monitor()` as the `native_event_pump` callback, and
  `MacOSSelectionMonitor` now auto-wires this pump by default when native observer mode
  activates successfully.
- `WindowsSelectionMonitor` (`windows-beta`) provides a Windows polling backend with
  de-duplication using UI Automation/Legacy IAccessible selection reads.
- `LinuxSelectionMonitor` (`linux-alpha`) provides a Linux polling backend with de-duplication
  using AT-SPI and primary-selection fallbacks.
- Integration coverage exists for ordered event delivery and monitor loop behavior through a stub
  backend.

What does not exist yet:

- Async streams, channels, or subscription orchestration
- Native observer/event-subscription lifecycle management beyond bridge activation and pump wiring
- Debounce semantics (current coalescing support is interval-throttling)

## Scope

This scaffold is intentionally narrow:

- Establish a stable public API surface for future monitoring work
- Avoid coupling the API to any current OS backend detail
- Make backend experimentation possible without changing the library entry point

The current API is for crate structure and early integration, not production monitoring.

## Current API

```rust
pub trait MonitorPlatform {
    fn next_selection_change(&self) -> Option<String>;
}

pub struct CaptureMonitor<P> {
    // backend storage
}

impl<P: MonitorPlatform> CaptureMonitor<P> {
    pub fn next_event(&self) -> Option<String>;
    pub fn run<F: FnMut(String)>(&self, on_event: F) -> usize;
    pub fn run_with_limit<F: FnMut(String)>(&self, max_events: usize, on_event: F) -> usize;
    pub fn collect_events(&self, max_events: usize) -> Vec<String>;
    pub fn poll_until<F: FnMut(String), C: FnMut() -> bool>(
        &self,
        poll_interval: Duration,
        should_continue: C,
        on_event: F,
    ) -> usize;
    pub fn poll_until_cancelled<F: FnMut(String), S: CancelSignal>(
        &self,
        poll_interval: Duration,
        cancel: &S,
        on_event: F,
    ) -> usize;
    pub fn poll_until_cancelled_coalesced<F: FnMut(String), S: CancelSignal>(
        &self,
        poll_interval: Duration,
        min_emit_interval: Duration,
        cancel: &S,
        on_event: F,
    ) -> usize;
    pub fn poll_until_cancelled_guarded<F: FnMut(String), S: CancelSignal>(
        &self,
        poll_interval: Duration,
        cancel: &S,
        guard: &MonitorSpamGuard,
        on_event: F,
    ) -> usize;
    pub fn poll_until_cancelled_guarded_with_stats<F: FnMut(String), S: CancelSignal>(
        &self,
        poll_interval: Duration,
        cancel: &S,
        guard: &MonitorSpamGuard,
        on_event: F,
    ) -> MonitorGuardStats;
}

pub struct MonitorSpamGuard {
    pub suppress_identical: bool,
    pub min_emit_interval: Duration,
    pub min_emit_interval_same_text: Duration,
    pub normalize_whitespace: bool,
    pub stable_polls_required: usize,
}

pub struct MonitorGuardStats {
    pub emitted: u64,
    pub dropped_duplicate: u64,
    pub dropped_global_interval: u64,
    pub dropped_same_text_interval: u64,
    pub dropped_unstable: u64,
}
```

`next_selection_change()` is deliberately minimal. It models a synchronous poll for the next
selection update and returns `None` when no more event data is available.

## Intended Platform Hooks

Future backends are expected to plug into `MonitorPlatform` without changing the public wrapper:

- macOS: accessibility notifications, focused element observers, or clipboard-adjacent fallbacks
- Windows: UI Automation text pattern events or focused control change notifications
- Linux: AT-SPI event listeners and toolkit-specific selection change hooks

Those platform backends may later require richer event metadata, blocking behavior, async
adaptation, or cancellation. That work is intentionally deferred until native hooks exist.

## Known Limitations

- Event payloads are plain `String` values only
- There is no timestamp, source app, or method metadata
- The wrapper does not own background tasks or event subscriptions
- The API does not distinguish "no event yet" from "monitor exhausted"
- macOS native-preferred mode can ingest bridge-fed callback payloads, but full OS-level
  `AXObserver` runloop lifecycle wiring is still pending
- Coalescing mode intentionally drops events inside the emit interval window
- Guarded mode intentionally suppresses events based on configured duplicate/interval policy
- `stable_polls_required` drops transient/flicker updates until the same value is observed enough polls
