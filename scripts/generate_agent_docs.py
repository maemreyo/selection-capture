#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re
import json
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "Cargo.toml"
AGENTS_MD = ROOT / "AGENTS.md"
LLMS_TXT = ROOT / "llms.txt"


def load_cargo() -> dict:
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    meta = json.loads(proc.stdout)
    root_manifest = str(CARGO_TOML)
    for package in meta["packages"]:
        if package.get("manifest_path") == root_manifest:
            return package
    raise RuntimeError("Unable to locate root package metadata")


def read_docsrs_default_target() -> str:
    content = CARGO_TOML.read_text(encoding="utf-8")
    m = re.search(
        r'(?m)^\s*default-target\s*=\s*"([^"]+)"\s*$',
        content,
    )
    return m.group(1) if m else "x86_64-unknown-linux-gnu"


def github_links(repository: str) -> tuple[str, str]:
    match = re.match(r"https://github.com/([^/]+)/([^/]+?)(?:\\.git)?$", repository.strip())
    if not match:
        return repository, repository
    owner, repo = match.group(1), match.group(2)
    web = f"https://github.com/{owner}/{repo}"
    raw = f"https://raw.githubusercontent.com/{owner}/{repo}/main/AGENTS.md"
    return web, raw


def render_agents_md(package: dict) -> str:
    features: dict[str, list[str]] = package.get("features", {})
    repository = package.get("repository", "")
    homepage = package.get("homepage", repository)
    web_repo, raw_agents = github_links(repository)
    docs_target = read_docsrs_default_target()

    feature_names = [name for name in features.keys() if name != "default"]
    feature_names.sort()

    feature_lines = "\n".join(f"- `{name}`" for name in feature_names) or "- _none_"

    return f"""# AGENTS.md

Machine-oriented operating guide for AI agents working with this repository.

## Canonical Links
- Repo: {web_repo}
- Crate: https://crates.io/crates/{package["name"]}
- API Docs: https://docs.rs/{package["name"]}
- Raw guide URL for agents: {raw_agents}

## Project Snapshot
- Crate: `{package["name"]}`
- Version: `{package["version"]}`
- Rust edition: `{package["edition"]}`
- Rust version: `{package.get("rust_version", "unspecified")}`
- Default docs.rs target: `{docs_target}`

## Features
- `default` (empty)
{feature_lines}

## Setup
```bash
cargo build
```

## Validation Commands (must pass before claiming done)
```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose
cargo test --features windows-beta --lib --verbose
cargo test --features linux-alpha --lib --verbose
cargo test --features rich-content --lib --verbose
```

## Release Commands
```bash
cargo release patch --execute --no-confirm
```

## Key API Entrypoints
- `capture(...)` and `try_capture(...)`
- `capture_async(...)` (feature: `async`)
- `capture_rich(...)` and `try_capture_rich(...)` (feature: `rich-content`)

## Safety Rules for Agents
- Do not run destructive git commands (`reset --hard`, history rewrites) unless explicitly requested.
- Keep changes minimal and scoped; preserve existing behavior.
- Always run validation commands before final output.
- If CI failures reference feature-specific targets, reproduce with the same feature set locally.

## Docs Drift Policy
- `AGENTS.md` and `llms.txt` are generated from `Cargo.toml`.
- If crate metadata/features/version change, regenerate docs:
```bash
python3 scripts/generate_agent_docs.py
```
- CI/local check command:
```bash
python3 scripts/check_agent_docs_sync.py
```
"""


def render_llms_txt(package: dict) -> str:
    features: dict[str, list[str]] = package.get("features", {})
    repository = package.get("repository", "")
    web_repo, raw_agents = github_links(repository)
    feature_names = [name for name in features.keys() if name != "default"]
    feature_names.sort()
    features_inline = ", ".join(feature_names) if feature_names else "none"

    return f"""# llms.txt
project: {package["name"]}
version: {package["version"]}
repo: {web_repo}
docs: https://docs.rs/{package["name"]}
crate: https://crates.io/crates/{package["name"]}
agent_guide: {raw_agents}
features: default(empty), {features_inline}
verify: cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --verbose
"""


def write_if_changed(path: pathlib.Path, content: str) -> bool:
    normalized = content.rstrip() + "\n"
    current = path.read_text(encoding="utf-8") if path.exists() else ""
    if current == normalized:
        return False
    path.write_text(normalized, encoding="utf-8")
    return True


def main() -> None:
    package = load_cargo()
    changed_agents = write_if_changed(AGENTS_MD, render_agents_md(package))
    changed_llms = write_if_changed(LLMS_TXT, render_llms_txt(package))
    changed = []
    if changed_agents:
        changed.append("AGENTS.md")
    if changed_llms:
        changed.append("llms.txt")
    if changed:
        print("Updated:", ", ".join(changed))
    else:
        print("No changes.")


if __name__ == "__main__":
    main()
