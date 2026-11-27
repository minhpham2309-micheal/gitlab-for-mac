use gpui_kit::{
    base::StyledExt as _,
    component::{ActiveTheme as _, Icon, IconName},
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{GitLab, MergeRequest, User};

use super::super::super::avatar::render_avatar;
use super::super::super::pajamas_theme::status_color;
use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

fn section_header(
    title: String,
    edit_id: &'static str,
    edit_url: String,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    div()
        .flex().items_center().justify_between().gap_2()
        .child(div().text_xs().font_semibold().text_color(theme.foreground).child(title))
        .child(
            div()
                .id(edit_id)
                .cursor_pointer()
                .text_xs()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .on_click(move |_, _, _| { let _ = safe_open_url(&edit_url); })
                .child("Edit"),
        )
}

pub fn render_mr_metadata_sidebar(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let edit_url = format!("{}/edit", mr.web_url.trim_end_matches('/'));
    let approved_ids: Vec<u64> = view
        .approvals
        .as_ref()
        .map(|a| a.approved_by.iter().map(|u| u.user.id).collect())
        .unwrap_or_default();

    let assignee_rows: Vec<_> = mr
        .assignees
        .iter()
        .map(|u| {
            let av = render_avatar(u.avatar_url.as_deref(), &u.username, px(22.), client, cx);
            let fg = cx.theme().foreground;
            div().flex().items_center().gap_2().child(av)
                .child(div().text_xs().text_color(fg).truncate().child(u.name.clone()))
        })
        .collect();

    let reviewer_rows: Vec<_> = mr
        .reviewers
        .iter()
        .map(|u| {
            let av = render_avatar(u.avatar_url.as_deref(), &u.username, px(22.), client, cx);
            let approved = approved_ids.contains(&u.id);
            let theme = cx.theme();
            let ok = status_color("success", theme.is_dark());
            div().flex().items_center().justify_between().gap_2()
                .child(div().flex().items_center().gap_2().min_w_0().child(av)
                    .child(div().text_xs().text_color(theme.foreground).truncate().child(u.name.clone())))
                .children(approved.then(|| Icon::new(IconName::CircleCheck).size_3p5().text_color(ok)))
        })
        .collect();

    let mut parts: Vec<User> = view.participants.clone();
    if parts.is_empty() && let Some(author) = mr.author.clone() {
        parts.push(author);
    }
    let participant_count = parts.len();
    let participant_avs: Vec<_> = parts
        .iter()
        .take(12)
        .map(|u| render_avatar(u.avatar_url.as_deref(), &u.username, px(24.), client, cx))
        .collect();

    let theme = cx.theme();
    let border = theme.border.opacity(0.6);
    let muted = theme.muted_foreground;
    let link = theme.link;

    let assignees_body = if assignee_rows.is_empty() {
        div().flex().items_center().gap_1().text_xs().text_color(muted)
            .child("None -")
            .child(
                div().id("mr-assign-self").cursor_pointer().text_color(link)
                    .hover(|s| s.text_color(theme.link_hover))
                    .on_click(cx.listener(|this, _, _, cx| this.assign_self(false, cx)))
                    .child("assign yourself").test_support(),
            )
            .into_any_element()
    } else {
        div().flex().flex_col().gap_1p5().children(assignee_rows).into_any_element()
    };

    let reviewers_body = if reviewer_rows.is_empty() {
        div().flex().items_center().gap_1().text_xs().text_color(muted)
            .child("None -")
            .child(
                div().id("mr-reviewer-self").cursor_pointer().text_color(link)
                    .hover(|s| s.text_color(theme.link_hover))
                    .on_click(cx.listener(|this, _, _, cx| this.assign_self(true, cx)))
                    .child("assign yourself").test_support(),
            )
            .into_any_element()
    } else {
        div().flex().flex_col().gap_1p5().children(reviewer_rows).into_any_element()
    };

    let labels_body = if mr.labels.is_empty() {
        div().text_xs().text_color(muted).child("None").into_any_element()
    } else {
        div().flex().flex_wrap().gap_1().children(mr.labels.iter().map(|lbl| {
            div().px_2().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border)
                .text_xs().text_color(theme.foreground).child(lbl.clone())
        })).into_any_element()
    };

    let milestone_text = mr.milestone.as_ref().map(|m| m.title.clone()).unwrap_or_else(|| "None".into());
    let milestone_color = if mr.milestone.is_some() { theme.foreground } else { muted };
    let time_text = mr.time_stats.as_ref().and_then(|t| t.summary()).unwrap_or_else(|| "No estimate or time spent".into());
    let time_url = edit_url.clone();

    div()
        .id("mr-metadata-sidebar")
        .w(px(268.)).flex_shrink_0().flex().flex_col()
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(section_header(
                    format!("{} {}", mr.assignees.len(), if mr.assignees.len() == 1 { "Assignee" } else { "Assignees" }),
                    "mr-sb-edit-assignees", edit_url.clone(), theme,
                ))
                .child(assignees_body),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(section_header(
                    format!("{} {}", mr.reviewers.len(), if mr.reviewers.len() == 1 { "Reviewer" } else { "Reviewers" }),
                    "mr-sb-edit-reviewers", edit_url.clone(), theme,
                ))
                .child(reviewers_body),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(section_header("Labels".into(), "mr-sb-edit-labels", edit_url.clone(), theme))
                .child(labels_body),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(section_header("Milestone".into(), "mr-sb-edit-milestone", edit_url, theme))
                .child(div().text_xs().text_color(milestone_color).child(milestone_text)),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(
                    div().flex().items_center().justify_between().gap_2()
                        .child(div().text_xs().font_semibold().text_color(theme.foreground).child("Time tracking"))
                        .child(
                            div().id("mr-sb-time-add").cursor_pointer().flex().items_center().gap_1()
                                .text_color(muted).hover(|s| s.text_color(theme.foreground))
                                .on_click(move |_, _, _| { let _ = safe_open_url(&time_url); })
                                .child(Icon::new(IconName::Calendar).size_3p5())
                                .child(Icon::new(IconName::Plus).size_3()),
                        ),
                )
                .child(div().text_xs().text_color(muted).child(time_text)),
        )
        .child(
            div().py_3().flex().flex_col().gap_2()
                .child(div().text_xs().font_semibold().text_color(theme.foreground).child(format!(
                    "{participant_count} {}",
                    if participant_count == 1 { "Participant" } else { "Participants" },
                )))
                .child(div().flex().flex_wrap().gap_1p5().children(participant_avs)),
        )
        .test_support()
}
