# Linux Alpha MVP

## Status

Linux support is currently a bounded alpha scaffold behind the `linux-alpha` feature flag.

What exists today:

- `LinuxPlatform` is available when `linux-alpha` is enabled.
- Capture methods dispatch through explicit Linux-oriented attempt paths:
  - AT-SPI
  - X11 selection
  - Clipboard
- Clipboard and primary-selection paths attempt real reads through host tooling:
  - Wayland: `wl-paste`
  - X11: `xclip` / `xsel`
- Active app detection is available through `xdotool` (active window PID) plus `ps`/`readlink`.
- AT-SPI primary path attempts focused-descendant text reads over the accessibility bus
  (`org.a11y.Bus` + `org.a11y.atspi.*` via `gdbus`).
- Dispatch behavior and engine-level fallback behavior are covered by unit and smoke tests.
- Linux command fallback now prioritizes Wayland/X11 tools based on session environment
  (`WAYLAND_DISPLAY`, `DISPLAY`) while still retaining mixed-session fallback behavior.
- `LinuxSelectionMonitor` supports polling (`LinuxMonitorBackend::Polling`) and a
  native-event-preferred scaffold mode (`LinuxMonitorBackend::NativeEventPreferred`) with bounded
  queueing, lifecycle-managed observer bridge acquire/release, and an optional event pump
  callback (`LinuxNativeEventPump`) that defaults to bridge-drain ingestion.
- `LinuxObserverLifecycleHook` can be registered to bind/unbind external native subscriber
  runtimes when observer bridge state transitions active/inactive.
- `set_linux_native_runtime_adapter(...)` allows wiring a concrete native attach/detach
  implementation while `linux_native_subscriber_stats()` tracks adapter attempts/failures.
- `linux_default_runtime_adapter_state()` exposes default adapter lifecycle state
  (`attached`, `worker_running`, `attach_calls`, `detach_calls`, `listener_exits`,
  `listener_restarts`, `listener_failures`) for diagnostic assertions during staged native
  rollout.
- `set_linux_default_runtime_event_source(...)` allows wiring a runtime event source used when the
  default Linux listener emits AT-SPI signal notifications.
- Default runtime adapter now runs an OS event listener process (`dbus-monitor` over the a11y bus
  via Python bootstrap) and pushes bridge events on signal boundaries, instead of timer polling.
- Listener attach/restart now uses bounded exponential backoff for transient startup failures and
  unexpected listener exits.
- Default runtime adapter installation auto-registers an AT-SPI-backed source hook when no custom
  event source is already configured.

## Setup

Enable the feature in `Cargo.toml`:

```toml
[dependencies]
selection-capture = { version = "0.1", features = ["linux-alpha"] }
```

Run the current alpha test surface:

```bash
cargo test --features linux-alpha --lib --verbose
cargo test --features linux-alpha --test linux_smoke --verbose
```

CI also runs these commands on `ubuntu-latest`.

## Scope

This MVP is intentionally narrow:

- Prove compile safety on Linux and non-Linux hosts with the feature enabled
- Prove feature-gated export and CI coverage
- Prove capture-engine fallback semantics with Linux-oriented smoke tests
- Keep the public dispatch surface aligned with planned Linux backends
- Limit "real capture" to shell-backed alpha paths while AT-SPI is unfinished

## Dispatch Mapping

Current `CaptureMethod` mapping in `LinuxPlatform`:

- `AccessibilityPrimary` -> AT-SPI attempt
- `AccessibilityRange` -> X11 selection attempt
- `ClipboardBorrow` -> Clipboard attempt
- `SyntheticCopy` -> Clipboard attempt

## Desktop Matrix Hardening

`linux_smoke` includes a desktop-oriented compatibility matrix to keep fallback paths deterministic
across representative Linux contexts:

- GNOME-like path: `AccessibilityPrimary`
- KDE-like path: `AccessibilityPrimary -> AccessibilityRange`
- Wayland compositor-like path: `AccessibilityPrimary -> AccessibilityRange -> ClipboardBorrow`
- X11-like override path: `AccessibilityPrimary -> SyntheticCopy`

In addition, backend command plans are session-aware:

- Wayland-only sessions prioritize `wl-paste` variants
- X11-only sessions prioritize `xclip`/`xsel`
- Mixed/unknown sessions keep multi-tool fallback ordering

## Known Limitations

- Clipboard/primary-selection capture depends on host tools (`wl-paste`, `xclip`, `xsel`) being
  installed and reachable in `PATH`.
- AT-SPI capture depends on `python3` and `gdbus`, and only works when the focused object exposes
  usable `Text`/`Accessible` data on the a11y bus.
- Active app detection depends on `xdotool`, `ps`, and `/proc` (`readlink`) and may fail on
  restricted Wayland sessions or minimal desktop environments.
- `AccessibilityRange` is mapped to primary-selection reads and may be unavailable on restricted
  Wayland/X11 sessions.
- `SyntheticCopy` currently shares the clipboard dispatch slot (no Linux key-synthesis path yet).
- Native-event-preferred monitor mode now supports default process-based AT-SPI signal push, but
  direct in-process AT-SPI listener bindings are not yet implemented.
- There is no Linux-specific cleanup behavior yet.
- There is no Linux permission, accessibility bus, display-server, or capability detection yet.
- The alpha surface is suitable for engine integration and limited local capture experiments.
