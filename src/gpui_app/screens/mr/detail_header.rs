use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, ClipboardItem, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{mr_can_review, MergeRequest};

use super::super::super::pajamas_theme::status_badge_palette;
use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

fn mr_state_pill(mr: &MergeRequest, dark: bool) -> (gpui_kit::Hsla, gpui_kit::Hsla, &'static str) {
    let (key, label) = if mr.state == "merged" {
        ("merged", "Merged")
    } else if mr.state == "closed" {
        ("closed", "Closed")
    } else if mr.draft {
        ("skipped", "Draft")
    } else {
        ("open", "Open")
    };
    let p = status_badge_palette(key, dark);
    (p.bg, p.text, label)
}

pub fn render_mr_detail_header(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let perms = view.mr_permissions(mr, cx);
    let (pill_bg, pill_fg, state_label) = mr_state_pill(mr, dark);
    let author = mr.author.as_ref()
        .map(|a| if a.name.trim().is_empty() { a.username.clone() } else { a.name.clone() })
        .unwrap_or_else(|| "unknown".into());
    let created_rel = mr.created_at.as_deref().map(format_relative).unwrap_or_default();

    let (src, src_copy, src_checkout) = (mr.source_branch.clone(), mr.source_branch.clone(), mr.source_branch.clone());
    let (web_url, web_copy, web_more) = (mr.web_url.clone(), mr.web_url.clone(), mr.web_url.clone());
    let web_patch = format!("{}.patch", web_url.trim_end_matches('/'));
    let mr_ref = format!("!{}", mr.iid);
    let (state_for_copy, state_for_code) = (view.state.clone(), view.state.clone());
    let (weak_more, weak_rev) = (cx.entity().downgrade(), cx.entity().downgrade());

    let src_bg: gpui_kit::Hsla = if dark { rgb(0x254581).into() } else { rgb(0xDFEEFC).into() };
    let active_tab_border = theme.primary;
    let active_badge_bg = theme.primary;
    let active_badge_fg = theme.primary_foreground;
    let inactive_badge_bg: gpui_kit::Hsla = if dark { rgb(0x383839).into() } else { theme.secondary };
    let inactive_badge_fg: gpui_kit::Hsla = if dark { rgb(0xBFBFBF).into() } else { theme.muted_foreground };
    let thread_bg: gpui_kit::Hsla = if dark { rgb(0x472614).into() } else { rgb(0xFDF1DD).into() };
    let thread_bd: gpui_kit::Hsla = if dark { rgb(0x7D2D0E).into() } else { rgb(0xE0B15C).into() };
    let thread_fg: gpui_kit::Hsla = if dark { rgb(0xF7B951).into() } else { rgb(0x7A4400).into() };
    let cta_bg = theme.primary;
    let cta_fg = theme.primary_foreground;

    let non_sys_notes: usize = view.discussions.iter().map(|d| d.notes.iter().filter(|n| !n.system).count()).sum();
    let open_threads = view.unresolved_thread_indices().len();
    let changes_count = view.raw_diffs.len();
    let tabs_meta = [
        ("Overview", non_sys_notes), ("Commits", view.commits.len()),
        ("Pipelines", view.mr_pipelines.len()), ("Changes", changes_count),
    ];

    let title_row = div()
        .flex().items_start().justify_between().gap_4()
        .child(div().id("mr-detail-title").flex_1().min_w_0().text_2xl().font_bold().text_color(theme.foreground).child(mr.title.clone()).test_support())
        .child(
            div().flex().items_center().gap_1p5().flex_shrink_0()
                .children(perms.can_edit.then(|| {
                    div().id("mr-edit-btn").px_3().py_1().rounded_md().cursor_pointer()
                        .bg(theme.secondary).border_1().border_color(theme.border).hover(|s| s.bg(theme.secondary_hover))
                        .text_xs().text_color(theme.foreground)
                        .on_click(cx.listener(|this, _, window, cx| this.open_edit_mr_dialog(window, cx)))
                        .child("Edit").test_support()
                }))
                .child(
                    Button::new("mr-code-menu").outline().small().label("Code ▾")
                        .dropdown_menu(move |menu, _, _| {
                            let (b1, b2, u1, u2, u3) = (src_copy.clone(), src_checkout.clone(), web_copy.clone(), web_url.clone(), web_patch.clone());
                            let (st1, st2) = (state_for_code.clone(), state_for_code.clone());
                            menu.item(PopupMenuItem::new("Copy branch name").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(b1.clone()));
                                st1.update(cx, |s, cx| s.set_status("Copied source branch name", cx));
                            }))
                            .item(PopupMenuItem::new("Copy git checkout command").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(format!("git fetch origin && git checkout -b '{b2}' 'origin/{b2}'")));
                                st2.update(cx, |s, cx| s.set_status("Copied checkout command", cx));
                            }))
                            .separator()
                            .item(PopupMenuItem::new("Copy merge request URL").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(u1.clone()))))
                            .item(PopupMenuItem::new("Open plain patch in browser").on_click(move |_, _, _| { let _ = safe_open_url(&u3); }))
                            .item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| { let _ = safe_open_url(&u2); }))
                        }),
                )
                .child(
                    Button::new("mr-detail-more").ghost().small().icon(Icon::new(IconName::EllipsisVertical).size_3p5())
                        .dropdown_menu(move |mut menu, _, _| {
                            let (r, w_ref, w_mut, u) = (mr_ref.clone(), weak_more.clone(), weak_more.clone(), web_more.clone());
                            menu = menu
                                .item(PopupMenuItem::new(format!("Copy reference ({r})")).on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(r.clone()))))
                                .item(PopupMenuItem::new("Refresh merge request").on_click(move |_, _, cx| {
                                    let _ = w_ref.update(cx, |v, cx| { if let Some(m) = v.selected.clone() { v.select_mr(m, false, cx); } });
                                }))
                                .item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| { let _ = safe_open_url(&u); }));
                            if perms.can_close {
                                menu = menu.separator().item(PopupMenuItem::new("Close merge request").on_click(move |_, window, cx| {
                                    let _ = w_mut.update(cx, |v, cx| v.perform_mr_mutation("close", Some(window), cx));
                                }));
                            } else if perms.can_reopen {
                                menu = menu.separator().item(PopupMenuItem::new("Reopen merge request").on_click(move |_, window, cx| {
                                    let _ = w_mut.update(cx, |v, cx| v.perform_mr_mutation("reopen", Some(window), cx));
                                }));
                            }
                            menu
                        }),
                ),
        );

    let meta_row = div()
        .mt_1p5().flex().items_center().flex_wrap().gap_1p5().text_xs()
        .child(
            div().px_2().py(px(2.)).rounded_full().bg(pill_bg).text_color(pill_fg)
                .flex().items_center().gap_1().font_semibold()
                .child(Icon::empty().path("icons/glab-mr.svg").size_3().text_color(pill_fg))
                .child(state_label),
        )
        .child(div().font_semibold().text_color(theme.foreground).child(author))
        .child(div().text_color(theme.muted_foreground).child("requested to merge"))
        .child(div().px_1p5().py(px(1.)).rounded_sm().bg(src_bg).text_color(theme.link).font_family(theme.mono_font_family.clone()).max_w(px(260.)).truncate().child(src.clone()))
        .child(
            Button::new("mr-copy-src-branch").ghost().xsmall().icon(Icon::new(IconName::Copy).size_3())
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(src.clone()));
                    state_for_copy.update(cx, |s, cx| s.set_status("Copied source branch", cx));
                }),
        )
        .child(div().text_color(theme.muted_foreground).child("into"))
        .child(div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).text_color(theme.link).font_family(theme.mono_font_family.clone()).child(mr.target_branch.clone()))
        .children((!created_rel.is_empty()).then(|| div().text_color(theme.muted_foreground).child(created_rel)));

    let open_threads_pill = (open_threads > 0).then(|| {
        let lbl = if open_threads == 1 { "1 open thread".into() } else { format!("{open_threads} open threads") };
        div()
            .id("mr-open-threads-nav").rounded_md().bg(thread_bg).border_1().border_color(thread_bd)
            .flex().items_center().text_xs().text_color(thread_fg)
            .child(div().id("mr-open-threads-jump").px_2().py_1().cursor_pointer().font_medium()
                .on_click(cx.listener(|this, _, _, cx| this.jump_to_open_thread(0, cx))).child(lbl))
            .child(div().id("mr-open-threads-prev").px_1p5().py_1().border_l_1().border_color(thread_bd).cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.jump_to_open_thread(-1, cx)))
                .child(Icon::new(IconName::ChevronUp).size_3().text_color(thread_fg)))
            .child(div().id("mr-open-threads-next").px_1p5().py_1().border_l_1().border_color(thread_bd).cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.jump_to_open_thread(1, cx)))
                .child(Icon::new(IconName::ChevronDown).size_3().text_color(thread_fg)))
            .test_support()
    });

    let show_review_cta = mr_can_review(&mr.state) && (perms.can_approve || perms.can_unapprove || perms.can_comment);
    let review_btn = show_review_cta.then(|| {
        div()
            .id("mr-your-review-btn")
            .child(
                Button::new("mr-your-review-menu").primary().small().label("Your review")
                    .bg(cta_bg).text_color(cta_fg)
                    .dropdown_menu(move |mut menu, _, _| {
                        let (w_app, w_cmt, w_chg) = (weak_rev.clone(), weak_rev.clone(), weak_rev.clone());
                        if perms.can_approve {
                            menu = menu.item(PopupMenuItem::new("Approve merge request").on_click(move |_, w, cx| {
                                let _ = w_app.update(cx, |v, cx| v.perform_mr_mutation("approve", Some(w), cx));
                            }));
                        } else if perms.can_unapprove {
                            menu = menu.item(PopupMenuItem::new("Revoke approval").on_click(move |_, w, cx| {
                                let _ = w_app.update(cx, |v, cx| v.perform_mr_mutation("unapprove", Some(w), cx));
                            }));
                        }
                        if perms.can_comment {
                            menu = menu.item(PopupMenuItem::new("Add comment").on_click(move |_, _, cx| {
                                let _ = w_cmt.update(cx, |v, cx| v.jump_to_comment_composer(cx));
                            }));
                        }
                        menu.item(PopupMenuItem::new(format!("View changes ({changes_count})")).on_click(move |_, _, cx| {
                            let _ = w_chg.update(cx, |v, cx| { v.active_tab = 3; cx.notify(); });
                        }))
                    }),
            )
            .test_support()
    });

    let tabs_row = div()
        .mt_3().border_b_1().border_color(theme.border)
        .flex().items_end().justify_between().gap_4()
        .child(div().flex().items_center().gap_1().children(tabs_meta.into_iter().enumerate().map(|(ix, (lbl, cnt))| {
            let active = view.active_tab == ix;
            let (badge_bg, badge_fg) = if active { (active_badge_bg, active_badge_fg) } else { (inactive_badge_bg, inactive_badge_fg) };
            div()
                .id(SharedString::from(format!("mr-detail-tab-{ix}")))
                .px_3().py_2().cursor_pointer()
                .border_b_2().border_color(if active { active_tab_border } else { gpui_kit::transparent_black() })
                .flex().items_center().gap_1p5().text_sm()
                .text_color(if active { theme.foreground } else { theme.muted_foreground })
                .hover(|s| s.text_color(theme.foreground))
                .on_click(cx.listener(move |this, _, _, cx| { this.active_tab = ix; cx.notify(); }))
                .child(div().when(active, |d| d.font_semibold()).child(lbl))
                .child(div().px_1p5().py(px(1.)).rounded_full().bg(badge_bg).text_color(badge_fg).text_xs().font_semibold().child(cnt.to_string()))
                .test_support()
        })))
        .child(
            div().id("mr-tabs-right").pb_1p5().flex().items_center().gap_2().flex_shrink_0()
                .children(open_threads_pill)
                .children(perms.can_todo.then(|| {
                    div().id("mr-add-todo").px_3().py_1().rounded_md().cursor_pointer()
                        .bg(theme.secondary).border_1().border_color(theme.border).hover(|s| s.bg(theme.secondary_hover))
                        .text_xs().text_color(theme.foreground)
                        .on_click(cx.listener(|this, _, _, cx| this.add_mr_todo(cx)))
                        .child(if view.todo_added { "Mark as done" } else { "Add a to-do item" })
                        .test_support()
                }))
                .children(review_btn)
                .test_support(),
        );

    div()
        .w_full().px_6().pt_3().flex().flex_col().items_center()
        .child(div().w_full().max_w(px(1120.)).flex().flex_col().child(title_row).child(meta_row).child(tabs_row))
}
