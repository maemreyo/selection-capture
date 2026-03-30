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
    let sanitized = strip_rtf_non_content_groups(input);
    let primary = normalize_plain_text(&rtf_to_markdown_from_bytes(sanitized.as_bytes())?);
    if !sanitized.contains(r"\par") || primary.contains('\n') {
        return Some(primary);
    }

    recover_paragraph_breaks_with_marker(&sanitized).or(Some(primary))
}

fn rtf_to_markdown_from_bytes(input: &[u8]) -> Option<String> {
    let html = rtf_to_html::rtf_to_html(input).ok()?;
    Some(quick_html2md::html_to_markdown(&html))
}

fn strip_rtf_non_content_groups(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut depth = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'\\' {
            out.push('\\');
            i += 1;
            if i < bytes.len() {
                out.push(bytes[i] as char);
                i += 1;
            }
            continue;
        }

        if bytes[i] == b'{' {
            let group_depth = depth + 1;
            if group_depth == 2
                && (input[i..].starts_with(r"{\info") || input[i..].starts_with(r"{\*\generator"))
            {
                if let Some(end) = find_rtf_group_end(bytes, i) {
                    i = end;
                    continue;
                }
            }

            depth += 1;
            out.push('{');
            i += 1;
            continue;
        }

        if bytes[i] == b'}' {
            depth = depth.saturating_sub(1);
            out.push('}');
            i += 1;
            continue;
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

fn find_rtf_group_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'{') {
        return None;
    }

    let mut depth = 0usize;
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i = i.saturating_add(2);
            continue;
        }

        if bytes[i] == b'{' {
            depth += 1;
        } else if bytes[i] == b'}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(i + 1);
            }
        }

        i += 1;
    }

    None
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

    fn assert_contains_all(haystack: &str, needles: &[&str]) {
        for needle in needles {
            assert!(
                haystack.contains(needle),
                "expected markdown to contain `{needle}`, got:\n{haystack}"
            );
        }
    }

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
    fn converts_word_style_rtf_fixture_to_multiline_text() {
        let input = include_str!("../tests/fixtures/rich/word-style-paragraphs.rtf");
        let markdown =
            convert_to_markdown(None, Some(input), "").expect("markdown should be present");
        assert_eq!(
            markdown,
            "Hello,\nThis is a Word exported paragraph.\nRegards,\nTeam"
        );
    }

    #[test]
    fn converts_outlook_style_rtf_fixture_to_multiline_text() {
        let input = include_str!("../tests/fixtures/rich/outlook-style-bullets.rtf");
        let markdown =
            convert_to_markdown(None, Some(input), "").expect("markdown should be present");
        assert_eq!(
            markdown,
            "Weekly update:\n- Ship crate-backed rich conversion\n- Keep observer tests deterministic\nThanks,\nSelection Capture Team"
        );
    }

    #[test]
    fn converts_word_style_rtf_fixture_with_crlf_line_endings() {
        let input = include_str!("../tests/fixtures/rich/word-style-paragraphs.rtf");
        let crlf_input = input.replace('\n', "\r\n");
        let markdown = convert_to_markdown(None, Some(&crlf_input), "")
            .expect("markdown should be present for crlf input");
        assert_eq!(
            markdown,
            "Hello,\nThis is a Word exported paragraph.\nRegards,\nTeam"
        );
    }

    #[test]
    fn falls_back_to_rtf_when_html_is_present_but_empty() {
        let rtf = include_str!("../tests/fixtures/rich/word-style-paragraphs.rtf");
        let markdown = convert_to_markdown(Some(""), Some(rtf), "")
            .expect("rtf should be used when html conversion is empty");
        assert_eq!(
            markdown,
            "Hello,\nThis is a Word exported paragraph.\nRegards,\nTeam"
        );
    }

    #[test]
    fn prefers_html_when_html_and_rtf_both_have_content() {
        let rtf = include_str!("../tests/fixtures/rich/outlook-style-bullets.rtf");
        let markdown = convert_to_markdown(Some("<p>HTML wins</p>"), Some(rtf), "")
            .expect("html should win when non-empty");
        assert_eq!(markdown, "HTML wins");
    }

    #[test]
    fn converts_messy_controls_fixture_with_stable_line_structure() {
        let rtf = include_str!("../tests/fixtures/rich/messy-controls-unicode.rtf");
        let markdown =
            convert_to_markdown(None, Some(rtf), "").expect("messy fixture should convert");
        let lines: Vec<&str> = markdown.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "Release notes:");
        assert!(lines[1].contains("Item 1:"));
        assert!(lines[1].contains("Align observer locks"));
        assert!(lines[2].contains("Item 2:"));
        assert!(lines[2].contains("Harden RTF fallback"));
        assert_eq!(lines[3], "Postscript:parser should keep this line.");
    }

    #[test]
    fn converts_messy_field_fixture_with_escaped_literals_and_link_text() {
        let rtf = include_str!("../tests/fixtures/rich/messy-field-escaped.rtf");
        let markdown =
            convert_to_markdown(None, Some(rtf), "").expect("messy field fixture should convert");
        let lines: Vec<&str> = markdown.lines().collect();
        assert_eq!(lines[0], "Meeting notes:");
        assert_eq!(lines[1], "- Verify fallback ordering");
        assert!(lines[2].contains("literal braces"));
        assert!(lines[2].contains("backslashes"));
        assert_eq!(lines[3], "docs portal");
        assert_eq!(lines[4], "End of note.");
    }

    #[test]
    fn strips_info_and_generator_groups_before_conversion() {
        let rtf = include_str!("../tests/fixtures/rich/messy-field-escaped.rtf");
        let sanitized = strip_rtf_non_content_groups(rtf);
        assert!(!sanitized.contains(r"{\info"));
        assert!(!sanitized.contains(r"{\*\generator"));
        assert!(sanitized.contains("Meeting notes:"));
    }

    #[test]
    fn converts_real_slack_fixture_with_expected_terms() {
        let rtf = include_str!("../tests/fixtures/rich/real/slack-thread-export.rtf");
        let markdown =
            convert_to_markdown(None, Some(rtf), "").expect("slack fixture should convert");
        assert_contains_all(
            &markdown,
            &[
                "Slack digest:",
                "Deploy succeeded",
                "Follow-up in thread",
                "cargo test --features rich-content",
                "open thread",
            ],
        );
    }

    #[test]
    fn converts_real_notion_fixture_and_ignores_info_metadata() {
        let rtf = include_str!("../tests/fixtures/rich/real/notion-checklist-export.rtf");
        let markdown =
            convert_to_markdown(None, Some(rtf), "").expect("notion fixture should convert");
        assert_contains_all(
            &markdown,
            &[
                "Notion page: Sprint Checklist",
                "Stabilize conversion on CRLF",
                "Add replay corpus for real payloads",
                "Preserve escaped braces {ok}",
                "Quote: Quality over luck.",
            ],
        );
        assert!(!markdown.contains("Notion Exporter"));
    }

    #[test]
    fn converts_real_teams_fixture_with_escaped_chars_intact() {
        let rtf = include_str!("../tests/fixtures/rich/real/teams-chat-export.rtf");
        let markdown =
            convert_to_markdown(None, Some(rtf), "").expect("teams fixture should convert");
        assert_contains_all(
            &markdown,
            &[
                "Teams chat recap:",
                "windows-beta and linux-alpha",
                "CI gate: make ci",
                r"Escaped path: C:\repo\selection-capture\tests",
                "Braces sample: {rich}",
            ],
        );
    }

    #[test]
    fn wraps_plain_text_as_minimal_rtf() {
        let rtf = plain_text_to_minimal_rtf("a{b}\\c\nd");
        assert!(rtf.starts_with("{\\rtf1\\ansi "));
        assert!(rtf.contains("a\\{b\\}\\\\c\\par\nd"));
    }
}
