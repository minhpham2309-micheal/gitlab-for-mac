use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        ActiveTheme as _, Sizable as _,
    },
    div, px, AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::{GitLab, Issue, Note};

use super::super::avatar::{avatar_size_sm, render_avatar};
use super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::super::pajamas_theme::status_color;
use super::super::safe_url::safe_open_url;
use super::issues::IssuesView;

pub fn render_issue_detail(
    issue: Option<Issue>,
    notes: &[Note],
    comment_input: &Entity<InputState>,
    md_ctx: MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<IssuesView>,
) -> AnyElement {
    let Some(issue) = issue else {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(cx.theme().muted_foreground)
            .child("Select an issue to view details and discussion")
            .into_any_element();
    };

    let is_open = issue.state == "opened";
    let web_url = issue.web_url.clone();
    let desc = issue
        .description
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "_No description._".into());
    let desc_el = render_markdown_with_uploads(format!("iss-desc-{}", issue.iid), desc, md_ctx.clone(), cx);
    let note_els: Vec<_> = notes
        .iter()
        .map(|note| {
            let username = note
                .author
                .as_ref()
                .map(|a| a.username.clone())
                .unwrap_or_else(|| "user".into());
            let av = render_avatar(
                note.author.as_ref().and_then(|a| a.avatar_url.as_deref()),
                &username,
                avatar_size_sm(),
                client,
                cx,
            );
            let md_el = render_markdown_with_uploads(
                format!("iss-note-{}", note.id),
                note.body.clone(),
                md_ctx.clone(),
                cx,
            );
            let theme = cx.theme();
            div()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .child(av)
                        .child(div().text_xs().text_color(theme.muted_foreground).child(format!("@{username}"))),
                )
                .child(md_el)
        })
        .collect();

    let theme = cx.theme();
    let dark = theme.is_dark();
    div()
        .id("issue-detail-scroll")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().text_base().text_color(theme.foreground).child(format!("#{} {}", issue.iid, issue.title)))
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            div()
                                .px_2()
                                .py(px(1.))
                                .rounded_full()
                                .bg(status_color(&issue.state, dark).opacity(0.2))
                                .text_color(status_color(&issue.state, dark))
                                .text_xs()
                                .child(issue.state.clone()),
                        )
                        .children(is_open.then(|| {
                            Button::new("issue-close")
                                .danger()
                                .xsmall()
                                .label("Close")
                                .on_click(cx.listener(|this, _, w, cx| this.close_selected(w, cx)))
                        }))
                        .child(
                            Button::new("issue-web")
                                .ghost()
                                .xsmall()
                                .label("Open web")
                                .on_click(move |_, _, _| {
                                    let _ = safe_open_url(&web_url);
                                }),
                        ),
                ),
        )
        .child(
            div()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .bg(theme.sidebar)
                .child(desc_el),
        )
        .children(note_els)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_2()
                .child(div().flex_1().child(Input::new(comment_input)))
                .child(
                    Button::new("issue-post-note")
                        .primary()
                        .small()
                        .label("Comment")
                        .on_click(cx.listener(|this, _, w, cx| this.post_comment(w, cx))),
                ),
        )
        .into_any_element()
}
