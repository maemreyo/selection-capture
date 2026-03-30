#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]


def main() -> int:
    run = subprocess.run(
        ["python3", str(ROOT / "scripts" / "generate_agent_docs.py")],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if run.returncode != 0:
        print(run.stdout, end="")
        print(run.stderr, end="", file=sys.stderr)
        return run.returncode

    diff = subprocess.run(
        ["git", "diff", "--exit-code", "--", "AGENTS.md", "llms.txt"],
        cwd=ROOT,
        check=False,
    )
    if diff.returncode != 0:
        print(
            "Agent docs are out of date. Run: python3 scripts/generate_agent_docs.py",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
