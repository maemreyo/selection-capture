# Cross-Platform Testing Strategy

## Goal

The project is developed from a shared Rust codebase, but feature-gated platform surfaces are
validated in CI on their native operating systems. The current bounded test matrix is:

- macOS default build and test surface
- Windows beta library and smoke tests behind `windows-beta`
- Linux alpha library and smoke tests behind `linux-alpha`

This keeps feature-gated platform work compile-safe while still exercising engine fallback behavior
in realistic OS-specific jobs.

## Local Commands

Default host tests:

```bash
cargo test --verbose
```

Windows beta validation:

```bash
cargo test --features windows-beta --lib --verbose
cargo test --features windows-beta --test windows_smoke --verbose
```

Linux alpha validation:

```bash
cargo test --features linux-alpha --lib --verbose
cargo test --features linux-alpha --test linux_smoke --verbose
```

When developing on macOS without Windows or Linux hardware, rely on native CI jobs for the
feature-gated platform surfaces. The smoke tests are intentionally backend-free, so they validate
dispatch and fallback semantics rather than real OS capture.

## CI Coverage

`/.github/workflows/ci.yml` currently runs:

- `cargo fmt --all -- --check` on Ubuntu
- `cargo clippy --all-targets --all-features -- -D warnings` on Ubuntu
- `cargo build --verbose` and `cargo test --verbose` on macOS
- `cargo test --features windows-beta --lib --verbose` on Windows
- `cargo test --features windows-beta --test windows_smoke --verbose` on Windows
- `cargo check --verbose` on Ubuntu
- `cargo test --features linux-alpha --lib --verbose` on Ubuntu
- `cargo test --features linux-alpha --test linux_smoke --verbose` on Ubuntu
- `cargo doc --no-deps --document-private-items` on Ubuntu

This split is deliberate:

- macOS covers the default crate surface
- Windows and Linux jobs cover feature-gated MVP scaffolds
- Ubuntu still keeps a plain `cargo check` job for generic Linux build validation outside the
  `linux-alpha` smoke path

## Smoke Test Design

Windows and Linux smoke tests follow the same pattern:

- use a stub `CapturePlatform`
- feed deterministic `PlatformAttemptResult` sequences into the engine
- verify fallback ordering across accessibility and clipboard methods
- verify `strategy_override` takes precedence over default method order
- on Windows, also validate an app-oriented compatibility matrix (Edge/Chrome/VS Code/Office-like
  capture paths)
- on Linux, also validate a desktop-oriented compatibility matrix (GNOME/KDE/Wayland/X11-like
  capture paths)

These tests do not try to exercise native APIs. Their purpose is to keep the engine contract stable
while native backend coverage remains partial and platform-dependent.

## Current Limitations

- CI only partially validates runtime integrations; it does not comprehensively verify UI
  Automation, AT-SPI, display-server permutations (X11/Wayland), or app-specific permission flows.
- Cross-platform smoke tests prove engine behavior, not end-user capture success.
- GUI and permission-specific behavior still requires future native backend work or manual platform
  validation.
