pub(crate) fn convert_to_markdown(
    html: Option<&str>,
    rtf: Option<&str>,
    plain_text: &str,
) -> Option<String> {
    if let Some(html) = html {
        let markdown = html_to_markdown(html);
        if !markdown.is_empty() {
            return Some(markdown);
        }
    }

    if let Some(rtf) = rtf {
        let markdown = normalize_plain_text(&rtf_to_text(rtf));
        if !markdown.is_empty() {
            return Some(markdown);
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

fn html_to_markdown(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut tag = String::new();
            for next in chars.by_ref() {
                if next == '>' {
                    break;
                }
                tag.push(next);
            }
            apply_tag_effect(&tag, &mut output);
            continue;
        }
        output.push(ch);
    }

    normalize_plain_text(&decode_html_entities(&output))
}

fn apply_tag_effect(tag: &str, output: &mut String) {
    let normalized = tag.trim().to_ascii_lowercase();
    let name = normalized
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');

    if matches!(
        name,
        "br" | "p"
            | "/p"
            | "div"
            | "/div"
            | "section"
            | "/section"
            | "article"
            | "/article"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "/h1"
            | "/h2"
            | "/h3"
            | "/h4"
            | "/h5"
            | "/h6"
            | "ul"
            | "/ul"
            | "ol"
            | "/ol"
            | "li"
            | "/li"
    ) {
        if !output.ends_with('\n') && !output.is_empty() {
            output.push('\n');
        }
        if name == "li" {
            output.push_str("- ");
        }
    }
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn normalize_plain_text(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().map(str::trim_end).collect();
    let joined = lines.join("\n");
    joined.trim().to_string()
}

fn rtf_to_text(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' | '}' => {}
            '\\' => {
                let Some(next) = chars.peek().copied() else {
                    break;
                };

                if matches!(next, '\\' | '{' | '}') {
                    out.push(next);
                    let _ = chars.next();
                    continue;
                }

                if next == '\'' {
                    let _ = chars.next();
                    let first = chars.next();
                    let second = chars.next();
                    if let (Some(first), Some(second)) = (first, second) {
                        let hex = [first, second].iter().collect::<String>();
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            out.push(byte as char);
                        }
                    }
                    continue;
                }

                let mut word = String::new();
                while let Some(peek) = chars.peek() {
                    if peek.is_ascii_alphabetic() {
                        word.push(*peek);
                        let _ = chars.next();
                    } else {
                        break;
                    }
                }

                let mut number = String::new();
                if chars
                    .peek()
                    .is_some_and(|peek| *peek == '-' || *peek == '+')
                {
                    number.push(chars.next().unwrap_or_default());
                }
                while let Some(peek) = chars.peek() {
                    if peek.is_ascii_digit() {
                        number.push(*peek);
                        let _ = chars.next();
                    } else {
                        break;
                    }
                }

                if chars.peek().is_some_and(|peek| *peek == ' ') {
                    let _ = chars.next();
                }

                match word.as_str() {
                    "par" | "line" => {
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    "tab" => out.push('\t'),
                    "u" => {
                        if let Ok(codepoint) = number.parse::<i32>() {
                            if let Some(ch) = char::from_u32(codepoint as u32) {
                                out.push(ch);
                            }
                        }
                        if chars.peek().is_some_and(|peek| *peek == '?') {
                            let _ = chars.next();
                        }
                    }
                    _ => {}
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_html_to_markdownish_text() {
        let markdown =
            convert_to_markdown(Some("<p>Hello<br>World</p><ul><li>A</li></ul>"), None, "")
                .expect("markdown should exist");
        assert_eq!(markdown, "Hello\nWorld\n- A");
    }

    #[test]
    fn converts_rtf_to_text_when_html_missing() {
        let markdown = convert_to_markdown(None, Some("{\\rtf1\\ansi Hello\\par World}"), "")
            .expect("markdown should exist");
        assert_eq!(markdown, "Hello\nWorld");
    }

    #[test]
    fn wraps_plain_text_as_minimal_rtf() {
        let rtf = plain_text_to_minimal_rtf("a{b}\\c\nd");
        assert!(rtf.starts_with("{\\rtf1\\ansi "));
        assert!(rtf.contains("a\\{b\\}\\\\c\\par\nd"));
    }
}
