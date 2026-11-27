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
use super::member_picker::{render_member_picker, MemberPickerKind};
use super::MergeRequestsView;

fn web_section_header(
    title: String,
    edit_id: &'static str,
    edit_url: String,
    can_edit: bool,
    theme: &gpui_kit::component::Theme,
) -> impl IntoElement {
    div()
        .flex().items_center().justify_between().gap_2()
        .child(div().text_xs().font_semibold().text_color(theme.foreground).child(title))
        .children(can_edit.then(|| {
            div().id(edit_id).cursor_pointer().text_xs().text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .on_click(move |_, _, _| { let _ = safe_open_url(&edit_url); })
                .child("Edit").test_support()
        }))
}

fn member_section_header(
    title: String,
    edit_id: &'static str,
    kind: MemberPickerKind,
    open: bool,
    can_edit: bool,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    div()
        .flex().items_center().justify_between().gap_2()
        .child(div().text_xs().font_semibold().text_color(theme.foreground).child(title))
        .children(can_edit.then(|| {
            div()
                .id(edit_id).cursor_pointer().text_xs()
                .text_color(if open { theme.link } else { theme.muted_foreground })
                .hover(|s| s.text_color(theme.foreground))
                .on_click(cx.listener(move |this, _, window, cx| this.toggle_member_picker(kind, window, cx)))
                .child(if open { "Done" } else { "Edit" })
                .test_support()
        }))
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

    let can_edit = view.mr_permissions(mr, cx).can_edit_metadata;
    let assignees_open = can_edit && view.member_picker == Some(MemberPickerKind::Assignees);
    let reviewers_open = can_edit && view.member_picker == Some(MemberPickerKind::Reviewers);
    let assignees_picker = assignees_open.then(|| render_member_picker(view, mr, MemberPickerKind::Assignees, client, cx));
    let reviewers_picker = reviewers_open.then(|| render_member_picker(view, mr, MemberPickerKind::Reviewers, client, cx));
    let assignees_hdr = member_section_header(
        format!("{} {}", mr.assignees.len(), if mr.assignees.len() == 1 { "Assignee" } else { "Assignees" }),
        "mr-sb-edit-assignees", MemberPickerKind::Assignees, assignees_open, can_edit, cx,
    );
    let reviewers_hdr = member_section_header(
        format!("{} {}", mr.reviewers.len(), if mr.reviewers.len() == 1 { "Reviewer" } else { "Reviewers" }),
        "mr-sb-edit-reviewers", MemberPickerKind::Reviewers, reviewers_open, can_edit, cx,
    );

    let theme = cx.theme();
    let border = theme.border.opacity(0.6);
    let muted = theme.muted_foreground;
    let link = theme.link;

    let assignees_body = if !assignee_rows.is_empty() {
        div().flex().flex_col().gap_1p5().children(assignee_rows).into_any_element()
    } else if can_edit {
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
        div().text_xs().text_color(muted).child("None").into_any_element()
    };

    let reviewers_body = if !reviewer_rows.is_empty() {
        div().flex().flex_col().gap_1p5().children(reviewer_rows).into_any_element()
    } else if can_edit {
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
        div().text_xs().text_color(muted).child("None").into_any_element()
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
                .child(assignees_hdr).child(assignees_body).children(assignees_picker),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(reviewers_hdr).child(reviewers_body).children(reviewers_picker),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(web_section_header("Labels".into(), "mr-sb-edit-labels", edit_url.clone(), can_edit, theme))
                .child(labels_body),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(web_section_header("Milestone".into(), "mr-sb-edit-milestone", edit_url, can_edit, theme))
                .child(div().text_xs().text_color(milestone_color).child(milestone_text)),
        )
        .child(
            div().py_3().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(
                    div().flex().items_center().justify_between().gap_2()
                        .child(div().text_xs().font_semibold().text_color(theme.foreground).child("Time tracking"))
                        .children(can_edit.then(|| {
                            div().id("mr-sb-time-add").cursor_pointer().flex().items_center().gap_1()
                                .text_color(muted).hover(|s| s.text_color(theme.foreground))
                                .on_click(move |_, _, _| { let _ = safe_open_url(&time_url); })
                                .child(Icon::new(IconName::Calendar).size_3p5())
                                .child(Icon::new(IconName::Plus).size_3())
                                .test_support()
                        })),
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
