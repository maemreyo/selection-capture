# Test Matrix

This project uses a small native CI matrix plus local smoke targets to validate the shared
capture engine and platform-specific scaffolds without overextending the workflow.

| Surface | Command or job | What it validates |
| --- | --- | --- |
| Default host crate surface | `cargo test --verbose` | Core library tests, integration tests such as `tests/shared_engine.rs`, and the default macOS-friendly build path. |
| Shared engine contract | `cargo test --test shared_engine --verbose` | Engine fallback ordering, unsupported-method propagation, and stub-platform dispatch semantics. |
| Windows beta smoke surface | `cargo test --features windows-beta --lib --verbose` and `cargo test --features windows-beta --test windows_smoke --verbose` | Feature-gated Windows exports plus the Windows-oriented smoke test harness. |
| Linux alpha smoke surface | `cargo test --features linux-alpha --lib --verbose` and `cargo test --features linux-alpha --test linux_smoke --verbose` | Feature-gated Linux exports plus the Linux-oriented smoke test harness. |
| Local CI alias | `make ci` | Formatting, Clippy, and the default host test surface. |
| Windows smoke alias | `make windows-beta-smoke` | Convenience wrapper for the Windows beta smoke test command. |
| Linux smoke alias | `make linux-alpha-smoke` | Convenience wrapper for the Linux alpha smoke test command. |
| GitHub Actions macOS job | `.github/workflows/ci.yml` `test-macos` | Default crate build and test surface on macOS, including shared integration tests. |
| GitHub Actions Windows job | `.github/workflows/ci.yml` `test-windows-beta` | Windows beta library and smoke validation. |
| GitHub Actions Linux job | `.github/workflows/ci.yml` `test-linux-alpha` plus `build-linux` | Linux alpha library and smoke validation, plus a plain `cargo check` job for general Linux build coverage. |

The matrix is intentionally explicit. The default host job keeps the core crate honest, while the
Windows and Linux jobs stay focused on the feature-gated surfaces that need native runners.
