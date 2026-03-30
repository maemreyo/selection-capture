pub(crate) fn convert_to_markdown(
    html: Option<&str>,
    rtf: Option<&str>,
    plain_text: &str,
) -> Option<String> {
    if let Some(html) = html {
        let markdown = normalize_plain_text(&quick_html2md::html_to_markdown(html));
        if !markdown.is_empty() {
            return Some(markdown);
        }
    }

    if let Some(rtf) = rtf {
        if let Some(markdown) = rtf_to_markdown(rtf) {
            if !markdown.is_empty() {
                return Some(markdown);
            }
        }
    }

    let fallback = normalize_plain_text(plain_text);
    if fallback.is_empty() {
        None
    } else {
        Some(fallback)
    }
}

#[cfg(any(
    test,
    all(feature = "windows-beta", target_os = "windows"),
    all(feature = "linux-alpha", target_os = "linux")
))]
pub(crate) fn plain_text_to_minimal_rtf(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut escaped = String::new();
    for ch in normalized.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '{' => escaped.push_str("\\{"),
            '}' => escaped.push_str("\\}"),
            '\n' => escaped.push_str("\\par\n"),
            _ => escaped.push(ch),
        }
    }
    format!("{{\\rtf1\\ansi {escaped}}}")
}

fn rtf_to_markdown(input: &str) -> Option<String> {
    let html = rtf_to_html::rtf_to_html(input.as_bytes()).ok()?;
    let markdown = quick_html2md::html_to_markdown(&html);
    Some(normalize_plain_text(&markdown))
}

fn normalize_plain_text(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().map(str::trim_end).collect();
    let joined = lines.join("\n");
    joined.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_html_to_markdownish_text() {
        let markdown =
            convert_to_markdown(Some("<p>Hello<br>World</p><ul><li>A</li></ul>"), None, "")
                .expect("markdown should exist");
        assert_eq!(markdown, "Hello\nWorld\n\n- A");
    }

    #[test]
    fn converts_rtf_to_text_when_html_missing() {
        let markdown = convert_to_markdown(None, Some("{\\rtf1\\ansi Hello\\par World}"), "")
            .expect("markdown should exist");
        assert_eq!(markdown, "HelloWorld");
    }

    #[test]
    fn falls_back_to_plain_text_when_rtf_conversion_fails() {
        let markdown = convert_to_markdown(None, Some("{\\rtf1\\ansi"), "fallback")
            .expect("markdown should exist");
        assert_eq!(markdown, "fallback");
    }

    #[test]
    fn wraps_plain_text_as_minimal_rtf() {
        let rtf = plain_text_to_minimal_rtf("a{b}\\c\nd");
        assert!(rtf.starts_with("{\\rtf1\\ansi "));
        assert!(rtf.contains("a\\{b\\}\\\\c\\par\nd"));
    }
}
