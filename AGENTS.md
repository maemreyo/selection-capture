# AGENTS.md

Machine-oriented operating guide for AI agents working with this repository.

## Canonical Links
- Repo: https://github.com/maemreyo/selection-capture
- Crate: https://crates.io/crates/selection-capture
- API Docs: https://docs.rs/selection-capture
- Raw guide URL for agents: https://raw.githubusercontent.com/maemreyo/selection-capture/main/AGENTS.md

## Project Snapshot
- Crate: `selection-capture`
- Version: `0.1.4`
- Rust edition: `2021`
- Rust version: `1.75`
- Default docs.rs target: `x86_64-unknown-linux-gnu`

## Features
- `default` (empty)
- `async`
- `linux-alpha`
- `rich-content`
- `windows-beta`

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
