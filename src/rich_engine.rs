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
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
    direct_rtf_reader: &impl Fn() -> Option<String>,
) -> CaptureRichOutcome {
    match outcome {
        CaptureOutcome::Failure(failure) => CaptureRichOutcome::Failure(failure),
        CaptureOutcome::Success(success) => {
            let plain_text = success.text;

            #[cfg(target_os = "macos")]
            if options.allow_direct_accessibility_rich && success.method.is_ax() {
                if let Some(rtf) = direct_rtf_reader() {
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

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{AppProfile, AppProfileUpdate};
    use crate::traits::CapturePlatform;
    use crate::types::{
        CaptureMethod, CaptureOptions, CleanupStatus, PlatformAttemptResult, RetryPolicy,
    };
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct NeverCancel;
    impl CancelSignal for NeverCancel {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    struct NoAdapters;
    impl AppAdapter for NoAdapters {
        fn matches(&self, _app: &ActiveApp) -> bool {
            false
        }
        fn strategy_override(&self, _app: &ActiveApp) -> Option<Vec<CaptureMethod>> {
            None
        }
        fn hint_override(
            &self,
            _context: &crate::types::CaptureFailureContext,
        ) -> Option<crate::types::UserHint> {
            None
        }
    }

    struct StubStore;
    impl AppProfileStore for StubStore {
        fn load(&self, app: &ActiveApp) -> AppProfile {
            AppProfile::unknown(app.bundle_id.clone())
        }
        fn merge_update(&self, _app: &ActiveApp, _update: AppProfileUpdate) {}
    }

    #[derive(Clone)]
    struct StubPlatform {
        app: Option<ActiveApp>,
        responses: Arc<Mutex<Vec<PlatformAttemptResult>>>,
        cleanup: CleanupStatus,
    }

    impl CapturePlatform for StubPlatform {
        fn active_app(&self) -> Option<ActiveApp> {
            self.app.clone()
        }

        fn attempt(
            &self,
            _method: CaptureMethod,
            _app: Option<&ActiveApp>,
        ) -> PlatformAttemptResult {
            let mut guard = self.responses.lock().expect("responses lock poisoned");
            if guard.is_empty() {
                PlatformAttemptResult::Unavailable
            } else {
                guard.remove(0)
            }
        }

        fn cleanup(&self) -> CleanupStatus {
            self.cleanup
        }
    }

    struct StubReader {
        payload: Option<RichClipboardPayload>,
    }

    impl RichClipboardReader for StubReader {
        fn read(&self) -> Option<RichClipboardPayload> {
            self.payload.clone()
        }
    }

    fn base_options() -> CaptureOptions {
        CaptureOptions {
            retry_policy: RetryPolicy {
                primary_accessibility: vec![Duration::ZERO],
                range_accessibility: vec![Duration::ZERO],
                clipboard: vec![Duration::ZERO],
                poll_interval: Duration::from_millis(20),
            },
            collect_trace: true,
            ..CaptureOptions::default()
        }
    }

    fn rich_options() -> CaptureRichOptions {
        CaptureRichOptions {
            base: base_options(),
            allow_direct_accessibility_rich: false,
            ..CaptureRichOptions::default()
        }
    }

    #[test]
    fn returns_rich_when_html_matches_plain_text() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: Some("<p>hello</p>".to_string()),
                rtf: None,
            }),
        };

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &rich_options(),
            &reader,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Rich(payload) => {
                    assert_eq!(payload.plain_text, "hello");
                    assert_eq!(payload.html.as_deref(), Some("<p>hello</p>"));
                    assert_eq!(payload.metadata.source, RichSource::ClipboardHtml);
                }
                CapturedContent::Plain(_) => panic!("expected rich content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn returns_rich_when_only_rtf_matches_plain_text() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: None,
                rtf: Some("{\\rtf1 hello}".to_string()),
            }),
        };

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &rich_options(),
            &reader,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Rich(payload) => {
                    assert_eq!(payload.rtf.as_deref(), Some("{\\rtf1 hello}"));
                    assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                }
                CapturedContent::Plain(_) => panic!("expected rich content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn populates_markdown_when_conversion_is_enabled() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello world".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello world".to_string()),
                html: Some("<p>hello<br>world</p>".to_string()),
                rtf: None,
            }),
        };
        let mut options = rich_options();
        options.conversion = Some(RichConversion::Markdown);

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Rich(payload) => {
                    assert_eq!(payload.markdown.as_deref(), Some("hello\nworld"));
                }
                CapturedContent::Plain(_) => panic!("expected rich content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn returns_plain_when_clipboard_plain_text_mismatches() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("different".to_string()),
                html: Some("<p>different</p>".to_string()),
                rtf: None,
            }),
        };

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &rich_options(),
            &reader,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Plain(text) => assert_eq!(text, "hello"),
                CapturedContent::Rich(_) => panic!("expected plain content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn returns_plain_when_payload_exceeds_size_limit() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: Some("0123456789".to_string()),
                rtf: None,
            }),
        };
        let mut options = rich_options();
        options.max_rich_payload_bytes = 4;

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Plain(text) => assert_eq!(text, "hello"),
                CapturedContent::Rich(_) => panic!("expected plain content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn returns_plain_when_rich_feature_is_disabled_by_option() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: Some("<p>hello</p>".to_string()),
                rtf: None,
            }),
        };
        let mut options = rich_options();
        options.allow_clipboard_rich = false;

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Plain(text) => assert_eq!(text, "hello"),
                CapturedContent::Rich(_) => panic!("expected plain content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn prefers_direct_ax_rtf_before_clipboard_payload() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: Some("<p>hello from clipboard</p>".to_string()),
                rtf: None,
            }),
        };
        let mut options = rich_options();
        options.allow_direct_accessibility_rich = true;

        let out = capture_rich_with_reader_and_direct_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
            &|| Some("{\\rtf1 hello from direct}".to_string()),
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Rich(payload) => {
                    assert_eq!(payload.rtf.as_deref(), Some("{\\rtf1 hello from direct}"));
                    assert_eq!(payload.html, None);
                    assert_eq!(payload.metadata.source, RichSource::AccessibilityAttributed);
                }
                CapturedContent::Plain(_) => panic!("expected rich content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn falls_back_to_clipboard_when_direct_ax_rtf_is_unavailable() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: Some("<p>hello from clipboard</p>".to_string()),
                rtf: None,
            }),
        };
        let mut options = rich_options();
        options.allow_direct_accessibility_rich = true;

        let out = capture_rich_with_reader_and_direct_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
            &|| None,
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Rich(payload) => {
                    assert_eq!(payload.html.as_deref(), Some("<p>hello from clipboard</p>"));
                    assert_eq!(payload.metadata.source, RichSource::ClipboardHtml);
                }
                CapturedContent::Plain(_) => panic!("expected rich content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn falls_back_to_clipboard_when_direct_ax_rtf_exceeds_limit() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                "hello".to_string(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some("hello".to_string()),
                html: Some("<p>hello from clipboard</p>".to_string()),
                rtf: None,
            }),
        };
        let mut options = rich_options();
        options.allow_direct_accessibility_rich = true;
        options.max_rich_payload_bytes = 32;

        let out = capture_rich_with_reader_and_direct_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
            &|| Some("{\\rtf1 this payload is definitely too long}".to_string()),
        );

        match out {
            CaptureRichOutcome::Success(success) => match success.content {
                CapturedContent::Rich(payload) => {
                    assert_eq!(payload.html.as_deref(), Some("<p>hello from clipboard</p>"));
                    assert_eq!(payload.metadata.source, RichSource::ClipboardHtml);
                }
                CapturedContent::Plain(_) => panic!("expected rich content"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success"),
        }
    }

    #[test]
    fn preserves_failure_from_plain_capture() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![
                PlatformAttemptResult::EmptySelection,
                PlatformAttemptResult::EmptySelection,
                PlatformAttemptResult::EmptySelection,
            ])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader { payload: None };

        let out = capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &rich_options(),
            &reader,
        );

        match out {
            CaptureRichOutcome::Failure(failure) => {
                assert_eq!(failure.status, crate::types::CaptureStatus::EmptySelection);
            }
            CaptureRichOutcome::Success(_) => panic!("expected failure"),
        }
    }

    #[test]
    fn try_capture_rich_propagates_would_block() {
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader { payload: None };
        let mut options = rich_options();
        options.base.retry_policy.primary_accessibility = vec![Duration::from_millis(25)];
        options.base.retry_policy.range_accessibility = vec![Duration::from_millis(25)];
        options.base.retry_policy.clipboard = vec![Duration::from_millis(25)];

        let out = try_capture_rich_with_reader(
            &platform,
            &StubStore,
            &NeverCancel,
            &[&NoAdapters],
            &options,
            &reader,
        );

        assert_eq!(out, Err(WouldBlock));
    }
}
