use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        diff::{DiffMode, DiffState},
        ActiveTheme as _, Sizable as _,
    },
    div, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _,
};

use super::super::super::pajamas_theme::status_color;
use super::super::super::state::AppState;

pub fn render_mr_file_tree(
    diff_state: &Entity<DiffState>,
    raw_count: usize,
    fallback_notices: &[(String, String)],
    app_state: &Entity<AppState>,
    head_ref: &str,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let file_items: Vec<(String, usize, usize, &'static str)> = diff_state
        .read(cx)
        .files()
        .iter()
        .map(|f| {
            let badge = if f.modified_path().is_none() {
                "D"
            } else if f.original_path().is_none() {
                "A"
            } else {
                "M"
            };
            (f.path().to_string(), f.additions(), f.deletions(), badge)
        })
        .collect();

    let diff_for_tree = diff_state.clone();
    let app_for_tree = app_state.clone();
    let ref_for_tree = head_ref.to_string();
    div()
        .id("mr-file-tree")
        .size_full()
        .overflow_y_scroll()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .p_2()
        .gap_1()
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("Changed files ({raw_count})")),
        )
        .children(file_items.into_iter().enumerate().map(|(ix, (path, add, del, badge))| {
            let state = diff_for_tree.clone();
            let scroll_path = path.clone();
            let open_state = app_for_tree.clone();
            let open_ref = ref_for_tree.clone();
            let open_path = path.clone();
            let can_view = badge != "D";
            let badge_status = match badge {
                "A" => "success",
                "D" => "failed",
                _ => "manual",
            };
            div()
                .id(SharedString::from(format!("mr-tree-{ix}")))
                .px_2()
                .py_1()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .flex()
                .items_center()
                .justify_between()
                .gap_1p5()
                .on_click(move |_, _, cx| {
                    state.update(cx, |ds, cx| ds.scroll_to_file(&scroll_path, cx));
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .min_w_0()
                        .child(div().text_xs().text_color(status_color(badge_status, dark)).child(badge))
                        .child(div().text_xs().text_color(theme.foreground).truncate().child(path)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .flex_shrink_0()
                        .child(div().text_color(theme.success).child(format!("+{add}")))
                        .child(div().text_color(theme.danger).child(format!("-{del}")))
                        .children(can_view.then(|| {
                            Button::new(SharedString::from(format!("mr-view-file-{ix}")))
                                .ghost()
                                .xsmall()
                                .label("↗")
                                .on_click(move |_, _, cx| {
                                    if let Some(pid) = open_state.read(cx).project.as_ref().map(|p| p.id) {
                                        open_state.update(cx, |s, cx| {
                                            s.open_repo_file(pid, Some(open_ref.clone()), open_path.clone(), None, None, cx);
                                        });
                                    }
                                })
                        })),
                )
        }))
        .children(fallback_notices.iter().enumerate().map(|(ix, (path, note))| {
            div()
                .id(SharedString::from(format!("mr-notice-{ix}")))
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("⚠ {path}: {note}"))
        }))
}

pub fn render_mr_diff_toolbar(
    diff_state: &Entity<DiffState>,
    has_truncated: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let mode = diff_state.read(cx).mode();
    let ds_mode = diff_state.clone();
    let ds_collapse = diff_state.clone();
    let ds_expand = diff_state.clone();
    let ds_prev = diff_state.clone();
    let ds_next = diff_state.clone();

    div()
        .flex().items_center().justify_between().gap_2().px_3().py_1p5()
        .border_b_1().border_color(theme.border).bg(theme.list_head)
        .child(
            div().flex().items_center().gap_1()
                .child(
                    Button::new("diff-mode-toggle").outline().xsmall()
                        .label(if mode == DiffMode::Split { "Split" } else { "Unified" })
                        .on_click(move |_, _, cx| {
                            let next = if mode == DiffMode::Split { DiffMode::Unified } else { DiffMode::Split };
                            ds_mode.update(cx, |ds, cx| ds.set_mode(next, cx));
                        }),
                )
                .child(
                    Button::new("diff-collapse").ghost().xsmall().label("Collapse all")
                        .on_click(move |_, _, cx| ds_collapse.update(cx, |ds, cx| ds.set_all_files_collapsed(true, cx))),
                )
                .child(
                    Button::new("diff-expand").ghost().xsmall().label("Expand all")
                        .on_click(move |_, _, cx| ds_expand.update(cx, |ds, cx| ds.set_all_files_collapsed(false, cx))),
                ),
        )
        .child(
            div().flex().items_center().gap_1()
                .children(has_truncated.then(|| div().text_xs().text_color(theme.warning).child("Truncated hunks present")))
                .child(
                    Button::new("diff-prev").ghost().xsmall().label("Prev")
                        .on_click(move |_, _, cx| ds_prev.update(cx, |ds, cx| ds.previous_change(cx))),
                )
                .child(
                    Button::new("diff-next").ghost().xsmall().label("Next")
                        .on_click(move |_, _, cx| ds_next.update(cx, |ds, cx| ds.next_change(cx))),
                ),
        )
}
