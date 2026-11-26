use std::time::Duration;

use gpui_kit::{ClipboardItem, Context, ScrollStrategy, Window};

use crate::gitlab::log_model::VisibleLogRow;

use super::pipelines::PipelinesView;

impl PipelinesView {
    pub(crate) fn wipe_log(&mut self) {
        self.log_model.clear();
        self.visible_rows.clear();
        self.log_offset = 0;
        self.search_hits.clear();
        self.search_idx = 0;
        self.sel_anchor = None;
        self.sel_end = None;
    }

    pub(crate) fn push_log_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.log_model.push_bytes(bytes);
        self.rebuild_visible_rows();
        let query = self.search_input.read(cx).value().to_string();
        if !query.trim().is_empty() {
            self.search_hits = self.log_model.search(&query);
            if self.search_hits.is_empty() {
                self.search_idx = 0;
            } else if self.search_idx >= self.search_hits.len() {
                self.search_idx = self.search_hits.len() - 1;
            }
        } else {
            self.search_hits.clear();
            self.search_idx = 0;
        }
        if self.auto_scroll_tail && !self.visible_rows.is_empty() {
            self.log_scroll
                .scroll_to_item(self.visible_rows.len() - 1, ScrollStrategy::Bottom);
        }
        cx.notify();
    }

    pub(crate) fn rebuild_visible_rows(&mut self) {
        self.visible_rows = self.log_model.visible_rows();
    }

    pub fn toggle_section(&mut self, section_ix: usize, cx: &mut Context<Self>) {
        self.log_model.toggle_section(section_ix);
        self.rebuild_visible_rows();
        cx.notify();
    }

    pub fn toggle_auto_scroll(&mut self, cx: &mut Context<Self>) {
        self.auto_scroll_tail = !self.auto_scroll_tail;
        if self.auto_scroll_tail && !self.visible_rows.is_empty() {
            self.log_scroll
                .scroll_to_item(self.visible_rows.len() - 1, ScrollStrategy::Bottom);
        }
        self.sync_summary(cx);
        cx.notify();
    }

    pub fn pause_auto_scroll(&mut self, cx: &mut Context<Self>) {
        if self.auto_scroll_tail {
            self.auto_scroll_tail = false;
            self.sync_summary(cx);
            cx.notify();
        }
    }

    pub fn toggle_wrap(&mut self, cx: &mut Context<Self>) {
        self.wrap_lines = !self.wrap_lines;
        cx.notify();
    }

    pub fn select_log_row(&mut self, row_ix: usize, shift: bool, cx: &mut Context<Self>) {
        if shift && self.sel_anchor.is_some() {
            self.sel_end = Some(row_ix);
        } else {
            self.sel_anchor = Some(row_ix);
            self.sel_end = Some(row_ix);
        }
        self.pause_auto_scroll(cx);
        cx.notify();
    }

    pub fn selection_bounds(&self) -> Option<(usize, usize)> {
        let (a, b) = (self.sel_anchor?, self.sel_end?);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    pub fn copy_selected_lines(&mut self, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.selection_bounds() else {
            return;
        };
        let text = self.log_model.plain_visible_range(&self.visible_rows, lo, hi);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        let count = hi - lo + 1;
        self.state
            .update(cx, |s, cx| s.set_status(format!("Copied {count} selected lines"), cx));
    }

    pub fn copy_all_log(&mut self, cx: &mut Context<Self>) {
        if self.log_model.is_empty() {
            return;
        }
        let plain = self.log_model.plain_all();
        let n = self.log_model.line_count();
        cx.write_to_clipboard(ClipboardItem::new_string(plain));
        self.state
            .update(cx, |s, cx| s.set_status(format!("Copied {n} log lines"), cx));
    }

    pub(crate) fn schedule_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        self.search_debounce_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(80)).await;
            let _ = this.update(cx, |v, cx| {
                v.search_hits = v.log_model.search(&query);
                v.search_idx = 0;
                if !v.search_hits.is_empty() {
                    v.scroll_to_hit(0);
                }
                cx.notify();
            });
        }));
    }

    pub fn jump_search(&mut self, next: bool, cx: &mut Context<Self>) {
        if self.search_hits.is_empty() {
            let q = self.search_input.read(cx).value().to_string();
            self.search_hits = self.log_model.search(&q);
        }
        if self.search_hits.is_empty() {
            return;
        }
        let n = self.search_hits.len();
        self.search_idx = if next {
            (self.search_idx + 1) % n
        } else {
            (self.search_idx + n - 1) % n
        };
        self.scroll_to_hit(self.search_idx);
        cx.notify();
    }

    pub fn clear_log_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_debounce_task = None;
        self.search_hits.clear();
        self.search_idx = 0;
        self.search_input.update(cx, |i, cx| i.set_value("", window, cx));
        cx.notify();
    }

    fn scroll_to_hit(&mut self, idx: usize) {
        let Some(hit) = self.search_hits.get(idx).copied() else {
            return;
        };
        self.log_model.expand_for_line(hit.line_ix);
        self.rebuild_visible_rows();
        if let Some(row_ix) = self
            .visible_rows
            .iter()
            .position(|r| *r == VisibleLogRow::Line(hit.line_ix))
        {
            self.log_scroll.scroll_to_item(row_ix, ScrollStrategy::Center);
        }
    }
}
