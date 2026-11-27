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

use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

fn mr_state_pill(mr: &MergeRequest, dark: bool) -> (gpui_kit::Hsla, gpui_kit::Hsla, &'static str) {
    if mr.state == "merged" {
        return (rgb(if dark { 0x1F75CB } else { 0x1068BF }).into(), rgb(0xFFFFFF).into(), "Merged");
    }
    if mr.state == "closed" {
        return (rgb(if dark { 0xDD2B0E } else { 0xC91C00 }).into(), rgb(0xFFFFFF).into(), "Closed");
    }
    if mr.draft {
        return (rgb(if dark { 0x4C4B51 } else { 0xDCDCDE }).into(), rgb(if dark { 0xECECEF } else { 0x28272D }).into(), "Draft");
    }
    (rgb(if dark { 0x2DA160 } else { 0x108548 }).into(), rgb(if dark { 0x102A1C } else { 0xFFFFFF }).into(), "Open")
}

pub fn render_mr_detail_header(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let can_review = mr_can_review(&mr.state);
    let is_closed = mr.state == "closed";
    let (pill_bg, pill_fg, state_label) = mr_state_pill(mr, dark);
    let author = mr.author.as_ref().map(|a| a.username.clone()).unwrap_or_else(|| "unknown".into());
    let created_rel = mr.created_at.as_deref().map(format_relative).unwrap_or_default();

    let src = mr.source_branch.clone();
    let src_copy = src.clone();
    let src_checkout = src.clone();
    let web_url = mr.web_url.clone();
    let web_copy = web_url.clone();
    let web_patch = format!("{}.patch", web_url.trim_end_matches('/'));
    let web_more = web_url.clone();
    let mr_ref = format!("!{}", mr.iid);
    let state_for_copy = view.state.clone();
    let state_for_code = view.state.clone();
    let weak_more = cx.entity().downgrade();

    let src_bg: gpui_kit::Hsla = if dark { rgb(0x1F3550).into() } else { rgb(0xE9F3FC).into() };
    let active_tab_border: gpui_kit::Hsla = if dark { rgb(0x63A6E9).into() } else { theme.primary };
    let badge_bg: gpui_kit::Hsla = if dark { rgb(0xBFBFBF).into() } else { theme.secondary };
    let badge_fg: gpui_kit::Hsla = if dark { rgb(0x18171D).into() } else { theme.foreground };

    let non_sys_notes: usize = view
        .discussions
        .iter()
        .map(|d| d.notes.iter().filter(|n| !n.system).count())
        .sum();
    let tabs_meta = [
        ("Overview", non_sys_notes),
        ("Commits", view.commits.len()),
        ("Pipelines", view.mr_pipelines.len()),
        ("Changes", view.raw_diffs.len()),
    ];

    let title_row = div()
        .flex().items_start().justify_between().gap_4()
        .child(
            div().id("mr-detail-title").flex_1().min_w_0()
                .text_2xl().font_bold().text_color(theme.foreground)
                .child(mr.title.clone()).test_support(),
        )
        .child(
            div().flex().items_center().gap_1p5().flex_shrink_0()
                .child(
                    div().id("mr-edit-btn").px_3().py_1().rounded_md().cursor_pointer()
                        .bg(theme.secondary).border_1().border_color(theme.border)
                        .hover(|s| s.bg(theme.secondary_hover))
                        .text_xs().text_color(theme.foreground)
                        .on_click(cx.listener(|this, _, window, cx| this.open_edit_mr_dialog(window, cx)))
                        .child("Edit").test_support(),
                )
                .child(
                    Button::new("mr-code-menu").outline().small().label("Code ▾")
                        .dropdown_menu(move |menu, _, _| {
                            let b1 = src_copy.clone();
                            let b2 = src_checkout.clone();
                            let u1 = web_copy.clone();
                            let u2 = web_url.clone();
                            let u3 = web_patch.clone();
                            let st1 = state_for_code.clone();
                            let st2 = state_for_code.clone();
                            menu.item(PopupMenuItem::new("Copy branch name").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(b1.clone()));
                                st1.update(cx, |s, cx| s.set_status("Copied source branch name", cx));
                            }))
                            .item(PopupMenuItem::new("Copy git checkout command").on_click(move |_, _, cx| {
                                let cmd = format!("git fetch origin && git checkout -b '{b2}' 'origin/{b2}'");
                                cx.write_to_clipboard(ClipboardItem::new_string(cmd));
                                st2.update(cx, |s, cx| s.set_status("Copied checkout command", cx));
                            }))
                            .separator()
                            .item(PopupMenuItem::new("Copy merge request URL").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(u1.clone()));
                            }))
                            .item(PopupMenuItem::new("Open plain patch in browser").on_click(move |_, _, _| { let _ = safe_open_url(&u3); }))
                            .item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| { let _ = safe_open_url(&u2); }))
                        }),
                )
                .child(
                    Button::new("mr-detail-more").ghost().small().icon(Icon::new(IconName::EllipsisVertical).size_3p5())
                        .dropdown_menu(move |mut menu, _, _| {
                            let r = mr_ref.clone();
                            let w_ref = weak_more.clone();
                            let w_mut = weak_more.clone();
                            let u = web_more.clone();
                            menu = menu
                                .item(PopupMenuItem::new(format!("Copy reference ({r})")).on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(r.clone()));
                                }))
                                .item(PopupMenuItem::new("Refresh merge request").on_click(move |_, _, cx| {
                                    let _ = w_ref.update(cx, |v, cx| {
                                        if let Some(m) = v.selected.clone() { v.select_mr(m, false, cx); }
                                    });
                                }))
                                .item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| { let _ = safe_open_url(&u); }))
                                .separator();
                            if can_review {
                                menu = menu.item(PopupMenuItem::new("Close merge request").on_click(move |_, window, cx| {
                                    let _ = w_mut.update(cx, |v, cx| v.perform_mr_mutation("close", Some(window), cx));
                                }));
                            } else if is_closed {
                                menu = menu.item(PopupMenuItem::new("Reopen merge request").on_click(move |_, window, cx| {
                                    let _ = w_mut.update(cx, |v, cx| v.perform_mr_mutation("reopen", Some(window), cx));
                                }));
                            }
                            menu
                        }),
                ),
        );

    let meta_row = div()
        .mt_2().flex().items_center().flex_wrap().gap_1p5().text_xs()
        .child(
            div().px_2().py(px(2.)).rounded_full().bg(pill_bg).text_color(pill_fg)
                .flex().items_center().gap_1().font_semibold()
                .child(Icon::empty().path("icons/glab-mr.svg").size_3().text_color(pill_fg))
                .child(state_label),
        )
        .child(div().font_semibold().text_color(theme.foreground).child(author))
        .child(div().text_color(theme.muted_foreground).child("requested to merge"))
        .child(
            div().px_1p5().py(px(1.)).rounded_sm().bg(src_bg).text_color(theme.link)
                .font_family(theme.mono_font_family.clone()).max_w(px(260.)).truncate().child(src.clone()),
        )
        .child(
            Button::new("mr-copy-src-branch").ghost().xsmall().icon(Icon::new(IconName::Copy).size_3())
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(src.clone()));
                    state_for_copy.update(cx, |s, cx| s.set_status("Copied source branch", cx));
                }),
        )
        .child(div().text_color(theme.muted_foreground).child("into"))
        .child(
            div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).text_color(theme.link)
                .font_family(theme.mono_font_family.clone()).child(mr.target_branch.clone()),
        )
        .children((!created_rel.is_empty()).then(|| div().text_color(theme.muted_foreground).child(created_rel)));

    let tabs_row = div()
        .mt_4().border_b_1().border_color(theme.border)
        .flex().items_end().justify_between().gap_4()
        .child(div().flex().items_center().gap_1().children(tabs_meta.into_iter().enumerate().map(|(ix, (lbl, cnt))| {
            let active = view.active_tab == ix;
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
            div().pb_1p5().child(
                div().id("mr-add-todo").px_3().py_1().rounded_md().cursor_pointer()
                    .bg(theme.secondary).border_1().border_color(theme.border)
                    .hover(|s| s.bg(theme.secondary_hover))
                    .text_xs().text_color(theme.foreground)
                    .on_click(cx.listener(|this, _, _, cx| this.add_mr_todo(cx)))
                    .child(if view.todo_added { "To-do added ✓" } else { "Add a to-do item" })
                    .test_support(),
            ),
        );

    div()
        .w_full().px_6().pt_4()
        .child(div().w_full().max_w(px(1120.)).mx_auto().flex().flex_col().child(title_row).child(meta_row).child(tabs_row))
}
