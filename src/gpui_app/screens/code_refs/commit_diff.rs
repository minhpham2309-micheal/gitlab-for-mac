use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        diff::Diff,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, ClipboardItem, Context, IntoElement, ParentElement as _, Styled as _,
};

use crate::gitlab::time_util::format_relative;

use super::super::super::safe_url::safe_open_url;
use super::commits::CommitsView;

pub fn render_commit_diff_pane(
    view: &CommitsView,
    cx: &mut Context<CommitsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let Some(commit) = view.selected_commit() else {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child("Select a commit to inspect its diff")
            .into_any_element();
    };

    let full_sha = commit.id.clone();
    let short_sha = commit.display_short_id().to_string();
    let author = commit.author_name.clone();
    let rel = commit.timestamp().map(format_relative).unwrap_or_default();
    let body = commit.extra_body().map(str::to_owned);
    let web = commit.web_url.clone();
    let copy_state = view.state.clone();
    let browse_state = view.state.clone();
    let browse_sha = commit.id.clone();

    let header = div()
        .w_full()
        .px_4()
        .py_3()
        .bg(theme.list_head)
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_sm().text_color(theme.foreground).truncate().child(commit.title.clone()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .flex_shrink_0()
                        .child(
                            Button::new("commit-copy-sha")
                                .outline()
                                .xsmall()
                                .icon(Icon::new(IconName::Copy).size_3())
                                .label(short_sha)
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone()));
                                    copy_state.update(cx, |s, cx| s.set_status("Copied commit SHA", cx));
                                }),
                        )
                        .child(
                            Button::new("commit-browse-files")
                                .outline()
                                .xsmall()
                                .label("Browse files")
                                .on_click(move |_, _, cx| {
                                    browse_state.update(cx, |s, cx| {
                                        s.open_overview_ref(browse_sha.clone(), String::new(), cx);
                                    });
                                }),
                        )
                        .children(web.map(|u| {
                            Button::new("commit-open-web")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(IconName::ExternalLink).size_3p5())
                                .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                        })),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("{author} · {rel}")),
        )
        .children(body.map(|b| {
            div()
                .pt_1()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(b)
        }));

    let diff_body = if view.diff_loading {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child("Loading commit diff…")
            .into_any_element()
    } else {
        Diff::new(&view.diff_state)
            .size_full()
            .line_number(true)
            .syntax_highlight(true)
            .into_any_element()
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(div().flex_1().min_h_0().child(diff_body))
        .into_any_element()
}
