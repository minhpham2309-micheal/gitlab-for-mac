#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogHit {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

pub fn log_hits(text: &str, query: &str) -> Vec<LogHit> {
    let needle = query.trim();
    if needle.is_empty() {
        return Vec::new();
    }
    let hay = text.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut hits = Vec::new();
    let mut from = 0;
    while let Some(rel) = hay[from..].find(&needle) {
        let at = from + rel;
        let end = at + needle.len();
        from = end.max(at + 1);
        if text.get(at..end).is_some_and(|span| span.contains('\n')) {
            continue;
        }
        let line = text[..at].bytes().filter(|&b| b == b'\n').count();
        let col = at - text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
        hits.push(LogHit {
            line,
            start: col,
            end: col + needle.len(),
        });
    }
    hits
}

pub const LOG_VIEW_LINES: usize = 400;
const LOG_VIEW_CHARS: usize = 160 * 1024;

pub fn log_view(log: &str) -> &str {
    let tailed = log_tail(log, LOG_VIEW_LINES);
    if tailed.len() <= LOG_VIEW_CHARS {
        return tailed;
    }
    let mut start = tailed.len() - LOG_VIEW_CHARS;
    while start < tailed.len() && !tailed.is_char_boundary(start) {
        start += 1;
    }
    match tailed[start..].find('\n') {
        Some(i) => &tailed[start + i + 1..],
        None => &tailed[start..],
    }
}

fn log_tail(log: &str, max_lines: usize) -> &str {
    let trimmed = log.trim_end_matches('\n');
    let mut seen = 0;
    let bytes = trimmed.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'\n' {
            seen += 1;
            if seen >= max_lines {
                return &trimmed[i + 1..];
            }
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_tail_keeps_last_lines() {
        let log = "a\nb\nc\nd\n";
        assert_eq!(log_tail(log, 2), "c\nd");
        assert_eq!(log_view(log), "a\nb\nc\nd");
    }

    #[test]
    fn log_view_drops_the_head() {
        let log = (0..900).map(|n| format!("line-{n}")).collect::<Vec<_>>().join("\n");
        let view = log_view(&log);
        assert!(view.starts_with("line-500"));
        assert!(view.ends_with("line-899"));
        assert!(!view.contains("line-100\n"));
    }

    #[test]
    fn log_hits_are_case_insensitive() {
        let hits = log_hits("ERROR\nwarn error\n", "Error");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0], LogHit { line: 0, start: 0, end: 5 });
        assert_eq!(hits[1], LogHit { line: 1, start: 5, end: 10 });
    }
}
