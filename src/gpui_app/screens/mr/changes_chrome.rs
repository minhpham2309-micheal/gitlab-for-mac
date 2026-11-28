use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        diff::DiffMode,
        input::Input,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{build_diff_tree_rows, DiffTreeRow};

use super::super::super::pajamas_theme::{status_badge_palette, status_color};
use super::MergeRequestsView;

pub fn render_mr_file_tree(
    view: &MergeRequestsView,
    head_ref: &str,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let (border, bg, hover_bg, head_bg, fg, muted, succ, dang, input_b, sec_bg) = (
        theme.border, theme.background, theme.list_hover, theme.list_head,
        theme.foreground, theme.muted_foreground, theme.success, theme.danger, theme.input, theme.secondary,
    );
    let query = view.diff_filter_input.read(cx).value().to_string();
    let file_items: Vec<(String, usize, usize, &'static str)> = view.diff_state.read(cx).files().iter().map(|f| {
        let badge = if f.modified_path().is_none() { "D" } else if f.original_path().is_none() { "A" } else { "M" };
        (f.path().to_string(), f.additions(), f.deletions(), badge)
    }).collect();
    let total_files = view.raw_diffs.len().max(file_items.len());
    let tree_mode = view.diff_tree_mode;

    let rows: Vec<DiffTreeRow> = if tree_mode {
        build_diff_tree_rows(&file_items, &query)
    } else {
        let q = query.trim().to_ascii_lowercase();
        file_items.iter().enumerate()
            .filter(|(_, (p, _, _, _))| q.is_empty() || p.to_ascii_lowercase().contains(&q))
            .map(|(ix, (p, add, del, badge))| DiffTreeRow::File {
                depth: 0, name: p.clone(), full_path: p.clone(), file_ix: ix, additions: *add, deletions: *del, badge,
            })
            .collect()
    };

    let mut row_els = Vec::with_capacity(rows.len());
    let mut skip_prefix: Option<String> = None;
    for (r_ix, row) in rows.into_iter().enumerate() {
        match row {
            DiffTreeRow::Dir { depth, name, path } => {
                if skip_prefix.as_deref().is_some_and(|p| path.starts_with(&format!("{p}/"))) { continue; }
                let collapsed = view.collapsed_diff_dirs.contains(&path);
                skip_prefix = collapsed.then(|| path.clone());
                let dir_key = path.clone();
                let chev = if collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
                row_els.push(
                    div().id(SharedString::from(format!("mr-dir-{r_ix}")))
                        .pl(px(6. + depth as f32 * 12.)).pr_2().py_1().rounded_sm().cursor_pointer()
                        .hover(move |s| s.bg(hover_bg)).flex().items_center().gap_1p5()
                        .on_click(cx.listener(move |v, _, _, cx| {
                            if !v.collapsed_diff_dirs.remove(&dir_key) { v.collapsed_diff_dirs.insert(dir_key.clone()); }
                            cx.notify();
                        }))
                        .child(Icon::new(chev).size_3().text_color(muted))
                        .child(Icon::new(IconName::Folder).size_3p5().text_color(muted))
                        .child(div().text_xs().text_color(fg).truncate().child(name))
                        .test_support().into_any_element(),
                );
            }
            DiffTreeRow::File { depth, name, full_path, file_ix, additions, deletions, badge } => {
                if skip_prefix.as_deref().is_some_and(|p| full_path.starts_with(&format!("{p}/"))) { continue; }
                let viewed = view.viewed_files.contains(&full_path);
                let (ds_click, scroll_path, view_path) = (view.diff_state.clone(), full_path.clone(), full_path.clone());
                let (open_state, open_ref, open_path) = (view.state.clone(), head_ref.to_string(), full_path.clone());
                let badge_status = match badge { "A" => "success", "D" => "failed", _ => "manual" };
                let checked_bg = status_badge_palette("success", dark);

                row_els.push(
                    div().id(SharedString::from(format!("mr-tree-{file_ix}")))
                        .pl(px(6. + depth as f32 * 12.)).pr_1p5().py_1().rounded_sm().cursor_pointer()
                        .hover(move |s| s.bg(hover_bg)).flex().items_center().justify_between().gap_1()
                        .on_click(move |_, _, cx| ds_click.update(cx, |ds, cx| ds.scroll_to_file(&scroll_path, cx)))
                        .child(
                            div().flex().items_center().gap_1p5().min_w_0()
                                .child(
                                    div().id(SharedString::from(format!("mr-file-viewed-{file_ix}")))
                                        .size(px(14.)).rounded(px(3.)).flex_shrink_0().cursor_pointer()
                                        .border_1().border_color(if viewed { checked_bg.icon } else { input_b })
                                        .bg(if viewed { checked_bg.bg } else { gpui_kit::transparent_black() })
                                        .flex().items_center().justify_center()
                                        .on_click(cx.listener(move |v, _, _, cx| {
                                            cx.stop_propagation();
                                            if !v.viewed_files.remove(&view_path) { v.viewed_files.insert(view_path.clone()); }
                                            cx.notify();
                                        }))
                                        .children(viewed.then(|| Icon::new(IconName::Check).size(px(10.)).text_color(checked_bg.icon)))
                                        .test_support(),
                                )
                                .child(div().text_xs().font_semibold().text_color(status_color(badge_status, dark)).child(badge))
                                .child(div().text_xs().text_color(if viewed { muted } else { fg }).truncate().child(name)),
                        )
                        .child(
                            div().flex().items_center().gap_1().text_xs().flex_shrink_0()
                                .child(div().text_color(succ).child(format!("+{additions}")))
                                .child(div().text_color(dang).child(format!("-{deletions}")))
                                .children((badge != "D").then(|| {
                                    Button::new(SharedString::from(format!("mr-view-file-{file_ix}"))).ghost().xsmall().label("↗")
                                        .on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            if let Some(pid) = open_state.read(cx).project.as_ref().map(|p| p.id) {
                                                open_state.update(cx, |s, cx| s.open_repo_file(pid, Some(open_ref.clone()), open_path.clone(), None, None, cx));
                                            }
                                        })
                                })),
                        )
                        .test_support().into_any_element(),
                );
            }
        }
    }

    div().id("mr-file-tree").size_full().bg(bg).border_r_1().border_color(border).flex().flex_col()
        .child(
            div().px_2p5().pt_2().pb_1p5().border_b_1().border_color(border).flex().flex_col().gap_1p5()
                .child(
                    div().flex().items_center().justify_between().gap_1()
                        .child(div().flex().items_center().gap_1p5()
                            .child(div().text_xs().font_semibold().text_color(fg).child("Files"))
                            .child(div().px_1p5().py(px(1.)).rounded_full().bg(sec_bg).text_xs().text_color(muted).child(total_files.to_string())))
                        .child(Button::new("mr-tree-mode-toggle").outline().xsmall().label(if tree_mode { "Flat" } else { "Tree" })
                            .on_click(cx.listener(|v, _, _, cx| { v.diff_tree_mode = !v.diff_tree_mode; cx.notify(); }))),
                )
                .child(div().id("mr-diff-file-filter").h(px(28.)).rounded_md().bg(head_bg).border_1().border_color(input_b).px_1p5().flex().items_center()
                    .child(div().flex_1().min_w_0().child(Input::new(&view.diff_filter_input).appearance(false).cleanable(true).xsmall())).test_support()),
        )
        .child(
            div().id("mr-file-tree-scroll").flex_1().min_h_0().overflow_y_scroll().p_1p5().flex().flex_col().gap_0p5()
                .children(row_els)
                .children(view.fallback_notices.iter().enumerate().map(|(ix, (path, note))| {
                    div().id(SharedString::from(format!("mr-notice-{ix}"))).px_2().py_1().text_xs().text_color(muted)
                        .child(format!("⚠ {path}: {note}"))
                })),
        )
        .test_support()
}

pub fn render_mr_diff_toolbar(view: &MergeRequestsView, cx: &mut Context<MergeRequestsView>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let (mode, total_files, total_add, total_del) = {
        let ds = view.diff_state.read(cx);
        let (a, d) = ds.files().iter().fold((0usize, 0usize), |(a, d), f| (a + f.additions(), d + f.deletions()));
        (ds.mode(), ds.files().len().max(view.raw_diffs.len()), a, d)
    };
    let viewed = view.viewed_files.len();
    let viewed_pal = status_badge_palette("success", dark);
    let (ds_mode, ds_collapse, ds_expand, ds_prev, ds_next) = (
        view.diff_state.clone(), view.diff_state.clone(), view.diff_state.clone(),
        view.diff_state.clone(), view.diff_state.clone(),
    );

    div().flex().items_center().justify_between().gap_2().px_3().py_1p5()
        .border_b_1().border_color(theme.border).bg(theme.list_head)
        .child(
            div().flex().items_center().gap_1()
                .child(Button::new("diff-mode-toggle").outline().xsmall().label(if mode == DiffMode::Split { "Split" } else { "Unified" })
                    .on_click(move |_, _, cx| {
                        let next = if mode == DiffMode::Split { DiffMode::Unified } else { DiffMode::Split };
                        ds_mode.update(cx, |ds, cx| ds.set_mode(next, cx));
                    }))
                .child(Button::new("diff-collapse").ghost().xsmall().label("Collapse all")
                    .on_click(move |_, _, cx| ds_collapse.update(cx, |ds, cx| ds.set_all_files_collapsed(true, cx))))
                .child(Button::new("diff-expand").ghost().xsmall().label("Expand all")
                    .on_click(move |_, _, cx| ds_expand.update(cx, |ds, cx| ds.set_all_files_collapsed(false, cx)))),
        )
        .child(
            div().flex().items_center().gap_2()
                .child(
                    div().id("mr-diff-stats").flex().items_center().gap_1p5().text_xs()
                        .child(div().text_color(theme.foreground).font_semibold().child(format!("{total_files} files")))
                        .child(div().text_color(theme.success).font_semibold().child(format!("+{total_add}")))
                        .child(div().text_color(theme.danger).font_semibold().child(format!("-{total_del}")))
                        .test_support(),
                )
                .children((viewed > 0).then(|| {
                    div().id("mr-diff-viewed-count").px_1p5().py(px(1.)).rounded_full()
                        .bg(viewed_pal.bg).text_color(viewed_pal.text).text_xs().font_semibold()
                        .child(format!("{viewed}/{total_files} viewed")).test_support()
                }))
                .children(view.has_truncated.then(|| div().text_xs().text_color(theme.warning).child("Truncated hunks")))
                .child(Button::new("diff-prev").ghost().xsmall().label("Prev")
                    .on_click(move |_, _, cx| ds_prev.update(cx, |ds, cx| ds.previous_change(cx))))
                .child(Button::new("diff-next").ghost().xsmall().label("Next")
                    .on_click(move |_, _, cx| ds_next.update(cx, |ds, cx| ds.next_change(cx)))),
        )
}
