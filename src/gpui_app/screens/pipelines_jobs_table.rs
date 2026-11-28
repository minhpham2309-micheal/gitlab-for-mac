use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, uniform_list, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{
    format_hms_duration, job_is_live, pipeline_status_label, Job, PipelineTestSummary,
};

use super::super::pajamas_theme::{status_badge_palette, status_icon_path};
use super::pipelines::PipelinesView;

pub fn render_pipeline_jobs_table(
    jobs: &[Job],
    selected_job_id: Option<u64>,
    can_run_ci: bool,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    if jobs.is_empty() {
        return div()
            .id("pipe-jobs-empty")
            .size_full().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
            .child("No jobs found for this pipeline.")
            .test_support()
            .into_any_element();
    }

    let items: Vec<Job> = jobs.to_vec();
    let weak = cx.entity().downgrade();
    let head = div()
        .w_full().px_6().py_2p5().border_b_1().border_color(theme.border)
        .flex().items_center().gap_4().text_xs().font_semibold().text_color(theme.foreground)
        .child(div().w(px(124.)).flex_shrink_0().child("Status"))
        .child(div().flex_1().min_w_0().child("Job"))
        .child(div().w(px(130.)).flex_shrink_0().child("Stage"))
        .child(div().w(px(92.)).flex_shrink_0().text_right().child("Duration"))
        .child(div().w(px(96.)).flex_shrink_0().text_right().child("Actions"));

    let list_el = uniform_list("pipe-jobs-table-list", items.len(), move |range, _, cx| {
        let theme = cx.theme();
        let dark = theme.is_dark();
        range.map(|ix| {
            let j = items[ix].clone();
            let jid = j.id;
            let active = selected_job_id == Some(jid);
            let live = job_is_live(&j.status);
            let badge = status_badge_palette(&j.status, dark);
            let dur = j.duration.map(|s| format_hms_duration(s.max(0.0).round() as u64)).unwrap_or_else(|| "—".into());
            let icon = if live {
                Spinner::new().xsmall().color(badge.icon).into_any_element()
            } else {
                Icon::empty().path(status_icon_path(&j.status)).size_3p5().text_color(badge.icon).into_any_element()
            };
            let (w_sel, w_act, w_dl) = (weak.clone(), weak.clone(), weak.clone());
            let (j_click, j_play) = (j.clone(), j.clone());
            let is_manual = j.status == "manual";

            div()
                .id(SharedString::from(format!("pipe-jobs-row-{jid}")))
                .w_full().h(px(52.)).px_6().border_b_1().border_color(theme.border.opacity(0.55))
                .cursor_pointer().bg(if active { theme.list_active } else { gpui_kit::transparent_black() })
                .hover(|s| s.bg(theme.list_hover.opacity(0.6)))
                .flex().items_center().gap_4()
                .on_click(move |_, _, cx| {
                    let _ = w_sel.update(cx, |v, cx| v.select_job(j_click.clone(), cx));
                })
                .child(
                    div().w(px(124.)).flex_shrink_0().flex().items_center().child(
                        div().px_2().py(px(2.)).rounded_full().bg(badge.bg).text_color(badge.text)
                            .flex().items_center().gap_1().text_xs().font_semibold()
                            .child(icon).child(pipeline_status_label(&j.status)),
                    ),
                )
                .child(
                    div().flex_1().min_w_0().flex().items_center().gap_2()
                        .child(div().text_sm().font_semibold().text_color(theme.foreground).truncate().child(j.name.clone()))
                        .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(format!("#{jid}"))),
                )
                .child(div().w(px(130.)).flex_shrink_0().text_xs().text_color(theme.muted_foreground).truncate().child(j.stage.clone()))
                .child(div().w(px(92.)).flex_shrink_0().text_right().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(dur))
                .child(
                    div().w(px(96.)).flex_shrink_0().flex().items_center().justify_end().gap_1()
                        .children(can_run_ci.then(|| {
                            if is_manual {
                                Button::new(SharedString::from(format!("pj-play-{jid}"))).outline().xsmall().label("Play")
                                    .on_click(move |_, w, cx| {
                                        cx.stop_propagation();
                                        let _ = w_act.update(cx, |v, cx| v.confirm_play_job(j_play.clone(), w, cx));
                                    })
                            } else {
                                let act = if live { "cancel" } else { "retry" };
                                let ic = if live { IconName::Close } else { IconName::RefreshCw };
                                Button::new(SharedString::from(format!("pj-act-{jid}"))).outline().xsmall().icon(Icon::new(ic).size_3())
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        let _ = w_act.update(cx, |v, cx| v.run_job_action(jid, act, cx));
                                    })
                            }
                        }))
                        .child(
                            Button::new(SharedString::from(format!("pj-art-{jid}"))).ghost().xsmall().icon(Icon::new(IconName::Folder).size_3())
                                .on_click(move |_, w, cx| {
                                    cx.stop_propagation();
                                    let _ = w_dl.update(cx, |v, cx| v.download_artifacts(jid, w, cx));
                                }),
                        ),
                )
                .test_support()
        }).collect()
    })
    .flex_1()
    .size_full();

    div().id("pipe-jobs-table").size_full().flex().flex_col().child(head).child(list_el).test_support().into_any_element()
}

pub fn render_pipeline_tests_tab(
    summary: Option<&PipelineTestSummary>,
    cx: &Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let Some(sum) = summary else {
        return div()
            .id("pipe-tests-empty").size_full().flex().items_center().justify_center()
            .text_sm().text_color(theme.muted_foreground)
            .child("No JUnit test report available for this pipeline.")
            .test_support()
            .into_any_element();
    };
    let t = &sum.total;
    div()
        .id("pipe-tests-summary").size_full().overflow_y_scroll().p_6().flex().flex_col().gap_4()
        .child(
            div().p_4().rounded_lg().border_1().border_color(theme.border).bg(theme.list_head)
                .flex().items_center().gap_6().text_xs()
                .child(div().font_semibold().text_color(theme.foreground).child(format!("Total: {}", t.count)))
                .child(div().text_color(theme.success).child(format!("Passed: {}", t.success)))
                .child(div().text_color(theme.danger).child(format!("Failed: {}", t.failed)))
                .child(div().text_color(theme.warning).child(format!("Errors: {}", t.error)))
                .child(div().text_color(theme.muted_foreground).child(format!("Skipped: {} · {:.1}s", t.skipped, t.time))),
        )
        .children(sum.test_suites.iter().enumerate().map(|(ix, s)| {
            div().id(SharedString::from(format!("pipe-suite-{ix}"))).px_4().py_2p5().rounded_md()
                .border_1().border_color(theme.border).bg(theme.background)
                .flex().items_center().justify_between().text_xs()
                .child(div().font_semibold().text_color(theme.foreground).child(s.name.clone()))
                .child(div().text_color(theme.muted_foreground).child(format!("{}/{} passed · {:.1}s", s.success_count, s.total_count, s.total_time)))
        }))
        .test_support()
        .into_any_element()
}
