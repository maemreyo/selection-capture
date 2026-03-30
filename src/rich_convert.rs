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
    let primary = normalize_plain_text(&rtf_to_markdown_from_bytes(input.as_bytes())?);
    if !input.contains(r"\par") || primary.contains('\n') {
        return Some(primary);
    }

    recover_paragraph_breaks_with_marker(input).or(Some(primary))
}

fn rtf_to_markdown_from_bytes(input: &[u8]) -> Option<String> {
    let html = rtf_to_html::rtf_to_html(input).ok()?;
    Some(quick_html2md::html_to_markdown(&html))
}

fn recover_paragraph_breaks_with_marker(input: &str) -> Option<String> {
    let marker = "__SELECTION_CAPTURE_PAR_BREAK__";
    let marked = inject_marker_after_par_control(input, marker);
    if marked == input {
        return None;
    }

    let plain = rtf_to_html::rtf_to_plain_text(marked.as_bytes()).ok()?;
    if !plain.contains(marker) {
        return None;
    }

    let normalized = normalize_plain_text(&plain.replace(marker, "\n"));
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn inject_marker_after_par_control(input: &str, marker: &str) -> String {
    let mut out = String::with_capacity(input.len() + marker.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1] == b'p'
            && bytes[i + 2] == b'a'
            && bytes[i + 3] == b'r'
            && (i + 4 == bytes.len() || !bytes[i + 4].is_ascii_alphabetic())
        {
            out.push_str(r"\par ");
            out.push_str(marker);
            i += 4;
            if i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
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
        assert_eq!(markdown, "Hello\nWorld");
    }

    #[test]
    fn falls_back_to_plain_text_when_rtf_conversion_fails() {
        let markdown = convert_to_markdown(None, Some("{\\rtf1\\ansi"), "fallback")
            .expect("markdown should exist");
        assert_eq!(markdown, "fallback");
    }

    #[test]
    fn injects_marker_only_for_par_control_word() {
        let marked = inject_marker_after_par_control(
            r"{\rtf1\ansi \pard\partightenfactor0 A\par B}",
            "__PAR__",
        );
        assert_eq!(
            marked,
            r"{\rtf1\ansi \pard\partightenfactor0 A\par __PAR__B}"
        );
    }

    #[test]
    fn marker_injection_survives_rtf_plain_text() {
        let marker = "__PAR__";
        let marked = inject_marker_after_par_control(r"{\rtf1\ansi Hello\par World}", marker);
        let plain =
            rtf_to_html::rtf_to_plain_text(marked.as_bytes()).expect("plain text extraction");
        assert!(plain.contains(marker));
    }

    #[test]
    fn marker_plain_text_differs_from_primary_plain_text() {
        let input = r"{\rtf1\ansi Hello\par World}";
        let primary = normalize_plain_text(
            &rtf_to_markdown_from_bytes(input.as_bytes()).expect("primary markdown"),
        );
        let marked_plain =
            recover_paragraph_breaks_with_marker(input).expect("marker recovery should work");
        assert_eq!(primary, "HelloWorld");
        assert_eq!(marked_plain, "Hello\nWorld");
    }

    #[test]
    fn wraps_plain_text_as_minimal_rtf() {
        let rtf = plain_text_to_minimal_rtf("a{b}\\c\nd");
        assert!(rtf.starts_with("{\\rtf1\\ansi "));
        assert!(rtf.contains("a\\{b\\}\\\\c\\par\nd"));
    }
}
