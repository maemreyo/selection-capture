# Linux Alpha MVP

## Status

Linux support is currently a bounded alpha scaffold behind the `linux-alpha` feature flag.

What exists today:

- `LinuxPlatform` is available when `linux-alpha` is enabled.
- Capture methods dispatch through explicit Linux-oriented attempt paths:
  - AT-SPI
  - X11 selection
  - Clipboard
- The current backend safely returns `PlatformAttemptResult::Unavailable` until native
  implementations are added.
- Dispatch behavior and engine-level fallback behavior are covered by unit and smoke tests.

What does not exist yet:

- Real AT-SPI capture
- Real X11 primary-selection range capture
- Real clipboard-backed synthetic copy flow
- Active application discovery on Linux
- Wayland-specific selection plumbing

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
- Avoid shipping placeholder behavior that pretends to capture text

## Dispatch Mapping

Current `CaptureMethod` mapping in `LinuxPlatform`:

- `AccessibilityPrimary` -> AT-SPI attempt
- `AccessibilityRange` -> X11 selection attempt
- `ClipboardBorrow` -> Clipboard attempt
- `SyntheticCopy` -> Clipboard attempt

## Known Limitations

- All Linux attempt methods currently return `Unavailable`.
- `AccessibilityRange` is mapped to an X11-oriented slot; Wayland handling is not implemented.
- `SyntheticCopy` does not yet synthesize key input; it shares the clipboard dispatch slot.
- There is no Linux-specific cleanup behavior yet.
- There is no Linux permission, accessibility bus, display-server, or capability detection yet.
- The alpha surface is suitable for engine integration and CI validation, not end-user capture.
