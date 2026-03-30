use crate::types::{ActiveApp, CaptureFailure, CaptureMethod, CaptureOptions, CaptureTrace};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichFormat {
    Html,
    Rtf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichSource {
    ClipboardHtml,
    ClipboardRtf,
    AccessibilityAttributed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichConversion {
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentMetadata {
    pub active_app: Option<ActiveApp>,
    pub method: CaptureMethod,
    pub source: RichSource,
    pub captured_at_unix_ms: u128,
    pub plain_text_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichPayload {
    pub plain_text: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub markdown: Option<String>,
    pub metadata: ContentMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapturedContent {
    Plain(String),
    Rich(RichPayload),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureRichSuccess {
    pub content: CapturedContent,
    pub method: CaptureMethod,
    pub trace: Option<CaptureTrace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureRichOutcome {
    Success(CaptureRichSuccess),
    Failure(CaptureFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureRichOptions {
    pub base: CaptureOptions,
    pub prefer_rich: bool,
    pub allow_clipboard_rich: bool,
    pub allow_direct_accessibility_rich: bool,
    pub conversion: Option<RichConversion>,
    pub max_rich_payload_bytes: usize,
    pub require_plain_text_match: bool,
}

impl Default for CaptureRichOptions {
    fn default() -> Self {
        Self {
            base: CaptureOptions::default(),
            prefer_rich: true,
            allow_clipboard_rich: true,
            allow_direct_accessibility_rich: true,
            conversion: None,
            max_rich_payload_bytes: 256 * 1024,
            require_plain_text_match: true,
        }
    }
}
