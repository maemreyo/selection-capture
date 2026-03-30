use crate::engine::{capture, try_capture};
#[cfg(all(feature = "linux-alpha", target_os = "linux"))]
use crate::linux::try_selected_rtf_by_atspi;
#[cfg(target_os = "macos")]
use crate::macos::try_selected_rtf_by_ax;
use crate::rich_clipboard::{RichClipboardPayload, RichClipboardReader, SystemRichClipboardReader};
use crate::rich_convert::convert_to_markdown;
use crate::rich_types::{
    CaptureRichOptions, CaptureRichOutcome, CaptureRichSuccess, CapturedContent, ContentMetadata,
    RichConversion, RichPayload, RichSource,
};
use crate::traits::{AppAdapter, AppProfileStore, CancelSignal, CapturePlatform};
use crate::types::{ActiveApp, CaptureOutcome, CaptureTrace, TraceEvent, WouldBlock};
#[cfg(all(feature = "windows-beta", target_os = "windows"))]
use crate::windows::try_selected_rtf_by_uia;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn capture_rich(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
) -> CaptureRichOutcome {
    let reader = SystemRichClipboardReader;
    capture_rich_with_reader(platform, store, cancel, adapters, options, &reader)
}

pub fn try_capture_rich(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
) -> Result<CaptureRichOutcome, WouldBlock> {
    let reader = SystemRichClipboardReader;
    try_capture_rich_with_reader(platform, store, cancel, adapters, options, &reader)
}

fn capture_rich_with_reader(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
    reader: &impl RichClipboardReader,
) -> CaptureRichOutcome {
    capture_rich_with_reader_and_direct_reader(
        platform,
        store,
        cancel,
        adapters,
        options,
        reader,
        &read_direct_rtf_for_current_selection,
    )
}

fn capture_rich_with_reader_and_direct_reader(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
    reader: &impl RichClipboardReader,
    direct_rtf_reader: &impl Fn() -> Option<String>,
) -> CaptureRichOutcome {
    let outcome = capture(platform, store, cancel, adapters, &options.base);
    enrich_capture_outcome(platform, outcome, options, reader, direct_rtf_reader)
}

fn try_capture_rich_with_reader(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
    reader: &impl RichClipboardReader,
) -> Result<CaptureRichOutcome, WouldBlock> {
    try_capture_rich_with_reader_and_direct_reader(
        platform,
        store,
        cancel,
        adapters,
        options,
        reader,
        &read_direct_rtf_for_current_selection,
    )
}

fn try_capture_rich_with_reader_and_direct_reader(
    platform: &impl CapturePlatform,
    store: &impl AppProfileStore,
    cancel: &impl CancelSignal,
    adapters: &[&dyn AppAdapter],
    options: &CaptureRichOptions,
    reader: &impl RichClipboardReader,
    direct_rtf_reader: &impl Fn() -> Option<String>,
) -> Result<CaptureRichOutcome, WouldBlock> {
    let outcome = try_capture(platform, store, cancel, adapters, &options.base)?;
    Ok(enrich_capture_outcome(
        platform,
        outcome,
        options,
        reader,
        direct_rtf_reader,
    ))
}

fn enrich_capture_outcome(
    platform: &impl CapturePlatform,
    outcome: CaptureOutcome,
    options: &CaptureRichOptions,
    reader: &impl RichClipboardReader,
    _direct_rtf_reader: &impl Fn() -> Option<String>,
) -> CaptureRichOutcome {
    match outcome {
        CaptureOutcome::Failure(failure) => CaptureRichOutcome::Failure(failure),
        CaptureOutcome::Success(success) => {
            let plain_text = success.text;

            #[cfg(target_os = "macos")]
            if options.allow_direct_accessibility_rich && success.method.is_ax() {
                if let Some(rtf) = _direct_rtf_reader() {
                    if rtf.len() <= options.max_rich_payload_bytes {
                        let markdown = maybe_convert_to_markdown(
                            options,
                            None,
                            Some(rtf.as_str()),
                            &plain_text,
                        );
                        let metadata = ContentMetadata {
                            active_app: detect_active_app(success.trace.as_ref())
                                .or_else(|| platform.active_app()),
                            method: success.method,
                            source: RichSource::AccessibilityAttributed,
                            captured_at_unix_ms: unix_epoch_millis(),
                            plain_text_hash: hash_text(&plain_text),
                        };

                        return CaptureRichOutcome::Success(CaptureRichSuccess {
                            content: CapturedContent::Rich(RichPayload {
                                plain_text: plain_text.clone(),
                                html: None,
                                rtf: Some(rtf),
                                markdown,
                                metadata,
                            }),
                            method: success.method,
                            trace: success.trace,
                        });
                    }
                }
            }

            if !options.prefer_rich || !options.allow_clipboard_rich {
                return CaptureRichOutcome::Success(CaptureRichSuccess {
                    content: CapturedContent::Plain(plain_text),
                    method: success.method,
                    trace: success.trace,
                });
            }

            let Some(payload) = reader.read() else {
                return CaptureRichOutcome::Success(CaptureRichSuccess {
                    content: CapturedContent::Plain(plain_text),
                    method: success.method,
                    trace: success.trace,
                });
            };

            if exceeds_payload_limit(&payload, options.max_rich_payload_bytes) {
                return CaptureRichOutcome::Success(CaptureRichSuccess {
                    content: CapturedContent::Plain(plain_text),
                    method: success.method,
                    trace: success.trace,
                });
            }

            if options.require_plain_text_match
                && !clipboard_plain_text_matches(payload.plain_text.as_deref(), &plain_text)
            {
                return CaptureRichOutcome::Success(CaptureRichSuccess {
                    content: CapturedContent::Plain(plain_text),
                    method: success.method,
                    trace: success.trace,
                });
            }

            let source = if payload.html.is_some() {
                RichSource::ClipboardHtml
            } else if payload.rtf.is_some() {
                RichSource::ClipboardRtf
            } else {
                return CaptureRichOutcome::Success(CaptureRichSuccess {
                    content: CapturedContent::Plain(plain_text),
                    method: success.method,
                    trace: success.trace,
                });
            };

            let metadata = ContentMetadata {
                active_app: detect_active_app(success.trace.as_ref())
                    .or_else(|| platform.active_app()),
                method: success.method,
                source,
                captured_at_unix_ms: unix_epoch_millis(),
                plain_text_hash: hash_text(&plain_text),
            };

            let html = payload.html;
            let rtf = payload.rtf;
            let markdown =
                maybe_convert_to_markdown(options, html.as_deref(), rtf.as_deref(), &plain_text);

            let rich_payload = RichPayload {
                plain_text: plain_text.clone(),
                html,
                rtf,
                markdown,
                metadata,
            };

            CaptureRichOutcome::Success(CaptureRichSuccess {
                content: CapturedContent::Rich(rich_payload),
                method: success.method,
                trace: success.trace,
            })
        }
    }
}

#[cfg(target_os = "macos")]
fn read_direct_rtf_for_current_selection() -> Option<String> {
    try_selected_rtf_by_ax()
}

#[cfg(all(feature = "windows-beta", target_os = "windows"))]
fn read_direct_rtf_for_current_selection() -> Option<String> {
    try_selected_rtf_by_uia()
}

#[cfg(all(feature = "linux-alpha", target_os = "linux"))]
fn read_direct_rtf_for_current_selection() -> Option<String> {
    try_selected_rtf_by_atspi()
}

#[cfg(not(any(
    target_os = "macos",
    all(feature = "windows-beta", target_os = "windows"),
    all(feature = "linux-alpha", target_os = "linux")
)))]
fn read_direct_rtf_for_current_selection() -> Option<String> {
    None
}

fn maybe_convert_to_markdown(
    options: &CaptureRichOptions,
    html: Option<&str>,
    rtf: Option<&str>,
    plain_text: &str,
) -> Option<String> {
    match options.conversion {
        Some(RichConversion::Markdown) => convert_to_markdown(html, rtf, plain_text),
        None => None,
    }
}

fn exceeds_payload_limit(payload: &RichClipboardPayload, max_bytes: usize) -> bool {
    payload
        .html
        .as_ref()
        .is_some_and(|value| value.len() > max_bytes)
        || payload
            .rtf
            .as_ref()
            .is_some_and(|value| value.len() > max_bytes)
}

fn clipboard_plain_text_matches(clipboard_text: Option<&str>, plain_text: &str) -> bool {
    let Some(clipboard_text) = clipboard_text else {
        return false;
    };
    normalize_for_match(clipboard_text) == normalize_for_match(plain_text)
}

fn normalize_for_match(input: &str) -> String {
    let normalized_line_endings = input.replace("\r\n", "\n").replace('\r', "\n");
    normalized_line_endings.trim_end_matches('\n').to_string()
}

fn detect_active_app(trace: Option<&CaptureTrace>) -> Option<ActiveApp> {
    trace.and_then(|trace| {
        trace.events.iter().find_map(|event| match event {
            TraceEvent::ActiveAppDetected(app) => Some(app.clone()),
            _ => None,
        })
    })
}

fn unix_epoch_millis() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis(),
        Err(_) => 0,
    }
}

/// FNV-1a 64-bit hash — deterministic and stable across process restarts and Rust versions.
/// Unlike `DefaultHasher`, this produces identical output for the same input in every run,
/// making it safe for deduplication, change-detection, and persistent comparison.
fn hash_text(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
#[path = "rich_engine_tests.rs"]
mod tests;
