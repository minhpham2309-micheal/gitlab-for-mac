use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        ActiveTheme as _, Selectable as _, Sizable as _,
    },
    div, px, Context, Entity, IntoElement, ParentElement as _, Styled as _,
};

use crate::gitlab::log_model::LogSearchHit;
use crate::gitlab::{job_is_live, Job};

use super::super::pajamas_theme::status_color;
use super::super::safe_url::safe_open_url;
use super::pipelines::PipelinesView;

#[allow(clippy::too_many_arguments)]
pub fn render_job_log_header(
    job: &Job,
    line_count: usize,
    auto_scroll: bool,
    wrap_lines: bool,
    has_selection: bool,
    can_run_ci: bool,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let job_id = job.id;
    let job_for_play = job.clone();
    let web_url = job.web_url.clone();
    let live = job_is_live(&job.status);

    div()
        .px_3()
        .py_1p5()
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.list_head)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .px_1p5()
                        .rounded_sm()
                        .bg(status_color(&job.status, dark).opacity(0.2))
                        .text_color(status_color(&job.status, dark))
                        .text_xs()
                        .child(job.status.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.foreground)
                        .truncate()
                        .child(format!("{} · {}", job.stage, job.name)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{line_count} lines")),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .flex_shrink_0()
                .child(
                    Button::new("log-follow-btn")
                        .ghost()
                        .xsmall()
                        .selected(auto_scroll)
                        .label("Follow")
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_scroll(cx))),
                )
                .child(
                    Button::new("log-wrap-btn")
                        .ghost()
                        .xsmall()
                        .selected(wrap_lines)
                        .label("Wrap")
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_wrap(cx))),
                )
                .children(has_selection.then(|| {
                    Button::new("log-copy-sel")
                        .outline()
                        .xsmall()
                        .label("Copy sel")
                        .on_click(cx.listener(|this, _, _, cx| this.copy_selected_lines(cx)))
                }))
                .child(
                    Button::new("log-copy-all")
                        .ghost()
                        .xsmall()
                        .label("Copy")
                        .on_click(cx.listener(|this, _, _, cx| this.copy_all_log(cx))),
                )
                .children(can_run_ci.then(|| {
                    Button::new("log-retry")
                        .ghost()
                        .xsmall()
                        .label("Retry")
                        .on_click(cx.listener(move |this, _, _, cx| this.run_job_action(job_id, "retry", cx)))
                }))
                .children((can_run_ci && live).then(|| {
                    Button::new("log-cancel")
                        .danger()
                        .xsmall()
                        .label("Cancel")
                        .on_click(cx.listener(move |this, _, _, cx| this.run_job_action(job_id, "cancel", cx)))
                }))
                .children((can_run_ci && job.status == "manual").then(|| {
                    Button::new("log-play")
                        .primary()
                        .xsmall()
                        .label("Play")
                        .on_click(cx.listener(move |this, _, w, cx| {
                            this.confirm_play_job(job_for_play.clone(), w, cx);
                        }))
                }))
                .child(
                    Button::new("log-artifacts")
                        .ghost()
                        .xsmall()
                        .label("Artifacts")
                        .on_click(cx.listener(move |this, _, w, cx| this.download_artifacts(job_id, w, cx))),
                )
                .child(
                    Button::new("log-open-web")
                        .ghost()
                        .xsmall()
                        .label("Web")
                        .on_click(move |_, _, _| {
                            let _ = safe_open_url(&web_url);
                        }),
                ),
        )
}

pub fn render_job_log_search_bar(
    search_input: &Entity<InputState>,
    search_hits: &[LogSearchHit],
    search_idx: usize,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let hits_label = if search_hits.is_empty() {
        String::new()
    } else {
        format!("{}/{}", search_idx + 1, search_hits.len())
    };
    div()
        .px_3()
        .py_1()
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.list_head)
        .flex()
        .items_center()
        .gap_2()
        .child(div().w(px(280.)).child(Input::new(search_input)))
        .child(div().text_xs().text_color(theme.muted_foreground).child(hits_label))
        .child(
            Button::new("log-find-prev")
                .ghost()
                .xsmall()
                .label("Prev")
                .on_click(cx.listener(|this, _, _, cx| this.jump_search(false, cx))),
        )
        .child(
            Button::new("log-find-next")
                .ghost()
                .xsmall()
                .label("Next")
                .on_click(cx.listener(|this, _, _, cx| this.jump_search(true, cx))),
        )
}
