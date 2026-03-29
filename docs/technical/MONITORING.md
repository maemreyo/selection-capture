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
- Native bridge lifecycle now uses acquire/release semantics so multiple monitor instances can
  coexist without tearing down each other; bridge usage is released automatically when a monitor
  is dropped.
- In native-preferred mode, monitor setup now attempts to register AX notifications
  (`AXSelectedTextChanged`, `AXFocusedUIElementChanged`) on the active app and pumps the runloop
  briefly per polling tick to ingest callback-driven updates into the same bounded queue path.
- During polling, native runtime now re-checks focused app PID and re-registers observer runtime
  when focus migrates to a different process.
- `native_observer_stats()` exposes lightweight lifecycle counters (attach attempts/success/fail
  and skipped same-PID retries) for production diagnostics.
- `WindowsSelectionMonitor` (`windows-beta`) provides a Windows polling backend with
  de-duplication using UI Automation/Legacy IAccessible selection reads, plus a
  native-event-preferred scaffold mode (`WindowsMonitorBackend::NativeEventPreferred`) with a
  bounded native queue, lifecycle-managed observer bridge acquire/release, and an optional event
  pump hook (`WindowsNativeEventPump`) that defaults to bridge-drain ingestion.
  Bridge lifecycle hooks (`WindowsObserverLifecycleHook`) are available to attach/detach external
  native subscriber runtimes at start/stop boundaries.
  A lightweight subscriber manager scaffold is now wired by default in native-preferred monitor
  mode, exposing `windows_native_subscriber_stats()` for start/stop diagnostics and
  `set_windows_native_runtime_adapter(...)` for attach/detach runtime integration.
  Native-preferred monitor construction now auto-installs a default runtime adapter scaffold
  (`install_default_windows_runtime_adapter_if_absent()`) that binds a process-based Windows
  UI Automation focus-change listener and triggers source reads on native signals. The default
  adapter now tracks lifecycle state via
  `windows_default_runtime_adapter_state()` (`attached`, `worker_running`, `attach_calls`,
  `detach_calls`, `listener_exits`, `listener_restarts`, `listener_failures`) with idempotent
  attach/detach transitions and bounded retry/backoff for listener startup/restart. A pluggable
  runtime event source
  hook (`set_windows_default_runtime_event_source(...)`) resolves text on listener signal edges.
  By default, installer wiring now registers a UIA-backed source hook via existing
  focused-selection reads when no custom source is provided.
- `LinuxSelectionMonitor` (`linux-alpha`) provides a Linux polling backend with de-duplication
  using AT-SPI and primary-selection fallbacks, plus a native-event-preferred scaffold mode
  (`LinuxMonitorBackend::NativeEventPreferred`) with a bounded native queue, lifecycle-managed
  observer bridge acquire/release, and an optional event pump hook (`LinuxNativeEventPump`) that
  defaults to bridge-drain ingestion.
  Bridge lifecycle hooks (`LinuxObserverLifecycleHook`) are available to attach/detach external
  native subscriber runtimes at start/stop boundaries.
  A lightweight subscriber manager scaffold is now wired by default in native-preferred monitor
  mode, exposing `linux_native_subscriber_stats()` for start/stop diagnostics and
  `set_linux_native_runtime_adapter(...)` for attach/detach runtime integration.
  Native-preferred monitor construction now auto-installs a default runtime adapter scaffold
  (`install_default_linux_runtime_adapter_if_absent()`) that binds a process-based AT-SPI signal
  listener (`dbus-monitor` over the accessibility bus) and triggers source reads on native
  signals. The default adapter now tracks lifecycle state via
  `linux_default_runtime_adapter_state()`
  (`attached`, `worker_running`, `attach_calls`, `detach_calls`, `listener_exits`,
  `listener_restarts`, `listener_failures`) with idempotent attach/detach transitions and bounded
  retry/backoff for listener startup/restart. A pluggable runtime event source hook
  (`set_linux_default_runtime_event_source(...)`) resolves text on listener signal edges. By
  default, installer wiring now registers an
  AT-SPI-backed source hook via existing focused-selection reads when no custom source is
  provided.
- Integration coverage exists for ordered event delivery and monitor loop behavior through a stub
  backend.
- Feature-gated monitoring parity tests now validate that pump-fed native queues and manual
  queue-fed fallback paths emit equivalent event streams for Windows/Linux monitor scaffolds.

What does not exist yet:

- Async streams, channels, or subscription orchestration
- Direct in-process native observer bindings (`IUIAutomationEventHandler`, AT-SPI client listeners)
  beyond the current process-listener integration
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
adaptation, or cancellation. Direct in-process subscriptions (`IUIAutomationEventHandler`,
AT-SPI client listeners) are still pending beyond the current process-listener integration.

## Known Limitations

- Event payloads are plain `String` values only
- There is no timestamp, source app, or method metadata
- The wrapper does not own background tasks or event subscriptions
- The API does not distinguish "no event yet" from "monitor exhausted"
- macOS native-preferred mode now includes minimal AXObserver notification wiring for the active
  app; broader observer lifecycle (e.g. app focus migration/re-registration) is still pending
- Coalescing mode intentionally drops events inside the emit interval window
- Guarded mode intentionally suppresses events based on configured duplicate/interval policy
- `stable_polls_required` drops transient/flicker updates until the same value is observed enough polls
