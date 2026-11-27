use gpui_kit::{
    base::StyledExt as _,
    component::{ActiveTheme as _, Icon, IconName},
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{GitLab, MergeRequest};

use super::super::super::avatar::render_avatar;
use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::MergeRequestsView;

pub fn render_mr_list_row(
    mr: &MergeRequest,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let clicked = mr.clone();
    let author = mr
        .author
        .as_ref()
        .map(|a| a.username.as_str())
        .unwrap_or("unknown");
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
        .flex()
        .items_center()
        .flex_wrap()
        .gap_2()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(subtitle);

    if let Some(ms) = &mr.milestone {
        meta_row = meta_row.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(Icon::new(IconName::Calendar).size_3())
                .child(ms.title.clone()),
        );
    }

    let labels_row = (!mr.labels.is_empty()).then(|| {
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_1()
            .pt_0p5()
            .children(mr.labels.iter().take(6).map(|lbl| {
                div()
                    .px_2()
                    .py(px(1.))
                    .rounded_full()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .text_xs()
                    .text_color(theme.foreground)
                    .child(lbl.clone())
            }))
    });

    let pipeline_icon = mr.head_pipeline.as_ref().map(|p| {
        let color = status_color(&p.status, dark);
        Icon::empty().path(status_icon_path(&p.status)).size(px(20.)).text_color(color)
    });

    let notes_badge = (mr.user_notes_count > 0).then(|| {
        div()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(Icon::new(IconName::Inbox).size_3())
            .child(mr.user_notes_count.to_string())
    });

    let mut title_line = div()
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(theme.foreground)
                .truncate()
                .child(mr.title.clone()),
        );
    if mr.draft {
        title_line = title_line.child(
            div()
                .px_1p5()
                .py(px(1.))
                .rounded_full()
                .bg(status_color("warning", dark).opacity(0.18))
                .text_color(status_color("warning", dark))
                .text_xs()
                .child("Draft"),
        );
    }

    div()
        .id(SharedString::from(format!("mr-list-row-{}", mr.iid)))
        .w_full()
        .px_2()
        .py_3()
        .border_b_1()
        .border_color(theme.border.opacity(0.6))
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover.opacity(0.45)))
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .on_click(cx.listener(move |this, _, _, cx| this.select_mr(clicked.clone(), true, cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(title_line)
                .child(meta_row)
                .children(labels_row),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap_1()
                .flex_shrink_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .children(avatars)
                        .children(notes_badge)
                        .children(pipeline_icon),
                )
                .children((!updated_rel.is_empty()).then(|| {
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("updated {updated_rel}"))
                })),
        )
        .test_support()
}
