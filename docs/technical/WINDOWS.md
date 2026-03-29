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
- UI Automation and IAccessible paths currently stay scaffolded and return `Unavailable`.
- Fallback behavior is covered by unit and smoke tests.

What does not exist yet:

- Real UI Automation capture
- Real IAccessible range capture
- Real synthetic copy flow (Ctrl+C + restore semantics)
- Active application discovery on Windows

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
- `SyntheticCopy` -> Clipboard attempt

## Known Limitations

- Clipboard capture currently reads existing clipboard content and does not synthesize copy.
- UI Automation and IAccessible attempts still return `Unavailable`.
- `SyntheticCopy` does not yet synthesize key input; it currently shares clipboard dispatch.
- There is no Windows-specific cleanup behavior yet.
- There is no Windows permission or capability detection yet.
- The beta surface is suitable for engine integration and incremental backend rollout.
