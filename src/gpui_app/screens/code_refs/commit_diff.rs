use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        diff::{Diff, DiffMode},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;

use super::super::super::safe_url::safe_open_url;
use super::commits::CommitsView;

pub fn render_commit_diff_pane(view: &CommitsView, cx: &mut Context<CommitsView>) -> impl IntoElement {
    let theme = cx.theme();
    let Some(commit) = view.selected_commit() else {
        return div()
            .size_full().flex().items_center().justify_center()
            .text_xs().text_color(theme.muted_foreground)
            .child("Select a commit to inspect its diff")
            .into_any_element();
    };

    let (full_sha, retry_sha, browse_sha) = (commit.id.clone(), commit.id.clone(), commit.id.clone());
    let short_sha = commit.display_short_id().to_string();
    let rel = commit.timestamp().map(format_relative).unwrap_or_default();
    let author_line = if rel.is_empty() { commit.author_name.clone() } else { format!("{} · {rel}", commit.author_name) };
    let (body, web) = (commit.extra_body().map(str::to_owned), commit.web_url.clone());
    let (copy_state, browse_state) = (view.state.clone(), view.state.clone());

    let header = div()
        .w_full().px_4().py_3().bg(theme.list_head).border_b_1().border_color(theme.border)
        .flex().flex_col().gap_1p5()
        .child(
            div().w_full().flex().items_center().justify_between().gap_3()
                .child(div().flex_1().min_w_0().text_sm().font_semibold().text_color(theme.foreground).truncate().child(commit.title.clone()))
                .child(
                    div().flex().items_center().gap_1().flex_shrink_0()
                        .child(
                            Button::new("commit-copy-sha").outline().xsmall().icon(Icon::new(IconName::Copy).size_3()).label(short_sha)
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone()));
                                    copy_state.update(cx, |s, cx| s.set_status("Copied commit SHA", cx));
                                }),
                        )
                        .child(
                            Button::new("commit-browse-files").outline().xsmall().label("Browse files")
                                .on_click(move |_, _, cx| {
                                    browse_state.update(cx, |s, cx| s.open_overview_ref(browse_sha.clone(), String::new(), cx));
                                }),
                        )
                        .children(web.map(|u| {
                            Button::new("commit-open-web").ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3p5())
                                .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                        })),
                ),
        )
        .child(div().text_xs().text_color(theme.muted_foreground).child(author_line))
        .children(body.map(|b| {
            div()
                .id("commit-body-scroll")
                .w_full()
                .max_h(px(140.))
                .overflow_y_scroll()
                .pt_1()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(b)
        }));

    let parsed_count = view.diff_state.read(cx).files().len();
    let total_files = view.raw_diff_count.max(parsed_count);
    let mode = view.diff_state.read(cx).mode();
    let toolbar = (!view.diff_loading && view.diff_error.is_none() && total_files > 0).then(|| {
        let (ds_mode, ds_collapse, ds_expand) = (view.diff_state.clone(), view.diff_state.clone(), view.diff_state.clone());
        let files_lbl = if total_files == 1 { "1 changed file".into() } else { format!("{total_files} changed files") };
        div()
            .id("commit-diff-toolbar").w_full().px_4().py_1p5()
            .border_b_1().border_color(theme.border).bg(theme.list_head)
            .flex().items_center().justify_between().gap_2()
            .child(div().text_xs().text_color(theme.muted_foreground).child(files_lbl))
            .child(
                div().flex().items_center().gap_1()
                    .children(view.has_truncated.then(|| div().text_xs().text_color(theme.warning).child("Truncated hunks")))
                    .child(
                        Button::new("commit-diff-mode").outline().xsmall()
                            .label(if mode == DiffMode::Split { "Split" } else { "Unified" })
                            .on_click(move |_, _, cx| {
                                let next = if mode == DiffMode::Split { DiffMode::Unified } else { DiffMode::Split };
                                ds_mode.update(cx, |ds, cx| ds.set_mode(next, cx));
                            }),
                    )
                    .child(Button::new("commit-diff-collapse").ghost().xsmall().label("Collapse all")
                        .on_click(move |_, _, cx| ds_collapse.update(cx, |ds, cx| ds.set_all_files_collapsed(true, cx))))
                    .child(Button::new("commit-diff-expand").ghost().xsmall().label("Expand all")
                        .on_click(move |_, _, cx| ds_expand.update(cx, |ds, cx| ds.set_all_files_collapsed(false, cx)))),
            )
            .test_support()
    });

    let notices = (!view.diff_loading && view.diff_error.is_none() && !view.fallback_notices.is_empty()).then(|| {
        div()
            .id("commit-diff-notices").w_full().px_4().py_2()
            .border_b_1().border_color(theme.border).bg(theme.sidebar).flex().flex_col().gap_1()
            .children(view.fallback_notices.iter().enumerate().map(|(ix, (path, note))| {
                div().id(SharedString::from(format!("commit-diff-notice-{ix}")))
                    .text_xs().text_color(theme.muted_foreground).child(format!("⚠ {path}: {note}"))
            }))
            .test_support()
    });

    let diff_body = if view.diff_loading {
        div().size_full().flex().items_center().justify_center()
            .text_xs().text_color(theme.muted_foreground).child("Loading commit diff…").into_any_element()
    } else if let Some(err) = view.diff_error.clone() {
        div()
            .id("commit-diff-error").size_full().p_6().flex().flex_col().items_center().justify_center().gap_2()
            .child(div().text_sm().font_semibold().text_color(theme.danger).child("Failed to load commit diff"))
            .child(div().text_xs().text_color(theme.muted_foreground).child(err))
            .child(Button::new("commit-diff-retry").outline().xsmall().label("Retry")
                .on_click(cx.listener(move |v, _, _, cx| v.select_commit(retry_sha.clone(), cx))))
            .test_support().into_any_element()
    } else if parsed_count == 0 {
        if view.fallback_notices.is_empty() {
            div().id("commit-diff-empty").size_full().flex().items_center().justify_center()
                .text_xs().text_color(theme.muted_foreground).child("No changes in this commit")
                .test_support().into_any_element()
        } else {
            div().size_full().into_any_element()
        }
    } else {
        div().id("commit-diff-viewer").size_full()
            .child(Diff::new(&view.diff_state).size_full().line_number(true).syntax_highlight(true))
            .test_support().into_any_element()
    };

    div()
        .size_full().flex().flex_col()
        .child(header).children(toolbar).children(notices)
        .child(div().flex_1().min_h_0().w_full().flex().flex_col().child(diff_body))
        .into_any_element()
}
