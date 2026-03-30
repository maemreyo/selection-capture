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

    fn attempt(&self, _method: CaptureMethod, _app: Option<&ActiveApp>) -> PlatformAttemptResult {
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

fn fixture_word_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/word-style-paragraphs.rtf")
}

fn fixture_outlook_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/outlook-style-bullets.rtf")
}

fn fixture_messy_controls_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/messy-controls-unicode.rtf")
}

fn fixture_messy_field_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/messy-field-escaped.rtf")
}

fn fixture_slack_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/real/slack-thread-export.rtf")
}

fn fixture_notion_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/real/notion-checklist-export.rtf")
}

fn fixture_teams_rtf() -> &'static str {
    include_str!("../tests/fixtures/rich/real/teams-chat-export.rtf")
}

fn fixture_word_markdown() -> &'static str {
    "Hello,\nThis is a Word exported paragraph.\nRegards,\nTeam"
}

#[derive(Clone, Copy)]
struct ReplayCase {
    name: &'static str,
    rtf: &'static str,
    required_terms: &'static [&'static str],
    expect_markdown_from_plain: bool,
}

fn assert_contains_all(haystack: &str, needles: &[&str], case_name: &str) {
    for needle in needles {
        assert!(
            haystack.contains(needle),
            "case `{case_name}` expected markdown to contain `{needle}`, got:\n{haystack}"
        );
    }
}

fn with_line_ending_variant(input: &str, variant_idx: usize) -> String {
    match variant_idx % 3 {
        0 => input.to_string(),
        1 => input.replace('\n', "\r\n"),
        _ => input.replace('\n', "\r"),
    }
}

fn replay_cases() -> [ReplayCase; 8] {
    const MALFORMED_RTF: &str = r"{\rtf1\ansi Broken {payload";
    [
        ReplayCase {
            name: "word",
            rtf: fixture_word_rtf(),
            required_terms: &["Hello,", "Word exported paragraph.", "Regards,", "Team"],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "outlook",
            rtf: fixture_outlook_rtf(),
            required_terms: &[
                "Weekly update:",
                "Ship crate-backed rich conversion",
                "observer tests deterministic",
                "Selection Capture Team",
            ],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "messy-controls",
            rtf: fixture_messy_controls_rtf(),
            required_terms: &[
                "Release notes:",
                "Align observer locks",
                "Harden RTF fallback",
                "Postscript:parser should keep this line.",
            ],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "messy-field",
            rtf: fixture_messy_field_rtf(),
            required_terms: &[
                "Meeting notes:",
                "Verify fallback ordering",
                "literal braces",
                "docs portal",
            ],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "slack",
            rtf: fixture_slack_rtf(),
            required_terms: &[
                "Slack digest:",
                "Deploy succeeded",
                "Follow-up in thread",
                "open thread",
            ],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "notion",
            rtf: fixture_notion_rtf(),
            required_terms: &[
                "Notion page: Sprint Checklist",
                "Stabilize conversion on CRLF",
                "Add replay corpus for real payloads",
                "Preserve escaped braces {ok}",
                "Quality over luck.",
            ],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "teams",
            rtf: fixture_teams_rtf(),
            required_terms: &[
                "Teams chat recap:",
                "windows-beta and linux-alpha",
                "CI gate: make ci",
                r"Escaped path: C:\repo\selection-capture\tests",
            ],
            expect_markdown_from_plain: false,
        },
        ReplayCase {
            name: "malformed",
            rtf: MALFORMED_RTF,
            required_terms: &[],
            expect_markdown_from_plain: true,
        },
    ]
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
fn populates_markdown_from_real_word_rtf_fixture() {
    let expected = fixture_word_markdown().to_string();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            expected.clone(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some(expected),
            html: None,
            rtf: Some(fixture_word_rtf().to_string()),
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
                assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                assert_eq!(payload.markdown.as_deref(), Some(fixture_word_markdown()));
            }
            CapturedContent::Plain(_) => panic!("expected rich content"),
        },
        CaptureRichOutcome::Failure(_) => panic!("expected success"),
    }
}

#[test]
fn prefers_html_source_when_html_and_real_rtf_are_both_present() {
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "HTML preferred".to_string(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some("HTML preferred".to_string()),
            html: Some("<p>HTML preferred</p>".to_string()),
            rtf: Some(fixture_outlook_rtf().to_string()),
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
                assert_eq!(payload.metadata.source, RichSource::ClipboardHtml);
                assert_eq!(payload.markdown.as_deref(), Some("HTML preferred"));
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
fn allows_real_rtf_when_plain_text_match_is_disabled() {
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            fixture_word_markdown().to_string(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some("mismatch plain text".to_string()),
            html: None,
            rtf: Some(fixture_word_rtf().to_string()),
        }),
    };
    let mut options = rich_options();
    options.require_plain_text_match = false;
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
                assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                assert_eq!(payload.markdown.as_deref(), Some(fixture_word_markdown()));
            }
            CapturedContent::Plain(_) => panic!("expected rich content"),
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
fn returns_plain_when_real_rtf_fixture_exceeds_size_limit() {
    let expected = fixture_word_markdown().to_string();
    let rtf = fixture_word_rtf().to_string();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            expected.clone(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some(expected.clone()),
            html: None,
            rtf: Some(rtf.clone()),
        }),
    };
    let mut options = rich_options();
    options.max_rich_payload_bytes = rtf.len().saturating_sub(1);

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
            CapturedContent::Plain(text) => assert_eq!(text, expected),
            CapturedContent::Rich(_) => panic!("expected plain content"),
        },
        CaptureRichOutcome::Failure(_) => panic!("expected success"),
    }
}

#[test]
fn plain_text_match_accepts_crlf_vs_lf_with_real_rtf_payload() {
    let expected = fixture_word_markdown().to_string();
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            expected.clone(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some(expected.replace('\n', "\r\n") + "\r\n"),
            html: None,
            rtf: Some(fixture_word_rtf().to_string()),
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
                assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                assert_eq!(payload.markdown.as_deref(), Some(fixture_word_markdown()));
            }
            CapturedContent::Plain(_) => panic!("expected rich content"),
        },
        CaptureRichOutcome::Failure(_) => panic!("expected success"),
    }
}

#[test]
fn populates_markdown_from_messy_controls_fixture() {
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "placeholder plain text".to_string(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some("mismatched plain text is allowed".to_string()),
            html: None,
            rtf: Some(fixture_messy_controls_rtf().to_string()),
        }),
    };
    let mut options = rich_options();
    options.require_plain_text_match = false;
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
                let markdown = payload
                    .markdown
                    .expect("markdown should be populated for messy controls fixture");
                let lines: Vec<&str> = markdown.lines().collect();
                assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                assert_eq!(lines.len(), 4);
                assert_eq!(lines[0], "Release notes:");
                assert!(lines[1].contains("Item 1:"));
                assert!(lines[1].contains("Align observer locks"));
                assert!(lines[2].contains("Item 2:"));
                assert!(lines[2].contains("Harden RTF fallback"));
                assert_eq!(lines[3], "Postscript:parser should keep this line.");
            }
            CapturedContent::Plain(_) => panic!("expected rich content"),
        },
        CaptureRichOutcome::Failure(_) => panic!("expected success"),
    }
}

#[test]
fn populates_markdown_from_messy_field_fixture() {
    let platform = StubPlatform {
        app: Some(ActiveApp {
            bundle_id: "app.rich".to_string(),
            name: "Rich App".to_string(),
        }),
        responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
            "placeholder plain text".to_string(),
        )])),
        cleanup: CleanupStatus::Clean,
    };
    let reader = StubReader {
        payload: Some(RichClipboardPayload {
            plain_text: Some("mismatched plain text is allowed".to_string()),
            html: None,
            rtf: Some(fixture_messy_field_rtf().to_string()),
        }),
    };
    let mut options = rich_options();
    options.require_plain_text_match = false;
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
                let markdown = payload
                    .markdown
                    .expect("markdown should be populated for messy field fixture");
                let lines: Vec<&str> = markdown.lines().collect();
                assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                assert_eq!(lines[0], "Meeting notes:");
                assert_eq!(lines[1], "- Verify fallback ordering");
                assert!(lines[2].contains("literal braces"));
                assert!(lines[2].contains("backslashes"));
                assert_eq!(lines[3], "docs portal");
                assert_eq!(lines[4], "End of note.");
            }
            CapturedContent::Plain(_) => panic!("expected rich content"),
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

#[test]
fn replays_real_fixture_corpus_with_plain_text_match_enabled() {
    let cases = replay_cases();

    for case in cases {
        if case.expect_markdown_from_plain {
            continue;
        }

        let mut options = rich_options();
        options.conversion = Some(RichConversion::Markdown);
        options.require_plain_text_match = true;

        let expected_markdown = maybe_convert_to_markdown(&options, None, Some(case.rtf), "")
            .expect("fixture should convert to markdown");
        assert_contains_all(&expected_markdown, case.required_terms, case.name);

        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                expected_markdown.clone(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some(expected_markdown.replace('\n', "\r\n") + "\r\n"),
                html: None,
                rtf: Some(case.rtf.to_string()),
            }),
        };

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
                    let markdown = payload
                        .markdown
                        .expect("markdown should be present for real replay case");
                    assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);
                    assert_eq!(markdown, expected_markdown);
                    assert_contains_all(&markdown, case.required_terms, case.name);
                }
                CapturedContent::Plain(_) => panic!("expected rich content for replay case"),
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success for replay case"),
        }
    }
}

#[test]
fn soak_replay_real_fixture_corpus_tracks_fallback_rate() {
    let cases = replay_cases();
    let iterations = 500usize;
    let mut plain_outcomes = 0usize;
    let mut markdown_from_rtf = 0usize;
    let mut markdown_from_plain = 0usize;
    let mut expected_plain_fallbacks = 0usize;

    for i in 0..iterations {
        let case = cases[i % cases.len()];
        if case.expect_markdown_from_plain {
            expected_plain_fallbacks += 1;
        }

        let plain_text = format!("plain capture from engine #{i}");
        let platform = StubPlatform {
            app: Some(ActiveApp {
                bundle_id: "app.rich".to_string(),
                name: "Rich App".to_string(),
            }),
            responses: Arc::new(Mutex::new(vec![PlatformAttemptResult::Success(
                plain_text.clone(),
            )])),
            cleanup: CleanupStatus::Clean,
        };
        let reader = StubReader {
            payload: Some(RichClipboardPayload {
                plain_text: Some(format!("clipboard mismatch #{i}")),
                html: None,
                rtf: Some(with_line_ending_variant(case.rtf, i)),
            }),
        };
        let mut options = rich_options();
        options.conversion = Some(RichConversion::Markdown);
        options.require_plain_text_match = false;

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
                    let markdown = payload
                        .markdown
                        .expect("markdown should always be populated during soak replay");
                    assert_eq!(payload.metadata.source, RichSource::ClipboardRtf);

                    if case.expect_markdown_from_plain {
                        assert_eq!(markdown, plain_text);
                        markdown_from_plain += 1;
                    } else {
                        assert_ne!(markdown, plain_text);
                        assert_contains_all(&markdown, case.required_terms, case.name);
                        markdown_from_rtf += 1;
                    }
                }
                CapturedContent::Plain(_) => {
                    plain_outcomes += 1;
                }
            },
            CaptureRichOutcome::Failure(_) => panic!("expected success during soak replay"),
        }
    }

    assert_eq!(
        plain_outcomes, 0,
        "unexpected plain rich outcomes during soak"
    );
    assert_eq!(markdown_from_plain, expected_plain_fallbacks);
    assert_eq!(
        markdown_from_rtf + markdown_from_plain,
        iterations,
        "every soak iteration must produce markdown"
    );
    assert!(markdown_from_rtf > markdown_from_plain);
}
