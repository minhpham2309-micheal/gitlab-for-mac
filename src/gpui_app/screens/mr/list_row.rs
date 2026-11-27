use gpui_kit::{
    base::StyledExt as _,
    component::{ActiveTheme as _, Icon, IconName},
    div, px, App, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, WeakEntity,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{GitLab, MergeRequest};

use super::super::super::avatar::render_avatar;
use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::MergeRequestsView;

pub fn render_mr_list_row(
    mr: &MergeRequest,
    client: Option<&GitLab>,
    view_weak: WeakEntity<MergeRequestsView>,
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

    let pipeline_icon = mr.head_pipeline.as_ref().map(|p| {
        let color = status_color(&p.status, dark);
        Icon::empty().path(status_icon_path(&p.status)).size(px(20.)).text_color(color)
    });

    let notes_badge = (mr.user_notes_count > 0).then(|| {
        div().flex().items_center().gap_1().text_xs().text_color(theme.muted_foreground)
            .child(Icon::new(IconName::Inbox).size_3())
            .child(mr.user_notes_count.to_string())
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
                        .child(div().flex().items_center().gap_2().children(avatars).children(notes_badge).children(pipeline_icon))
                        .children((!updated_rel.is_empty()).then(|| {
                            div().text_xs().text_color(theme.muted_foreground).child(format!("updated {updated_rel}"))
                        })),
                )
                .test_support(),
        )
}
