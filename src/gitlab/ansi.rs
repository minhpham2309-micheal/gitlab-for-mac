pub use super::ansi_palette::{apply_sgr, tint, Rgb, Style, LOG_FG};
#[cfg(test)]
use super::ansi_palette::{GREEN, RED};
use super::log_sanitize::{sanitize_streaming_escapes, strip_to_plain};

fn decorate(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace("section_start:", "▶ ")
        .replace("section_end:", "■ ")
}

fn push_utf8(out: &mut String, input: &str, i: &mut usize) {
    match input[*i..].chars().next() {
        Some(ch) => {
            out.push(ch);
            *i += ch.len_utf8();
        }
        None => *i = input.len(),
    }
}

pub fn sanitize(input: &str) -> String {
    let mut pending = String::new();
    decorate(&sanitize_streaming_escapes(input, &mut pending))
}

pub fn strip(input: &str) -> String {
    decorate(&strip_to_plain(input))
}

pub fn colorize(input: &str) -> Vec<(String, Rgb)> {
    let source = sanitize(input);
    let bytes = source.as_bytes();
    let mut spans: Vec<(String, Rgb)> = Vec::new();
    let mut buf = String::new();
    let mut style = Style::reset();
    let mut i = 0;
    let mut flush = |buf: &mut String, color: Rgb| {
        if buf.is_empty() {
            return;
        }
        let text = std::mem::take(buf);
        if let Some((last, last_color)) = spans.last_mut() {
            if *last_color == color {
                last.push_str(&text);
                return;
            }
        }
        spans.push((text, color));
    };
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let color = tint(&buf, style.color());
            flush(&mut buf, color);
            i += 2;
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'm' {
                apply_sgr(&mut style, &source[start..i]);
            }
            i = (i + 1).min(bytes.len());
            continue;
        }
        push_utf8(&mut buf, &source, &mut i);
    }
    let color = tint(&buf, style.color());
    flush(&mut buf, color);
    if spans.is_empty() {
        spans.push((String::new(), LOG_FG));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_csi_and_keeps_text() {
        let raw = "\u{1b}[32mok\u{1b}[0m\r\nnext";
        assert_eq!(strip(raw), "ok\nnext");
    }

    #[test]
    fn colorize_uses_intellij_green() {
        let spans = colorize("\u{1b}[32mok\u{1b}[0m");
        assert_eq!(spans[0].0, "ok");
        assert_eq!(spans[0].1, GREEN);
    }

    #[test]
    fn colorize_marks_failed_lines() {
        let spans = colorize("ERROR: Job failed: exit code 1");
        assert_eq!(spans[0].1, RED);
    }

    #[test]
    fn keeps_utf8_instead_of_mojibake() {
        assert_eq!(strip("ok ✓ xong"), "ok ✓ xong");
        let text: String = colorize("section_end:step_script")
            .into_iter()
            .map(|(part, _)| part)
            .collect();
        assert!(text.contains('■'), "{text}");
        assert!(!text.contains('\u{00e2}'), "{text}");
    }

    #[test]
    fn job_succeeded_is_its_own_last_line() {
        let raw = "section_end:1:step_script\r\u{1b}[0KJob succeeded\r\n";
        let out = strip(raw);
        assert!(out.trim_end().ends_with("Job succeeded"), "{out:?}");
        assert!(out.lines().any(|line| line == "Job succeeded"), "{out:?}");
        assert!(!out.contains("scriptJob"), "{out:?}");
    }
}
