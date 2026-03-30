# Phase 3.3 Rich Content Capture Specification

**Goal:** Add rich-content capture (RTF/HTML + metadata) without breaking the existing plain-text API.

**Architecture:** Keep `capture()` unchanged and introduce additive rich APIs/types. Implement rich extraction as additive orchestration around the plain engine: direct accessibility-rich path first, then clipboard rich extraction with strict consistency guards, then plain fallback.

**Tech Stack:** Rust, existing `selection-capture` core engine, optional `clipboard-rs` (feature-gated), existing trace/monitoring types.

---

## 1. Context and Problem Statement

Current output is plain text only:
- `CaptureSuccess { text: String, method, focused_window_frame: Option<CGRect>, trace }`
- `PlatformAttemptResult::Success(String)`

This is stable and should not be broken.  
Phase 3.3 needs richer output for use cases like:
- Preserve formatting (RTF/HTML)
- Keep links/citation context
- Better downstream conversion (Markdown, note-taking, AI context ingest)

Reference input: `docs/plans/rich-content-research-verdict.md`.

---

## 2. Product Scope

## 2.1 In Scope (Phase 3.3 initial delivery)
- Add new rich-capture API surface (additive only).
- Rich extraction pipeline:
  - macOS direct selected-range RTF when available.
  - Windows UIA and Linux AT-SPI direct-text baselines wrapped into minimal RTF.
  - Clipboard HTML/RTF fallback.
- Metadata envelope for capture context.
- Consistency guard to avoid returning stale clipboard rich payloads.
- Optional Markdown normalization from rich/plain payloads.
- Test coverage for fallback chain and consistency behavior.
- Documentation for new API and behavior contract.

## 2.2 Out of Scope (defer)
- Deep attributed runs on Windows/Linux beyond plain-text-wrapped RTF baselines.
- Syntax highlighting and semantic code tokenization.
- Persistent storage/indexing of captured rich payloads.
- Plugin system and custom converters.

---

## 3. Compatibility and Versioning Requirements

- `capture()` behavior must remain unchanged.
- Existing public types in `src/types.rs` must remain source-compatible.
- Rich capture must be opt-in via new API:
  - `capture_rich(...)`
  - `try_capture_rich(...)`
- New dependencies must be behind a Cargo feature:
  - Proposed feature: `rich-content`

No breaking changes to existing callers in this phase.

---

## 4. API Specification (Proposed)

## 4.1 New Types

```rust
pub enum RichFormat {
    Html,
    Rtf,
}

pub enum RichSource {
    ClipboardHtml,
    ClipboardRtf,
    AccessibilityAttributed, // reserved for future phases
}

pub struct ContentMetadata {
    pub active_app: Option<ActiveApp>,
    pub method: CaptureMethod,
    pub source: RichSource,
    pub captured_at_unix_ms: u128,
    pub plain_text_hash: u64,
}

pub struct RichPayload {
    pub plain_text: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub markdown: Option<String>,
    pub metadata: ContentMetadata,
}

pub enum CapturedContent {
    Plain(String),
    Rich(RichPayload),
}

pub struct CaptureRichSuccess {
    pub content: CapturedContent,
    pub method: CaptureMethod,
    pub trace: Option<CaptureTrace>,
}

pub enum CaptureRichOutcome {
    Success(CaptureRichSuccess),
    Failure(CaptureFailure),
}
```

## 4.2 Options

```rust
pub struct CaptureRichOptions {
    pub base: CaptureOptions,
    pub prefer_rich: bool,              // default: true
    pub allow_clipboard_rich: bool,     // default: true
    pub allow_direct_accessibility_rich: bool, // default: true
    pub conversion: Option<RichConversion>, // default: None
    pub max_rich_payload_bytes: usize,  // default: 262_144 (256 KiB)
    pub require_plain_text_match: bool, // default: true
}
```

## 4.3 New Functions

```rust
pub fn capture_rich<P, S, C>(
    platform: &P,
    store: &S,
    cancel: &C,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
) -> CaptureRichOutcome
where
    P: CapturePlatform;

pub fn try_capture_rich<P, S, C>(
    platform: &P,
    store: &S,
    cancel: &C,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
) -> Result<CaptureRichOutcome, WouldBlock>
where
    P: CapturePlatform;
```

---

## 5. Functional Behavior

## 5.1 Primary Flow (Direct-first on macOS, then clipboard)
1. Run existing plain-text engine (`capture` / `try_capture`) to obtain baseline plain text + method.
2. If plain result is `Failure`, return `Failure` unchanged.
3. If rich is disabled by options, return `CapturedContent::Plain`.
4. If enabled and method is accessibility-based, attempt direct platform rich extraction:
   - macOS: AX selected-range RTF.
   - Windows/Linux: direct accessibility text wrapped to minimal RTF baseline.
5. If no direct rich payload is available, attempt clipboard rich read (`HTML`, then `RTF`) if enabled.
6. Validate clipboard consistency:
   - Clipboard plain text must equal captured plain text (normalized compare), OR
   - hash match if exact compare disabled in future.
7. If valid rich format found, optionally run conversion pipeline (Markdown normalization).
8. If valid rich format found, return `CapturedContent::Rich`.
9. Else return `CapturedContent::Plain`.

## 5.2 Consistency Guard (Required)

Problem: clipboard may hold stale content unrelated to current selection.

Guard rule (v1):
- Only accept rich payload when clipboard plain text matches the baseline plain text from capture.

Normalization for comparison (v1):
- Trim trailing `\r\n`/`\n`
- Normalize line endings to `\n`
- Preserve internal whitespace (no aggressive normalization)

## 5.3 Size and Safety Limits

- If `html` or `rtf` exceeds `max_rich_payload_bytes`, discard rich payload and fallback to plain.
- No disk persistence in this phase.
- No network I/O.

---

## 6. Internal Design and File-Level Plan

## 6.1 New Files
- `src/rich_types.rs`
- `src/rich_engine.rs`
- `src/rich_clipboard.rs`
- `src/rich_convert.rs`

## 6.2 Modified Files
- `src/lib.rs` (exports + feature-gated API)
- `Cargo.toml` (feature + optional dependencies)
- `docs/technical/ROADMAP_2026_2027.md` (phase status update after implementation)
- `README.md` (new API examples)

## 6.3 Integration Strategy
- `rich_engine` calls existing `engine::capture` / `engine::try_capture` as baseline.
- `rich_clipboard` provides a small adapter trait to isolate `clipboard-rs` and allow deterministic tests.
- `rich_convert` uses crate-backed conversion (`quick_html2md`, `rtf-to-html`) instead of custom parsers.
- All rich logic remains additive and does not modify existing `PlatformAttemptResult`.

---

## 7. Error Handling and Trace Semantics

- Reuse existing `CaptureFailure` unchanged.
- Rich extraction failure must not upgrade a plain-text success into failure.
- Rich-specific failures are downgraded to `CapturedContent::Plain`.
- Trace:
  - Reuse baseline trace from plain capture.
  - Optional future extension: add rich-specific trace events (deferred for this phase).

---

## 8. Test Specification

## 8.1 Unit Tests (`src/rich_engine.rs`, `src/rich_convert.rs`)
- Returns `Rich` when clipboard HTML exists and plain-text match passes.
- Returns `Rich` when only RTF exists and plain-text match passes.
- Returns `Plain` when clipboard formats exist but plain-text mismatch.
- Returns `Plain` when rich payload exceeds max size.
- Returns `Plain` when rich disabled in options.
- Uses direct-rich path first when available, then clipboard fallback.
- Produces normalized markdown when conversion is enabled.
- Preserves baseline `Failure` unchanged.
- `try_capture_rich` mirrors `WouldBlock` behavior from baseline engine.

## 8.2 Compatibility Tests
- Existing `make ci` remains green.
- Existing windows/linux feature test matrices remain green:
  - `cargo test --features windows-beta --lib --verbose`
  - `cargo test --features linux-alpha --lib --verbose`

## 8.3 Non-Goals in Tests (defer)
- No benchmark gate yet (can be added later).

---

## 9. Documentation Requirements

- Add a dedicated section in README:
  - When to use `capture()` vs `capture_rich()`
  - Consistency guard behavior
  - Feature flag requirements (`rich-content`)
- Update roadmap Phase 3.3 checklist with delivered scope.
- Add platform notes:
  - Clipboard richness availability varies by app and desktop environment.

---

## 10. Rollout Plan

## Milestone A: API and scaffolding
- Add rich types/options/outcome + exports
- Add feature flag and compile gates
- No behavior change to existing API

## Milestone B: Clipboard-first rich extraction
- Implement `rich_clipboard` provider + consistency guard
- Implement `capture_rich` and `try_capture_rich`
- Add full test suite for fallback behavior

## Milestone C: Docs and stabilization
- README + roadmap updates
- Full matrix verification

---

## 11. Risks and Mitigations

- Risk: stale clipboard rich payload
  - Mitigation: strict plain-text match guard (required in v1).

- Risk: platform-specific format availability inconsistent
  - Mitigation: treat rich as best-effort; always keep plain fallback.

- Risk: payload size/memory spikes
  - Mitigation: configurable payload cap and hard fallback.

- Risk: cross-platform direct-rich parity quality differs
  - Mitigation: preserve plain-text baseline semantics and clearly label source in metadata.

---

## 12. Acceptance Criteria

- New additive API compiles behind `rich-content` feature.
- Existing API remains source-compatible and behaviorally unchanged.
- Direct-first rich capture path works where available, with clipboard consistency guard fallback.
- All existing CI checks pass.
- New rich tests pass with deterministic stubs.
- Documentation explains contracts and fallbacks clearly.

---

## 13. Next Phase Hooks (Post-3.3 initial)

- Improve Windows/Linux direct-rich quality beyond plain-text-wrapped RTF baselines.
- Add richer HTML-to-Markdown semantics (links/lists/code blocks/tables).
- Add configurable source-priority policies for app-specific preferences.

---
