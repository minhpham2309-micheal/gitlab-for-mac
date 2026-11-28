use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        resizable::{resizable_panel, v_resizable},
        spinner::Spinner,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, ClipboardItem, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{
    format_hms_duration, format_pipeline_ref, pipeline_is_live, pipeline_kind_tag,
    pipeline_status_label, Pipeline, PipelineDetailTab,
};

use super::super::pajamas_theme::{status_badge_palette, status_icon_path};
use super::super::safe_url::safe_open_url;
use super::job_log::render_job_log;
use super::pipeline_graph::render_stage_graph;
use super::pipelines::PipelinesView;
use super::pipelines_jobs_table::{render_pipeline_jobs_table, render_pipeline_tests_tab};

pub fn render_pipeline_detail_view(
    view: &PipelinesView,
    pipe: &Pipeline,
    can_run_ci: bool,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let pid = pipe.id;
    let live = pipeline_is_live(&pipe.status);
    let badge = status_badge_palette(&pipe.status, dark);
    let detail = view.selected_detail.as_ref().filter(|d| d.id == pid);
    let created_rel = detail.and_then(|d| d.created_at.as_deref()).or(pipe.created_at.as_deref()).map(format_relative).unwrap_or_default();
    let finished_rel = detail.and_then(|d| d.finished_at.as_deref()).map(format_relative);
    let creator = detail.and_then(|d| d.user.as_ref()).map(|u| u.username.clone());
    let total_dur = detail.and_then(|d| d.duration).map(format_hms_duration);
    let queued = detail.and_then(|d| d.queued_duration).filter(|&q| q >= 1.0).map(|q| format!("queued for {}s", q.round() as u64));
    let short_sha: String = pipe.sha.chars().take(8).collect();
    let full_sha = pipe.sha.clone();
    let ref_label = format_pipeline_ref(&pipe.git_ref);
    let kind = pipeline_kind_tag(pipe);
    let web_url = pipe.web_url.clone();
    let jobs_count = view.jobs.len();
    let tests_count = view.selected_test_summary.as_ref().map(|t| t.total.count).unwrap_or(0);
    let copy_state = view.state.clone();

    let status_icon = if live {
        Spinner::new().xsmall().color(badge.icon).into_any_element()
    } else {
        Icon::empty().path(status_icon_path(&pipe.status)).size_3p5().text_color(badge.icon).into_any_element()
    };

    let (kind_bg, kind_fg) = {
        let s = status_badge_palette("running", dark);
        (s.bg, s.text)
    };
    let active_tab_border = theme.primary;
    let active_badge_bg = theme.primary;
    let active_badge_fg = theme.primary_foreground;
    let inactive_badge_bg: gpui_kit::Hsla = if dark { rgb(0x383839).into() } else { theme.secondary };
    let inactive_badge_fg: gpui_kit::Hsla = if dark { rgb(0xBFBFBF).into() } else { theme.muted_foreground };

    let mut meta_line = div().flex().items_center().flex_wrap().gap_2().text_xs().text_color(theme.muted_foreground)
        .child(
            div().px_2().py(px(2.)).rounded_full().bg(badge.bg).text_color(badge.text)
                .flex().items_center().gap_1().font_semibold()
                .child(status_icon).child(pipeline_status_label(&pipe.status)),
        );
    if !created_rel.is_empty() {
        let by = creator.map(|u| format!(" by {u}")).unwrap_or_default();
        let fin = finished_rel.map(|f| format!(", finished {f}")).unwrap_or_default();
        meta_line = meta_line.child(format!("Created {created_rel}{by}{fin}"));
    }

    let sub_line = div()
        .flex().items_center().flex_wrap().gap_2().text_xs().text_color(theme.muted_foreground)
        .child("For commit")
        .child(div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).font_family(theme.mono_font_family.clone()).text_color(theme.foreground).child(short_sha))
        .child(
            Button::new("pipe-copy-sha").ghost().xsmall().icon(Icon::new(IconName::Copy).size_3())
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone()));
                    copy_state.update(cx, |s, cx| s.set_status("Copied pipeline SHA", cx));
                }),
        )
        .child("In")
        .child(div().px_1p5().py(px(1.)).rounded_sm().bg(kind_bg).text_color(kind_fg).font_family(theme.mono_font_family.clone()).child(ref_label))
        .child(div().px_1p5().py(px(1.)).rounded_full().bg(kind_bg).text_color(kind_fg).font_semibold().child(kind))
        .child(format!("{jobs_count} jobs"))
        .children(total_dur.map(|d| div().child(format!("· {d}"))))
        .children(queued.map(|q| div().child(format!("({q})"))));

    let top_actions = div()
        .flex().items_center().justify_between().gap_3().pb_2()
        .child(
            Button::new("pipe-back-list").ghost().xsmall().icon(Icon::new(IconName::ArrowLeft).size_3p5()).label("All pipelines")
                .on_click(cx.listener(|this, _, _, cx| this.back_to_list(cx))),
        )
        .child(
            div().flex().items_center().gap_1p5()
                .child(Button::new("pipe-layout-toggle").outline().xsmall().label("3-pane").on_click(cx.listener(|this, _, _, cx| this.toggle_split_mode(cx))))
                .children(can_run_ci.then(|| {
                    if live {
                        Button::new("pipe-detail-cancel").danger().xsmall().label("Cancel")
                            .on_click(cx.listener(|this, _, w, cx| this.confirm_cancel_pipeline(w, cx)))
                    } else {
                        Button::new("pipe-detail-retry").outline().xsmall().label("Retry")
                            .on_click(cx.listener(|this, _, _, cx| this.run_pipeline_action("retry", cx)))
                    }
                }))
                .child(
                    Button::new("pipe-detail-web").ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3p5())
                        .on_click(move |_, _, _| { let _ = safe_open_url(&web_url); }),
                ),
        );

    let tabs_row = div()
        .mt_2().border_b_1().border_color(theme.border).flex().items_end().gap_1()
        .children(PipelineDetailTab::ALL.into_iter().enumerate().map(|(ix, tab)| {
            let active = view.detail_tab == tab;
            let count = match tab {
                PipelineDetailTab::Pipeline => None,
                PipelineDetailTab::Jobs => Some(jobs_count),
                PipelineDetailTab::Tests => Some(tests_count as usize),
            };
            let (bbg, bfg) = if active { (active_badge_bg, active_badge_fg) } else { (inactive_badge_bg, inactive_badge_fg) };
            div()
                .id(SharedString::from(format!("pipe-detail-tab-{ix}")))
                .px_3().py_2().cursor_pointer()
                .border_b_2().border_color(if active { active_tab_border } else { gpui_kit::transparent_black() })
                .flex().items_center().gap_1p5().text_sm()
                .text_color(if active { theme.foreground } else { theme.muted_foreground })
                .hover(|s| s.text_color(theme.foreground))
                .on_click(cx.listener(move |this, _, _, cx| { this.detail_tab = tab; cx.notify(); }))
                .child(div().when(active, |d| d.font_semibold()).child(tab.label()))
                .children(count.map(|n| {
                    div().px_1p5().py(px(1.)).rounded_full().bg(bbg).text_color(bfg).text_xs().font_semibold().child(n.to_string())
                }))
                .test_support()
        }));

    let header = div()
        .id("pipe-detail-header")
        .w_full().px_6().pt_3().flex_shrink_0().flex().flex_col().gap_1p5()
        .child(top_actions)
        .child(div().text_2xl().font_bold().text_color(theme.foreground).child(format!("#{pid}")))
        .child(meta_line)
        .child(sub_line)
        .child(tabs_row)
        .test_support();

    let tab_content = match view.detail_tab {
        PipelineDetailTab::Pipeline => div().size_full().p_4().child(
            div().size_full().rounded_lg().border_1().border_color(theme.border).bg(theme.list_head.opacity(0.35)).overflow_hidden().child(
                render_stage_graph(&view.jobs, &view.bridges, view.selected_job.as_ref().map(|j| j.id), view.graph_zoom_pct, can_run_ci, cx),
            ),
        ).into_any_element(),
        PipelineDetailTab::Jobs => render_pipeline_jobs_table(&view.jobs, view.selected_job.as_ref().map(|j| j.id), can_run_ci, cx).into_any_element(),
        PipelineDetailTab::Tests => render_pipeline_tests_tab(view.selected_test_summary.as_ref(), cx).into_any_element(),
    };

    let body = if view.selected_job.is_some() {
        let log_view = render_job_log(view, cx);
        v_resizable("pipe-detail-log-split")
            .child(resizable_panel().size(px(300.)).size_range(px(160.)..px(560.)).child(tab_content))
            .child(resizable_panel().child(log_view))
            .into_any_element()
    } else {
        div().flex_1().min_h_0().w_full().child(tab_content).into_any_element()
    };

    div().id("pipeline-detail-view").size_full().flex().flex_col().child(header).child(div().flex_1().min_h_0().w_full().flex().flex_col().child(body)).test_support()
}
