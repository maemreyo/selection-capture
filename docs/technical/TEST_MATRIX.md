# Test Matrix

This project uses a small native CI matrix plus local smoke targets to validate the shared
capture engine and platform-specific scaffolds without overextending the workflow.

| Surface | Command or job | What it validates |
| --- | --- | --- |
| Default host crate surface | `cargo test --verbose` | Core library tests, integration tests such as `tests/shared_engine.rs`, and the default macOS-friendly build path. |
| Shared engine contract | `cargo test --test shared_engine --verbose` | Engine fallback ordering, unsupported-method propagation, and stub-platform dispatch semantics. |
| Windows beta smoke surface | `cargo test --features windows-beta --lib --verbose`, `cargo test --features windows-beta --test windows_smoke --verbose`, and `cargo test --features windows-beta --test monitoring --verbose` | Feature-gated Windows exports, Windows-oriented smoke tests, and monitoring parity tests for native-queue vs polling-fallback paths. |
| Linux alpha smoke surface | `cargo test --features linux-alpha --lib --verbose`, `cargo test --features linux-alpha --test linux_smoke --verbose`, and `cargo test --features linux-alpha --test monitoring --verbose` | Feature-gated Linux exports, Linux-oriented smoke tests, and monitoring parity tests for native-queue vs polling-fallback paths. |
| Phase 2 scheduling benchmark | `cargo bench --bench capture_latency -- --noplot` | Baseline latency for primary success/fallback and interleaved-vs-sequential retry scheduling behavior. |
| Benchmark regression guard | `make bench-regression` | Runs capture latency benchmark plus threshold validation script (`scripts/check_bench_regression.py`). |
| Local CI alias | `make ci` | Formatting, Clippy, and the default host test surface. |
| Windows smoke alias | `make windows-beta-smoke` | Convenience wrapper for the Windows beta smoke test command. |
| Linux smoke alias | `make linux-alpha-smoke` | Convenience wrapper for the Linux alpha smoke test command. |
| GitHub Actions macOS job | `.github/workflows/ci.yml` `test-macos` | Default crate build and test surface on macOS, including shared integration tests. |
| GitHub Actions Windows job | `.github/workflows/ci.yml` `test-windows-beta` | Windows beta library and smoke validation. |
| GitHub Actions Linux job | `.github/workflows/ci.yml` `test-linux-alpha` plus `build-linux` | Linux alpha library and smoke validation, plus a plain `cargo check` job for general Linux build coverage. |
| GitHub Actions perf guard job | `.github/workflows/ci.yml` `perf-regression` | Benchmark regression thresholds for capture scheduling performance. |

The matrix is intentionally explicit. The default host job keeps the core crate honest, while the
Windows and Linux jobs stay focused on the feature-gated surfaces that need native runners.
