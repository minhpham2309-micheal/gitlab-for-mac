use gpui_kit::{
    component::ActiveTheme as _, div, px, rgb, uniform_list, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText,
};

use crate::gitlab::log_model::VisibleLogRow;

use super::job_log_header::{render_job_log_header, render_job_log_search_bar};
use super::job_log_hl::merge_highlight_runs;
use super::pipelines::PipelinesView;

pub fn render_job_log(
    view: &PipelinesView,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement {
    let Some(job) = view.selected_job.as_ref() else {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("Click a job card above to stream its live log")
            .into_any_element();
    };

    let can_run_ci = view.state.read(cx).project_permissions.can_run_pipelines();
    let header = render_job_log_header(
        job,
        view.log_model.line_count(),
        view.auto_scroll_tail,
        view.wrap_lines,
        view.selection_bounds().is_some(),
        can_run_ci,
        cx,
    );
    let search_bar = render_job_log_search_bar(
        &view.search_input,
        &view.search_hits,
        view.search_idx,
        cx,
    );

    let dark = cx.theme().is_dark();
    let row_count = view.visible_rows.len();
    let log_bg = rgb(if dark { 0x121117 } else { 0x1A1920 });
    let log_fg = rgb(0xE8ECF1);
    let active_hit = view.search_hits.get(view.search_idx).copied();

    let list_el = uniform_list(
        "job-log-uniform-list",
        row_count,
        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
            let theme = cx.theme();
            let mono = theme.mono_font_family.clone();
            let mono_size = theme.mono_font_size;
            let row_h = px((f32::from(mono_size) * 1.55).ceil().max(20.));
            let sel = this.selection_bounds();
            range
                .map(|row_ix| {
                    let row = this.visible_rows[row_ix];
                    let selected = sel.is_some_and(|(lo, hi)| row_ix >= lo && row_ix <= hi);
                    match row {
                        VisibleLogRow::SectionHeader(sec_ix) => {
                            let sec = &this.log_model.sections()[sec_ix];
                            let caret = if sec.collapsed { "▸" } else { "▾" };
                            let dur = sec
                                .duration_secs
                                .map(|d| format!("{:02}:{:02}", d / 60, d % 60))
                                .unwrap_or_default();
                            div()
                                .id(SharedString::from(format!("log-sec-{sec_ix}")))
                                .h(row_h)
                                .px_3()
                                .bg(rgb(0x23222B))
                                .hover(|s| s.bg(rgb(0x2E2C38)))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_between()
                                .font_family(mono.clone())
                                .text_size(mono_size)
                                .on_click(cx.listener(move |v, _, _, cx| v.toggle_section(sec_ix, cx)))
                                .child(div().text_color(rgb(0x6CDADA)).truncate().child(format!("{caret} {}", sec.header)))
                                .child(div().text_color(rgb(0x89888D)).child(dur))
                                .into_any_element()
                        }
                        VisibleLogRow::Line(line_ix) => {
                            let line = &this.log_model.lines()[line_ix];
                            let hit = active_hit
                                .filter(|h| h.line_ix == line_ix)
                                .map(|h| (h.start, h.end));
                            let runs = merge_highlight_runs(&line.spans, line.text.len(), hit);
                            let styled = StyledText::new(line.text.clone()).with_highlights(runs);
                            let content_el = if this.wrap_lines {
                                div().flex_1().min_w_0().truncate().child(styled).into_any_element()
                            } else {
                                div()
                                    .id(SharedString::from(format!("log-txt-{row_ix}")))
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_x_scroll()
                                    .whitespace_nowrap()
                                    .child(styled)
                                    .into_any_element()
                            };
                            div()
                                .id(SharedString::from(format!("log-ln-{row_ix}")))
                                .h(row_h)
                                .px_2()
                                .flex()
                                .items_center()
                                .gap_3()
                                .font_family(mono.clone())
                                .text_size(mono_size)
                                .bg(if selected { rgb(0x263B59) } else { log_bg })
                                .text_color(log_fg)
                                .on_click(cx.listener(move |v, ev: &gpui_kit::ClickEvent, _, cx| {
                                    v.select_log_row(row_ix, ev.modifiers().shift, cx);
                                }))
                                .child(
                                    div()
                                        .w(px(46.))
                                        .flex_shrink_0()
                                        .text_right()
                                        .text_color(rgb(0x6E6D75))
                                        .child(line.number.to_string()),
                                )
                                .child(content_el)
                                .into_any_element()
                        }
                    }
                })
                .collect()
        }),
    )
    .track_scroll(&view.log_scroll)
    .flex_1()
    .size_full();

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(search_bar)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .bg(log_bg)
                .on_scroll(cx.listener(|this, ev: &gpui_kit::ScrollEvent, _, cx| {
                    if ev.delta.pixel_delta(px(20.)).y > px(0.) {
                        this.pause_auto_scroll(cx);
                    }
                }))
                .child(list_el),
        )
        .into_any_element()
}
