# Performance Baseline (Phase 2)

Date: 2026-03-29  
Scope: `capture_latency` Criterion benchmark (initial Phase 2 baseline)

## Command

```bash
cargo bench --bench capture_latency -- --noplot
```

## Environment Snapshot

- Host: macOS (Apple Silicon)
- Build profile: `bench` (optimized)
- Benchmark harness: Criterion (100 samples per case)

## Baseline Results

| Benchmark | Time (lower bound) | Time (median) | Time (upper bound) |
|---|---:|---:|---:|
| `capture_success_primary` | 742.15 ns | 747.00 ns | 751.88 ns |
| `capture_fallback_to_clipboard` | 1.0536 µs | 1.0570 µs | 1.0604 µs |
| `capture_interleaved_retry_schedule` | 755.53 ns | 759.80 ns | 763.87 ns |
| `capture_sequential_retry_schedule` | 2.4946 ms | 2.5047 ms | 2.5171 ms |

## Interpretation

- Interleaved scheduling is dramatically faster than sequential retry in the benchmarked scenario
  where `AccessibilityRange` can succeed immediately after an initial primary miss.
- These values are micro-bench references for engine scheduling behavior, not end-user UIA/AT-SPI
  real-world latency figures.

## Next Step

- CI regression gates are now wired via `scripts/check_bench_regression.py` and
  `.github/workflows/ci.yml` (`perf-regression` job); tune thresholds over time using additional
  runner data.
