#!/usr/bin/env python3
import json
import pathlib
import sys

REQUIRED_BENCHES = {
    "capture_success_primary": 5_000.0,  # 5 µs
    "capture_fallback_to_clipboard": 8_000.0,  # 8 µs
    "capture_interleaved_retry_schedule": 8_000.0,  # 8 µs
    "capture_sequential_retry_schedule": 8_000_000.0,  # 8 ms
}


def load_mean_ns(path: pathlib.Path) -> float:
    data = json.loads(path.read_text())
    try:
        return float(data["mean"]["point_estimate"])
    except (KeyError, TypeError, ValueError) as exc:
        raise RuntimeError(f"invalid criterion estimate format: {path}") from exc


def main() -> int:
    base = pathlib.Path("target/criterion")
    if not base.exists():
        print("target/criterion not found. Run benchmark first.", file=sys.stderr)
        return 2

    failed = False
    for bench, threshold in REQUIRED_BENCHES.items():
        estimate_path = base / bench / "new" / "estimates.json"
        if not estimate_path.exists():
            print(f"missing benchmark output: {estimate_path}", file=sys.stderr)
            failed = True
            continue

        mean_ns = load_mean_ns(estimate_path)
        status = "PASS" if mean_ns <= threshold else "FAIL"
        print(
            f"{status} {bench}: mean={mean_ns:.2f}ns threshold={threshold:.2f}ns"
        )
        if mean_ns > threshold:
            failed = True

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
