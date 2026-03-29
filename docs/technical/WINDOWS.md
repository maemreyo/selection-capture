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
- UI Automation primary path uses PowerShell/.NET UIAutomation against the focused element
  (`TextPattern` first, `ValuePattern` fallback).
- IAccessible range path uses `LegacyIAccessiblePattern` (`Value` first, `Name` fallback).
- Fallback behavior is covered by unit and smoke tests.

What does not exist yet:

- Real synthetic copy flow (Ctrl+C + restore semantics)

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
- Active app detection depends on PowerShell/user32 lookup and may fail in restricted sessions.
- UI Automation capture depends on the focused element exposing `TextPattern` or `ValuePattern`;
  many desktop apps still return empty values.
- Legacy IAccessible capture depends on `LegacyIAccessiblePattern`; many apps do not expose useful
  value/name content for selected text.
- `SyntheticCopy` does not yet synthesize key input; it currently shares clipboard dispatch.
- There is no Windows-specific cleanup behavior yet.
- There is no Windows permission or capability detection yet.
- The beta surface is suitable for engine integration and incremental backend rollout.
