use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        checkbox::Checkbox,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, rgb, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{mr_can_review, mr_merge_readiness, MergeReadinessTone, MergeRequest};

use super::super::super::pajamas_theme::status_color;
use super::MergeRequestsView;

pub fn render_merge_widget(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let readiness = mr_merge_readiness(mr);
    let can_review = mr_can_review(&mr.state);
    let behind = mr.diverged_commits_count.unwrap_or(0);
    let commits_n = view.commits.len();
    let check_fg: gpui_kit::Hsla = if dark { rgb(0x18171D).into() } else { rgb(0xFFFFFF).into() };

    let status_badge = match readiness.tone {
        MergeReadinessTone::Ready => div()
            .size(px(20.)).rounded_full().bg(status_color("success", dark)).flex_shrink_0()
            .flex().items_center().justify_center()
            .child(Icon::new(IconName::Check).size_3().text_color(check_fg))
            .into_any_element(),
        MergeReadinessTone::Merged => div()
            .size(px(20.)).rounded_full().bg(status_color("running", dark)).flex_shrink_0()
            .flex().items_center().justify_center()
            .child(Icon::empty().path("icons/glab-mr.svg").size_3().text_color(check_fg))
            .into_any_element(),
        MergeReadinessTone::Blocked | MergeReadinessTone::Closed => {
            Icon::new(IconName::CircleX).size(px(20.)).text_color(status_color("failed", dark)).into_any_element()
        }
        MergeReadinessTone::Warning => {
            Icon::new(IconName::CircleAlert).size(px(20.)).text_color(status_color("warning", dark)).into_any_element()
        }
    };

    let mut card = div()
        .id("mr-widget-merge")
        .rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
        .flex().flex_col()
        .child(
            div().px_4().py_3().flex().items_center().justify_between().gap_3()
                .child(
                    div().flex().items_center().gap_3().min_w_0()
                        .child(status_badge)
                        .child(div().text_xs().font_semibold().text_color(theme.foreground).child(readiness.headline)),
                )
                .child(
                    div().flex().items_center().gap_2().flex_shrink_0()
                        .children((can_review && readiness.can_merge_now).then(|| {
                            Button::new("mr-widget-merge-btn").primary().xsmall().label("Merge")
                                .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("merge", Some(w), cx)))
                        }))
                        .child(
                            div().pl_2().border_l_1().border_color(theme.border).child(
                                Button::new("mr-merge-expand").ghost().xsmall()
                                    .icon(Icon::new(if view.merge_details_expanded { IconName::ChevronUp } else { IconName::ChevronDown }).size_3p5())
                                    .on_click(cx.listener(|this, _, _, cx| { this.merge_details_expanded = !this.merge_details_expanded; cx.notify(); })),
                            ),
                        ),
                ),
        );

    if view.merge_details_expanded && can_review {
        let noun = if commits_n == 1 { "commit" } else { "commits" };
        let commit_line = if view.squash_commits {
            format!("• {commits_n} {noun} will be squashed into 1 commit on {}.", mr.target_branch)
        } else {
            format!("• {commits_n} {noun} and 1 merge commit will be added to {}.", mr.target_branch)
        };
        let branch_line = if view.remove_source_branch {
            "• Source branch will be deleted."
        } else {
            "• Source branch will not be deleted."
        };
        let mut details = div()
            .pl(px(48.)).pr_4().py_3().border_t_1().border_color(theme.border)
            .flex().flex_col().gap_1p5().text_xs().text_color(theme.muted_foreground);
        if behind > 0 {
            let b_noun = if behind == 1 { "commit" } else { "commits" };
            details = details.child(
                div().flex().items_center().flex_wrap().gap_1()
                    .child(format!("• The source branch is {behind} {b_noun} behind the target branch."))
                    .child(
                        div().id("mr-rebase-link").cursor_pointer().text_color(theme.link)
                            .hover(|s| s.text_color(theme.link_hover))
                            .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("rebase", Some(w), cx)))
                            .child(if mr.rebase_in_progress == Some(true) { "Rebasing…" } else { "Rebase source branch" }),
                    ),
            );
        }
        details = details.child(commit_line).child(branch_line).child(
            div().pt_1().flex().items_center().flex_wrap().gap_4()
                .child(Checkbox::new(SharedString::from("mr-opt-del-branch")).label("Delete source branch").checked(view.remove_source_branch)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| { this.remove_source_branch = *checked; cx.notify(); })))
                .child(Checkbox::new(SharedString::from("mr-opt-squash")).label("Squash commits").checked(view.squash_commits)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| { this.squash_commits = *checked; cx.notify(); }))),
        );
        card = card.child(details);
    }
    card.test_support()
}
