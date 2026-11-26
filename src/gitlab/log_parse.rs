use std::ops::Range;

use super::ansi_palette::{apply_sgr, tint, Rgb, Style};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsiSpan {
    pub range: Range<usize>,
    pub color: Rgb,
    pub bold: bool,
}

#[derive(Debug, Clone)]
pub enum SectionMarker {
    Start { ts: u64, id: String, collapsed: bool },
    End { ts: u64, id: String },
}

pub fn parse_section_marker(segment: &str) -> Option<SectionMarker> {
    let trimmed = segment.trim();
    if let Some(rest) = trimmed.strip_prefix("section_start:") {
        let mut parts = rest.splitn(2, ':');
        let ts = parts.next()?.parse::<u64>().ok()?;
        let id_raw = parts.next()?;
        let (id, collapsed) = match id_raw.split_once('[') {
            Some((name, opts)) => (name.trim(), opts.contains("collapsed=true")),
            None => (id_raw.trim(), false),
        };
        if !id.is_empty() {
            return Some(SectionMarker::Start { ts, id: id.to_string(), collapsed });
        }
    } else if let Some(rest) = trimmed.strip_prefix("section_end:") {
        let mut parts = rest.splitn(2, ':');
        let ts = parts.next()?.parse::<u64>().ok()?;
        let id = parts.next()?.trim();
        if !id.is_empty() {
            return Some(SectionMarker::End { ts, id: id.to_string() });
        }
    }
    None
}

pub fn parse_line_spans(raw: &str, style: &mut Style) -> (String, Vec<AnsiSpan>) {
    let bytes = raw.as_bytes();
    let mut text = String::with_capacity(raw.len());
    let mut spans: Vec<AnsiSpan> = Vec::new();
    let mut i = 0;
    let mut run_start = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            flush_span(&text, run_start, *style, &mut spans);
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'm' {
                j += 1;
            }
            if j < bytes.len() {
                apply_sgr(style, &raw[i + 2..j]);
                i = j + 1;
            } else {
                i = bytes.len();
            }
            run_start = text.len();
            continue;
        }
        if let Some(ch) = raw[i..].chars().next() {
            text.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    flush_span(&text, run_start, *style, &mut spans);
    (text, spans)
}

fn flush_span(text: &str, start: usize, style: Style, spans: &mut Vec<AnsiSpan>) {
    if start >= text.len() {
        return;
    }
    let slice = &text[start..];
    let color = tint(slice, style.color());
    if let Some(last) = spans.last_mut()
        && last.color == color && last.bold == style.bold && last.range.end == start {
            last.range.end = text.len();
            return;
        }
    spans.push(AnsiSpan {
        range: start..text.len(),
        color,
        bold: style.bold,
    });
}
