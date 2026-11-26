use std::collections::HashMap;

use super::ansi_palette::Style;
use super::log_parse::{parse_line_spans, parse_section_marker, AnsiSpan, SectionMarker};
use super::log_sanitize::{sanitize_streaming_escapes, split_utf8_complete};

const MAX_LOG_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub number: usize,
    pub text: String,
    pub spans: Vec<AnsiSpan>,
    pub section: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct LogSection {
    pub id: String,
    pub header: String,
    pub start_line: usize,
    pub end_line: Option<usize>,
    pub start_ts: u64,
    pub duration_secs: Option<u64>,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleLogRow {
    SectionHeader(usize),
    Line(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogSearchHit {
    pub line_ix: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Default)]
pub struct LogModel {
    utf8_buf: Vec<u8>,
    pending_esc: String,
    pending_line: String,
    pending_cr: bool,
    style: Style,
    lines: Vec<LogLine>,
    sections: Vec<LogSection>,
    open_sections: HashMap<String, usize>,
    active_section_stack: Vec<usize>,
    total_bytes: usize,
    next_line_number: usize,
}

impl Default for Style {
    fn default() -> Self {
        Self::reset()
    }
}

impl LogModel {
    pub fn new() -> Self {
        Self { next_line_number: 1, ..Self::default() }
    }

    pub fn clear(&mut self) { *self = Self::new(); }
    pub fn line_count(&self) -> usize { self.lines.len() }
    pub fn lines(&self) -> &[LogLine] { &self.lines }
    pub fn sections(&self) -> &[LogSection] { &self.sections }
    pub fn is_empty(&self) -> bool { self.lines.is_empty() && self.pending_line.is_empty() }

    pub fn toggle_section(&mut self, section_ix: usize) {
        if let Some(sec) = self.sections.get_mut(section_ix) {
            sec.collapsed = !sec.collapsed;
        }
    }

    pub fn expand_for_line(&mut self, line_ix: usize) {
        if let Some(sec_ix) = self.lines.get(line_ix).and_then(|l| l.section)
            && let Some(sec) = self.sections.get_mut(sec_ix) {
                sec.collapsed = false;
            }
    }

    pub fn push_bytes(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        self.utf8_buf.extend_from_slice(chunk);
        let decoded = split_utf8_complete(&mut self.utf8_buf);
        let clean = sanitize_streaming_escapes(&decoded, &mut self.pending_esc);

        for ch in clean.chars() {
            if self.pending_cr {
                self.pending_cr = false;
                if ch == '\n' {
                    let raw = std::mem::take(&mut self.pending_line);
                    self.commit_segment(&raw);
                    continue;
                }
                self.pending_line.clear();
            }
            match ch {
                '\n' => {
                    let raw = std::mem::take(&mut self.pending_line);
                    self.commit_segment(&raw);
                }
                '\r' => {
                    if parse_section_marker(&self.pending_line).is_some() {
                        let raw = std::mem::take(&mut self.pending_line);
                        self.commit_segment(&raw);
                    } else {
                        self.pending_cr = true;
                    }
                }
                c => self.pending_line.push(c),
            }
        }
        self.enforce_memory_cap();
    }

    pub fn flush_partial(&mut self) {
        self.pending_cr = false;
        if !self.pending_line.is_empty() {
            let raw = std::mem::take(&mut self.pending_line);
            self.commit_segment(&raw);
        }
    }

    fn commit_segment(&mut self, raw: &str) {
        if let Some(marker) = parse_section_marker(raw) {
            match marker {
                SectionMarker::Start { ts, id, collapsed } => {
                    let idx = self.sections.len();
                    let header = id.replace('_', " ");
                    self.sections.push(LogSection {
                        id: id.clone(),
                        header,
                        start_line: self.lines.len(),
                        end_line: None,
                        start_ts: ts,
                        duration_secs: None,
                        collapsed,
                    });
                    self.open_sections.insert(id, idx);
                    self.active_section_stack.push(idx);
                }
                SectionMarker::End { ts, id } => {
                    if let Some(idx) = self.open_sections.remove(&id) {
                        if let Some(sec) = self.sections.get_mut(idx) {
                            sec.end_line = Some(self.lines.len());
                            sec.duration_secs = Some(ts.saturating_sub(sec.start_ts));
                        }
                        self.active_section_stack.retain(|&s| s != idx);
                    }
                }
            }
            return;
        }
        let (text, spans) = parse_line_spans(raw, &mut self.style);
        let section = self.active_section_stack.last().copied();
        if let Some(sec_ix) = section
            && let Some(sec) = self.sections.get_mut(sec_ix)
                && sec.start_line == self.lines.len() && !text.trim().is_empty() {
                    sec.header = text.trim().to_string();
                }
        self.total_bytes = self.total_bytes.saturating_add(text.len());
        let number = self.next_line_number;
        self.next_line_number += 1;
        self.lines.push(LogLine { number, text, spans, section });
    }

    fn enforce_memory_cap(&mut self) {
        if self.total_bytes <= MAX_LOG_BYTES {
            return;
        }
        let target = MAX_LOG_BYTES * 3 / 4;
        let mut drop_count = 0;
        while drop_count < self.lines.len() && self.total_bytes > target {
            self.total_bytes = self.total_bytes.saturating_sub(self.lines[drop_count].text.len());
            drop_count += 1;
        }
        if drop_count > 0 {
            self.lines.drain(..drop_count);
            for sec in &mut self.sections {
                sec.start_line = sec.start_line.saturating_sub(drop_count);
                sec.end_line = sec.end_line.map(|e| e.saturating_sub(drop_count));
            }
        }
    }
}
