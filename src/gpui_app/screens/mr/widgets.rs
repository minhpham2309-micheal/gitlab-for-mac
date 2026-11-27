use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, rgb, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{mr_can_review, MergeRequest, Pipeline};

use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::merge_widget::render_merge_widget;
use super::MergeRequestsView;

fn pipeline_verb(status: &str) -> &'static str {
    match status {
        "success" | "passed" => "passed",
        "failed" => "failed",
        "running" => "running",
        "pending" | "created" => "pending",
        "canceled" | "cancelled" => "canceled",
        "manual" => "waiting for manual action",
        _ => "finished",
    }
}

pub fn render_mr_widgets(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let pipe = mr.head_pipeline.clone().or_else(|| view.mr_pipelines.first().cloned());
    div()
        .id("mr-widgets-stack")
        .w_full().pb_4().flex().flex_col().gap_3()
        .children(pipe.map(|p| render_pipeline_widget(view, &p, cx)))
        .child(render_approval_widget(view, mr, cx))
        .child(render_merge_widget(view, mr, cx))
        .test_support()
}

fn render_pipeline_widget(
    view: &MergeRequestsView,
    pipe: &Pipeline,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let color = status_color(&pipe.status, dark);
    let verb = pipeline_verb(&pipe.status);
    let short_sha = pipe.sha.get(..8).unwrap_or(&pipe.sha).to_string();
    let rel = pipe.updated_at.as_deref().or(pipe.created_at.as_deref()).map(format_relative).unwrap_or_default();
    let pid = pipe.id;
    let st_open = view.state.clone();
    let st_graph = view.state.clone();

    let fallback = [if pipe.status == "success" { "success" } else { "pending" }];
    let stage_statuses: &[&'static str] = if view.cached_stages.is_empty() { &fallback } else { &view.cached_stages };

    let mut mini_graph = div()
        .id("mr-widget-pipeline-stages")
        .flex().items_center().cursor_pointer()
        .on_click(move |_, _, cx| st_graph.update(cx, |s, cx| s.open_pipeline(pid, cx)));
    for (ix, &st) in stage_statuses.iter().enumerate() {
        let sc = status_color(st, dark);
        if ix > 0 {
            mini_graph = mini_graph.child(div().w(px(6.)).h(px(1.5)).bg(theme.border));
        }
        mini_graph = mini_graph.child(Icon::empty().path(status_icon_path(st)).size(px(22.)).text_color(sc));
    }

    div()
        .id("mr-widget-pipeline")
        .rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
        .px_4().py_3().flex().items_center().justify_between().gap_3()
        .child(
            div().flex().items_center().gap_3().min_w_0()
                .child(Icon::empty().path(status_icon_path(&pipe.status)).size(px(22.)).text_color(color))
                .child(
                    div().flex().flex_col().gap_0p5().min_w_0()
                        .child(
                            div().flex().items_center().flex_wrap().gap_1().text_xs()
                                .child(div().font_semibold().text_color(theme.foreground).child("Merge request pipeline"))
                                .child(
                                    div().id("mr-pipe-id-link").cursor_pointer().font_semibold().text_color(theme.link)
                                        .hover(|s| s.text_color(theme.link_hover))
                                        .on_click(move |_, _, cx| st_open.update(cx, |s, cx| s.open_pipeline(pid, cx)))
                                        .child(format!("#{pid}")),
                                )
                                .child(div().font_semibold().text_color(theme.foreground).child(verb)),
                        )
                        .child(
                            div().flex().items_center().flex_wrap().gap_1().text_xs().text_color(theme.muted_foreground)
                                .child(format!("Merge request pipeline {verb} for"))
                                .child(div().font_family(theme.mono_font_family.clone()).text_color(theme.foreground).child(short_sha))
                                .children((!rel.is_empty()).then(|| div().child(rel)))
                                .child(Icon::new(IconName::Info).size_3().text_color(theme.muted_foreground)),
                        ),
                ),
        )
        .child(mini_graph)
        .test_support()
}

fn render_approval_widget(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let can_review = mr_can_review(&mr.state);
    let user_approved = view.approvals.as_ref().is_some_and(|a| a.user_has_approved);
    let status_text = match &view.approvals {
        Some(a) if !a.approved_by.is_empty() => {
            let names: Vec<String> = a.approved_by.iter().map(|u| u.user.username.clone()).collect();
            if a.approvals_left > 0 {
                format!("Approved by {} · {} more required", names.join(", "), a.approvals_left)
            } else {
                format!("Approved by {}", names.join(", "))
            }
        }
        Some(a) if a.approvals_required > 0 => format!("Requires {} approval(s)", a.approvals_left.max(a.approvals_required)),
        _ => "Approval is optional".into(),
    };
    let approve_bg: gpui_kit::Hsla = if dark { rgb(0x283E58).into() } else { rgb(0xE9F3FC).into() };
    let approve_border: gpui_kit::Hsla = if dark { rgb(0x428FDC).into() } else { theme.primary };
    let approve_fg: gpui_kit::Hsla = if dark { rgb(0x9DC7F1).into() } else { theme.primary };

    let mut card = div()
        .id("mr-widget-approval")
        .rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
        .flex().flex_col()
        .child(
            div().px_4().py_2p5().flex().items_center().justify_between().gap_3()
                .child(
                    div().flex().items_center().gap_3().min_w_0()
                        .child(Icon::new(IconName::User).size(px(20.)).text_color(theme.muted_foreground))
                        .children(can_review.then(|| {
                            let kind = if user_approved { "unapprove" } else { "approve" };
                            div().id("mr-widget-approve-btn").px_2p5().py_1().rounded_md().cursor_pointer()
                                .bg(approve_bg).border_1().border_color(approve_border).text_color(approve_fg)
                                .hover(|s| s.opacity(0.9)).text_xs().font_semibold()
                                .on_click(cx.listener(move |this, _, w, cx| this.perform_mr_mutation(kind, Some(w), cx)))
                                .child(if user_approved { "Revoke approval" } else { "Approve" }).test_support()
                        }))
                        .child(div().text_xs().text_color(theme.foreground).child(status_text))
                        .child(Icon::new(IconName::Info).size_3p5().text_color(theme.muted_foreground)),
                )
                .child(
                    div().pl_2().border_l_1().border_color(theme.border).child(
                        Button::new("mr-approval-expand").ghost().xsmall()
                            .icon(Icon::new(if view.approvals_expanded { IconName::ChevronUp } else { IconName::ChevronDown }).size_3p5())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.approvals_expanded = !this.approvals_expanded;
                                this.overview_list.remeasure_items(1..2);
                                cx.notify();
                            })),
                    ),
                ),
        );

    if view.approvals_expanded {
        let detail = match &view.approvals {
            Some(a) if !a.approved_by.is_empty() => format!(
                "Approvers: {} (required: {}, remaining: {})",
                a.approved_by.iter().map(|u| format!("@{}", u.user.username)).collect::<Vec<_>>().join(", "),
                a.approvals_required, a.approvals_left,
            ),
            _ => "No mandatory approval rules blocking this merge request.".into(),
        };
        card = card.child(div().px_4().py_2p5().border_t_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground).child(detail));
    }
    card.test_support()
}
