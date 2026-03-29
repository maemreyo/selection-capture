use clipboard_rs::{Clipboard, ClipboardContext};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RichClipboardPayload {
    pub(crate) plain_text: Option<String>,
    pub(crate) html: Option<String>,
    pub(crate) rtf: Option<String>,
}

pub(crate) trait RichClipboardReader {
    fn read(&self) -> Option<RichClipboardPayload>;
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemRichClipboardReader;

impl RichClipboardReader for SystemRichClipboardReader {
    fn read(&self) -> Option<RichClipboardPayload> {
        let ctx = ClipboardContext::new().ok()?;
        let html = ctx.get_html().ok().filter(|value| !value.is_empty());
        let rtf = ctx.get_rich_text().ok().filter(|value| !value.is_empty());

        if html.is_none() && rtf.is_none() {
            return None;
        }

        let plain_text = ctx.get_text().ok().filter(|value| !value.is_empty());
        Some(RichClipboardPayload {
            plain_text,
            html,
            rtf,
        })
    }
}
