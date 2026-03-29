# Monitoring API Scaffold

## Status

Monitoring support is currently an experimental, backend-agnostic scaffold.

What exists today:

- `MonitorPlatform` defines the minimal backend contract for selection-change polling.
- `CaptureMonitor<P>` wraps a backend and exposes `next_event()`, `run()`, `run_with_limit()`,
  `collect_events()`, and `poll_until()` helpers for synchronous processing loops.
- `MacOSSelectionMonitor` provides a first-party macOS monitor backend (polling + de-duplication
  via AX selected-text reads).
- Integration coverage exists for ordered event delivery through a stub backend.

What does not exist yet:

- Built-in Windows monitoring hooks
- Built-in Linux monitoring hooks
- Async streams, channels, or subscription orchestration
- Permission handling, lifecycle management, or debounce/coalescing logic

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
- macOS implementation is polling-based, not callback-based (`AXObserver`) yet
