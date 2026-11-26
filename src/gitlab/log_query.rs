use super::log_model::{LogModel, LogSearchHit, VisibleLogRow};

impl LogModel {
    pub fn visible_rows(&self) -> Vec<VisibleLogRow> {
        let mut rows = Vec::with_capacity(self.lines().len() + self.sections().len());
        let mut emitted = vec![false; self.sections().len()];
        for (line_ix, line) in self.lines().iter().enumerate() {
            if let Some(sec_ix) = line.section {
                if !emitted[sec_ix] {
                    emitted[sec_ix] = true;
                    rows.push(VisibleLogRow::SectionHeader(sec_ix));
                }
                if self.sections().get(sec_ix).is_some_and(|s| s.collapsed) {
                    continue;
                }
            }
            rows.push(VisibleLogRow::Line(line_ix));
        }
        rows
    }

    pub fn search(&self, query: &str) -> Vec<LogSearchHit> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for (line_ix, line) in self.lines().iter().enumerate() {
            let lower = line.text.to_ascii_lowercase();
            let mut from = 0;
            while let Some(rel) = lower[from..].find(&needle) {
                let start = from + rel;
                let end = start + needle.len();
                hits.push(LogSearchHit { line_ix, start, end });
                from = end.max(start + 1);
            }
        }
        hits
    }

    pub fn plain_all(&self) -> String {
        self.lines()
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn plain_visible_range(&self, rows: &[VisibleLogRow], a: usize, b: usize) -> String {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let mut out = Vec::new();
        for row in rows.iter().skip(lo).take(hi.saturating_sub(lo) + 1) {
            match *row {
                VisibleLogRow::Line(ix) => {
                    if let Some(l) = self.lines().get(ix) {
                        out.push(l.text.as_str());
                    }
                }
                VisibleLogRow::SectionHeader(sx) => {
                    if let Some(sec) = self.sections().get(sx) {
                        out.push(sec.header.as_str());
                    }
                }
            }
        }
        out.join("\n")
    }
}
