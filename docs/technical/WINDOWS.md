# Windows Beta MVP

## Status

Windows support is currently a bounded beta scaffold behind the `windows-beta` feature flag.

What exists today:

- `WindowsPlatform` is available when `windows-beta` is enabled.
- Capture methods dispatch through explicit Windows-oriented attempt paths:
  - UI Automation
  - IAccessible
  - Clipboard
- Clipboard path reads real clipboard text via PowerShell (`Get-Clipboard -Raw`) on Windows.
- Active app detection is available on Windows via foreground-window process lookup.
- Focused window frame extraction is available via user32 `GetForegroundWindow` + `GetWindowRect`
  and is exposed through `CaptureSuccess.focused_window_frame`.
- UI Automation primary path uses PowerShell/.NET UIAutomation against the focused element
  (`TextPattern` first, `ValuePattern` fallback).
- IAccessible range path uses `LegacyIAccessiblePattern` (`Value` first, `Name` fallback).
- Synthetic copy path sends guarded `Ctrl+C`, reads clipboard, and restores prior clipboard text
  when prior text content exists.
- `WindowsSelectionMonitor` supports polling
  (`WindowsMonitorBackend::Polling`) and a native-event-preferred scaffold mode
  (`WindowsMonitorBackend::NativeEventPreferred`) with bounded queueing, lifecycle-managed
  observer bridge acquire/release, and an optional event pump callback
  (`WindowsNativeEventPump`) that defaults to bridge-drain ingestion.
- `WindowsObserverLifecycleHook` can be registered to bind/unbind external native subscriber
  runtimes when observer bridge state transitions active/inactive.
- `set_windows_native_runtime_adapter(...)` allows wiring a concrete native attach/detach
  implementation while `windows_native_subscriber_stats()` tracks adapter attempts/failures.
- `windows_default_runtime_adapter_state()` exposes default adapter lifecycle state
  (`attached`, `worker_running`, `attach_calls`, `detach_calls`, `listener_exits`,
  `listener_restarts`, `listener_failures`) for diagnostic assertions during staged native
  rollout.
- `set_windows_default_runtime_event_source(...)` allows wiring a runtime event source used when
  the default Windows listener emits native focus-change notifications.
- Default runtime adapter now runs an OS event listener process (PowerShell + UI Automation focus
  change hook) and pushes bridge events on signal boundaries, instead of polling on a fixed timer.
- Listener attach/restart now uses bounded exponential backoff for transient startup failures and
  unexpected listener exits.
- Default runtime adapter installation auto-registers a UIA-backed source hook when no custom
  event source is already configured.
- Fallback behavior is covered by unit and smoke tests.

## Setup

Enable the feature in `Cargo.toml`:

```toml
[dependencies]
selection-capture = { version = "0.1", features = ["windows-beta"] }
```

Run the current beta test surface:

```bash
cargo test --features windows-beta --lib --verbose
cargo test --features windows-beta --test windows_smoke --verbose
```

CI also runs these commands on `windows-latest`.

## Scope

This MVP is intentionally narrow:

- Prove compile safety on non-Windows hosts
- Prove feature-gated export and CI coverage
- Prove capture-engine fallback semantics with a Windows-oriented smoke test
- Avoid shipping placeholder behavior that pretends to capture text

## Dispatch Mapping

Current `CaptureMethod` mapping in `WindowsPlatform`:

- `AccessibilityPrimary` -> UI Automation attempt
- `AccessibilityRange` -> IAccessible attempt
- `ClipboardBorrow` -> Clipboard attempt
- `SyntheticCopy` -> Synthetic copy attempt (`Ctrl+C` + clipboard read + best-effort restore)

## Compatibility Smoke Matrix

`windows_smoke` now validates app-oriented fallback semantics for representative desktop targets:

- Edge-like path: `AccessibilityPrimary`
- Chrome-like path: `AccessibilityPrimary -> AccessibilityRange`
- VS Code-like path: `AccessibilityPrimary -> AccessibilityRange -> ClipboardBorrow`
- Office-like path: `AccessibilityPrimary -> SyntheticCopy` (with strategy override)

## Known Limitations

- Active app detection depends on PowerShell/user32 lookup and may fail in restricted sessions.
- Focused window frame extraction currently uses the foreground top-level window bounds and may not
  match inner control geometry in complex UI trees.
- UI Automation capture depends on the focused element exposing `TextPattern` or `ValuePattern`;
  many desktop apps still return empty values.
- Legacy IAccessible capture depends on `LegacyIAccessiblePattern`; many apps do not expose useful
  value/name content for selected text.
- Synthetic copy currently depends on foreground focus and SendKeys behavior from PowerShell STA.
- Native-event-preferred monitor mode now supports default process-based push signaling, but direct
  in-process `IUIAutomationEventHandler` bindings are not yet implemented.
- There is no Windows-specific cleanup behavior yet.
- There is no Windows permission or capability detection yet.
- The beta surface is suitable for engine integration and incremental backend rollout.
