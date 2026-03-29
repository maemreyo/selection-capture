# selection-capture v3

This crate is a standalone boundary for selected-text capture. It is designed to
stay extractable from Koe from day one.

## Locked decisions

- Core engine is synchronous.
- Caller owns threading and lifecycle.
- Cancellation is cooperative via `CancelSignal`.
- Retry waits use short polling intervals instead of one large blocking sleep.
- Clipboard cleanup is always attempted before returning.
- If `collect_trace` is true, every outcome contains a trace.
- App profile updates are merge-based, never blind replace.
- `CaptureOptions.strategy_override` takes priority over adapter overrides.
- No TTL in v1.

## Public model

- `capture(...) -> CaptureOutcome`
- `CaptureOptions::default()` is the recommended general-use policy
- `CaptureOutcome` is either `Success` or `Failure`
- `CaptureFailure` includes `cleanup_failed`
- `CaptureFailureContext` contains:
  - `status`
  - `active_app`
  - `methods_tried`
  - `last_method`

## v1 scope

- Public types and trait seams
- Sync engine skeleton
- Polling wait helper
- Retry-budget handling
- Cleanup invariant handling

Platform-specific macOS capture strategies are intentionally left for follow-up.
