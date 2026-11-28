use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, Entity, IntoElement, ParentElement as _, Styled as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::RepoCommit;

use super::super::super::avatar::render_avatar;
use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::super::super::state::AppState;
use super::ProjectOverviewView;

pub fn render_commit_banner(
    commit: Option<&RepoCommit>,
    loading: bool,
    expanded: bool,
    app_state: &Entity<AppState>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let Some(commit) = commit else {
        return div()
            .w_full()
            .h(px(52.))
            .px_4()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.list_head)
            .flex()
            .items_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(if loading { "Loading latest commit…" } else { "No commits yet" });
    };

    let author = if commit.author_name.is_empty() { "author".to_string() } else { commit.author_name.clone() };
    let rel = commit.timestamp().map(format_relative).unwrap_or_default();
    let short_sha = commit.display_short_id().to_string();
    let full_sha = commit.id.clone();
    let title = commit.title.clone();
    let body = commit.extra_body().map(str::to_owned);
    let pipeline = commit.last_pipeline.clone();
    let av = render_avatar(None, &author, px(30.), None, cx);
    let theme = cx.theme();
    let dark = theme.is_dark();
    let copy_state = app_state.clone();
    let pipe_state = copy_state.clone();

    let pipe_btn = pipeline.map(|p| {
        let color = status_color(&p.status, dark);
        let icon_path = status_icon_path(&p.status);
        let pid = p.id;
        Button::new("ov-commit-pipeline")
            .ghost()
            .xsmall()
            .icon(Icon::empty().path(icon_path).size_4().text_color(color))
            .on_click(move |_, _, cx| pipe_state.update(cx, |s, cx| s.open_pipeline(pid, cx)))
    });

    div()
        .w_full()
        .px_3p5()
        .py_2p5()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.list_head)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .min_w_0()
                        .child(av)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .min_w_0()
                                        .child(div().text_sm().text_color(theme.foreground).truncate().child(title))
                                        .children(body.is_some().then(|| {
                                            Button::new("ov-commit-expand")
                                                .outline()
                                                .xsmall()
                                                .icon(Icon::new(IconName::Ellipsis).size_3())
                                                .on_click(cx.listener(|this, _, _, cx| this.toggle_commit_body(cx)))
                                        })),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{author} authored {rel}")),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .flex_shrink_0()
                        .children(pipe_btn)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.background)
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .font_family(theme.mono_font_family.clone())
                                        .text_xs()
                                        .text_color(theme.foreground)
                                        .child(short_sha),
                                )
                                .child(
                                    Button::new("ov-copy-sha")
                                        .ghost()
                                        .xsmall()
                                        .icon(Icon::new(IconName::Copy).size_3p5())
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone()));
                                            copy_state.update(cx, |s, cx| s.set_status("Copied commit SHA", cx));
                                        }),
                                ),
                        )
                        .child(
                            Button::new("ov-history-btn")
                                .outline()
                                .small()
                                .label("History")
                                .on_click(cx.listener(|this, _, _, cx| this.open_history(cx))),
                        ),
                ),
        )
        .children(expanded.then_some(body).flatten().map(|extra| {
            div()
                .pt_1p5()
                .pl(px(40.))
                .border_t_1()
                .border_color(theme.border.opacity(0.6))
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(extra)
        }))
}
