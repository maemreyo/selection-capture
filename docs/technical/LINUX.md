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
- AT-SPI path is still scaffolded and returns `PlatformAttemptResult::Unavailable`.
- Dispatch behavior and engine-level fallback behavior are covered by unit and smoke tests.

What does not exist yet:

- Real AT-SPI capture
- Real clipboard-backed synthetic copy flow

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

## Known Limitations

- Clipboard/primary-selection capture depends on host tools (`wl-paste`, `xclip`, `xsel`) being
  installed and reachable in `PATH`.
- Active app detection depends on `xdotool`, `ps`, and `/proc` (`readlink`) and may fail on
  restricted Wayland sessions or minimal desktop environments.
- `AccessibilityPrimary` (AT-SPI) still returns `Unavailable`.
- `AccessibilityRange` is mapped to primary-selection reads and may be unavailable on restricted
  Wayland/X11 sessions.
- `SyntheticCopy` does not yet synthesize key input; it shares the clipboard dispatch slot.
- There is no Linux-specific cleanup behavior yet.
- There is no Linux permission, accessibility bus, display-server, or capability detection yet.
- The alpha surface is suitable for engine integration and limited local capture experiments.
