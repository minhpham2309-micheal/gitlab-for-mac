use gpui_kit::{
    base::StyledExt as _,
    component::{spinner::Spinner, ActiveTheme as _, Icon, IconName, Sizable as _},
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, WeakEntity,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{pipeline_is_live, GitLab, MergeRequest};

use super::super::super::avatar::render_avatar;
use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::super::super::state::AppState;
use super::MergeRequestsView;

pub(crate) fn ci_status_label(status: &str) -> &str {
    match status {
        "success" | "passed" => "passed",
        "failed" => "failed",
        "running" => "running",
        "pending" => "pending",
        "created" => "created",
        "preparing" => "preparing",
        "waiting_for_resource" | "waiting_for_callback" => "waiting",
        "canceling" => "canceling",
        "canceled" | "cancelled" => "canceled",
        "manual" => "manual",
        "skipped" => "skipped",
        other => other,
    }
}

pub fn render_mr_list_row(
    mr: &MergeRequest,
    client: Option<&GitLab>,
    view_weak: WeakEntity<MergeRequestsView>,
    app_state: Entity<AppState>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let clicked = mr.clone();
    let author = mr.author.as_ref().map(|a| a.username.as_str()).unwrap_or("unknown");
    let created_rel = mr.created_at.as_deref().map(format_relative).unwrap_or_default();
    let updated_rel = mr.updated_at.as_deref().map(format_relative).unwrap_or_default();
    let subtitle = if created_rel.is_empty() {
        format!("!{} · by {author}", mr.iid)
    } else {
        format!("!{} · created {created_rel} by {author}", mr.iid)
    };

    let avatars: Vec<_> = mr
        .assignees
        .iter()
        .chain(mr.reviewers.iter())
        .take(3)
        .map(|u| render_avatar(u.avatar_url.as_deref(), &u.username, px(20.), client, cx))
        .collect();

    let theme = cx.theme();
    let dark = theme.is_dark();

    let mut meta_row = div()
        .flex().items_center().gap_2().text_xs().text_color(theme.muted_foreground)
        .child(div().truncate().child(subtitle));

    if let Some(ms) = &mr.milestone {
        meta_row = meta_row.child(
            div().flex().items_center().gap_1().flex_shrink_0()
                .child(Icon::new(IconName::Calendar).size_3())
                .child(ms.title.clone()),
        );
    }
    for lbl in mr.labels.iter().take(4) {
        meta_row = meta_row.child(
            div().px_1p5().py(px(1.)).rounded_full().flex_shrink_0()
                .bg(theme.secondary).border_1().border_color(theme.border)
                .text_xs().text_color(theme.foreground).child(lbl.clone()),
        );
    }

    let pipeline_badge = mr.head_pipeline.as_ref().map(|p| {
        let color = status_color(&p.status, dark);
        let live = pipeline_is_live(&p.status);
        let pid = p.id;
        let state_open = app_state.clone();
        let indicator = if live {
            Spinner::new().xsmall().color(color).into_any_element()
        } else {
            Icon::empty().path(status_icon_path(&p.status)).size_3p5().text_color(color).into_any_element()
        };
        let label = ci_status_label(&p.status).to_string();
        div()
            .id(SharedString::from(format!("mr-list-ci-{}", mr.iid)))
            .px_2().py(px(2.)).rounded_full().flex_shrink_0()
            .bg(color.opacity(if dark { 0.16 } else { 0.12 }))
            .border_1().border_color(color.opacity(0.35))
            .cursor_pointer().hover(|s| s.opacity(0.85))
            .flex().items_center().gap_1()
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                state_open.update(cx, |s, cx| s.open_pipeline(pid, cx));
            })
            .child(indicator)
            .child(div().text_xs().font_medium().text_color(color).child(label))
            .test_support()
    });

    let notes_badge = (mr.user_notes_count > 0).then(|| {
        div().flex().items_center().gap_1().text_xs().text_color(theme.muted_foreground)
            .child(Icon::new(IconName::Inbox).size_3())
            .child(mr.user_notes_count.to_string())
    });

    let top_right = (!avatars.is_empty() || notes_badge.is_some()).then(|| {
        div().flex().items_center().gap_2().children(avatars).children(notes_badge)
    });

    let mut title_line = div()
        .flex().items_center().gap_2().min_w_0()
        .child(div().text_sm().font_semibold().text_color(theme.foreground).truncate().child(mr.title.clone()));
    if mr.draft {
        title_line = title_line.child(
            div().px_1p5().py(px(1.)).rounded_full().flex_shrink_0()
                .bg(status_color("warning", dark).opacity(0.18))
                .text_color(status_color("warning", dark)).text_xs().child("Draft"),
        );
    }

    div()
        .w_full().h(px(68.)).px_6().flex().flex_col().items_center()
        .child(
            div()
                .id(SharedString::from(format!("mr-list-row-{}", mr.iid)))
                .w_full().max_w(px(1120.)).h_full().px_2()
                .border_b_1().border_color(theme.border.opacity(0.6))
                .cursor_pointer().hover(|s| s.bg(theme.list_hover.opacity(0.45)))
                .flex().items_center().justify_between().gap_4()
                .on_click(move |_, _, cx| {
                    let _ = view_weak.update(cx, |this, cx| this.select_mr(clicked.clone(), true, cx));
                })
                .child(
                    div().flex_1().min_w_0().flex().flex_col().justify_center().gap_1()
                        .child(title_line).child(meta_row),
                )
                .child(
                    div().flex().flex_col().items_end().justify_center().gap_1().flex_shrink_0()
                        .children(top_right)
                        .child(
                            div()
                                .flex().items_center().gap_2().flex_shrink_0()
                                .children(pipeline_badge)
                                .children((!updated_rel.is_empty()).then(|| {
                                    div()
                                        .id(SharedString::from(format!("mr-list-updated-{}", mr.iid)))
                                        .text_xs().text_color(theme.muted_foreground)
                                        .child(format!("updated {updated_rel}"))
                                        .test_support()
                                })),
                        ),
                )
                .test_support(),
        )
}
