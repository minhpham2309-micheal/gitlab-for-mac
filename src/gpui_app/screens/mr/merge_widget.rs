use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        checkbox::Checkbox,
        input::Textarea,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, rgb, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{
    mr_can_review, mr_merge_readiness_with_threads, MergeCheckKind, MergeReadinessTone,
    MergeRequest,
};

use super::super::super::pajamas_theme::status_color;
use super::MergeRequestsView;

pub fn render_merge_widget(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let open_threads = view.unresolved_thread_indices().len();
    let readiness = mr_merge_readiness_with_threads(mr, open_threads);
    let can_review = mr_can_review(&mr.state);
    let perms = view.mr_permissions(mr, cx);
    let behind = mr.diverged_commits_count.unwrap_or(0);
    let commits_n = view.commits.len().max(1);
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
                    div().pl_2().border_l_1().border_color(theme.border).child(
                        Button::new("mr-merge-expand").ghost().xsmall()
                            .icon(Icon::new(if view.merge_details_expanded { IconName::ChevronUp } else { IconName::ChevronDown }).size_3p5())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.merge_details_expanded = !this.merge_details_expanded;
                                this.overview_list.remeasure_items(1..2);
                                cx.notify();
                            })),
                    ),
                ),
        );

    if view.merge_details_expanded && can_review {
        let mut body = div().w_full().border_t_1().border_color(theme.border).flex().flex_col();

        if !readiness.failed_checks.is_empty() {
            let mut checks_box = div().pl(px(48.)).pr_4().py_2p5().flex().flex_col().gap_1p5().text_xs();
            for (ix, chk) in readiness.failed_checks.iter().enumerate() {
                let icon_col = match chk.kind {
                    MergeCheckKind::PipelineRunning | MergeCheckKind::Draft => status_color("warning", dark),
                    _ => status_color("failed", dark),
                };
                let action = match chk.kind {
                    MergeCheckKind::OpenThreads if open_threads > 0 => Some(
                        div().id("mr-go-first-open-thread").cursor_pointer().text_color(theme.link)
                            .hover(|s| s.text_color(theme.link_hover))
                            .on_click(cx.listener(|this, _, _, cx| this.jump_to_open_thread(0, cx)))
                            .child("Go to first open thread").test_support().into_any_element(),
                    ),
                    MergeCheckKind::NeedRebase if perms.can_rebase => Some(
                        div().id("mr-rebase-link").cursor_pointer().text_color(theme.link)
                            .hover(|s| s.text_color(theme.link_hover))
                            .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("rebase", Some(w), cx)))
                            .child(if mr.rebase_in_progress == Some(true) { "Rebasing…" } else { "Rebase source branch" })
                            .test_support().into_any_element(),
                    ),
                    _ => None,
                };
                checks_box = checks_box.child(
                    div().id(SharedString::from(format!("mr-merge-check-{ix}")))
                        .flex().items_center().justify_between().gap_3()
                        .child(
                            div().flex().items_center().gap_2().min_w_0()
                                .child(Icon::new(IconName::CircleX).size_3p5().text_color(icon_col))
                                .child(div().text_color(theme.foreground).child(chk.message.clone())),
                        )
                        .children(action),
                );
            }
            body = body.child(checks_box);
        }

        let has_rebase_check = readiness.failed_checks.iter().any(|c| c.kind == MergeCheckKind::NeedRebase);
        let noun = if commits_n == 1 { "commit" } else { "commits" };
        let commit_line = if view.squash_commits {
            format!("• 1 commit and 1 merge commit will be added to {} (squashes {commits_n} {noun}).", mr.target_branch)
        } else {
            format!("• {commits_n} {noun} and 1 merge commit will be added to {}.", mr.target_branch)
        };

        let mut controls = div()
            .pl(px(48.)).pr_4().py_3()
            .flex().flex_col().gap_2().text_xs().text_color(theme.muted_foreground);
        if !readiness.failed_checks.is_empty() {
            controls = controls.border_t_1().border_color(theme.border.opacity(0.6));
        }

        if behind > 0 && !has_rebase_check {
            let b_noun = if behind == 1 { "commit" } else { "commits" };
            controls = controls.child(
                div().flex().items_center().flex_wrap().gap_1()
                    .child(format!("• The source branch is {behind} {b_noun} behind the target branch."))
                    .children(perms.can_rebase.then(|| {
                        div().id("mr-rebase-link").cursor_pointer().text_color(theme.link)
                            .hover(|s| s.text_color(theme.link_hover))
                            .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("rebase", Some(w), cx)))
                            .child(if mr.rebase_in_progress == Some(true) { "Rebasing…" } else { "Rebase source branch" })
                            .test_support()
                    })),
            );
        }

        if perms.can_merge {
            controls = controls
                .child(
                    div().id("mr-merge-options").flex().items_center().flex_wrap().gap_4()
                        .child(Checkbox::new(SharedString::from("mr-opt-del-branch")).label("Delete source branch").checked(view.remove_source_branch)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| { this.remove_source_branch = *checked; cx.notify(); })))
                        .child(Checkbox::new(SharedString::from("mr-opt-squash")).label("Squash commits").checked(view.squash_commits)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| { this.squash_commits = *checked; cx.notify(); })))
                        .child(Checkbox::new(SharedString::from("mr-opt-edit-msg")).label("Edit commit message").checked(view.edit_commit_msg)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.edit_commit_msg = *checked;
                                this.overview_list.remeasure_items(1..2);
                                cx.notify();
                            })))
                        .test_support(),
                )
                .children(view.edit_commit_msg.then(|| {
                    Textarea::new(&view.commit_msg_input).h(px(72.))
                }))
                .child(commit_line);

            if readiness.can_merge_now {
                controls = controls.child(
                    div().pt_1().flex().items_center().gap_2().child(
                        Button::new("mr-widget-merge-btn").primary().small().label("Merge")
                            .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("merge", Some(w), cx))),
                    ),
                );
            } else if readiness.can_auto_merge {
                controls = controls.child(
                    div().pt_1().flex().items_center().gap_2p5()
                        .child(
                            Button::new("mr-widget-auto-merge-btn").primary().small().label("Set to auto-merge")
                                .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("auto_merge", Some(w), cx))),
                        )
                        .child(div().text_xs().text_color(theme.muted_foreground).child("Merge when all merge checks pass")),
                );
            }
        } else {
            controls = controls.child(commit_line);
        }

        body = body.child(controls);
        card = card.child(body);
    }
    card.test_support()
}
