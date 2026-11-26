use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        ActiveTheme as _, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::{GitLab, MergeRequest};

use super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::super::pajamas_theme::status_color;
use super::merge_requests::MergeRequestsView;
use super::mr_discussion_card::render_discussion_card;

pub fn render_mr_overview(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    md_ctx: MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement {
    let head_sha = mr.diff_refs.as_ref().and_then(|r| r.head_sha.as_deref());
    let desc = mr
        .description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("_No description provided._")
        .to_string();

    let desc_block = render_markdown_with_uploads(format!("mr-desc-{}", mr.iid), desc, md_ctx.clone(), cx);
    let cards: Vec<_> = view
        .discussions
        .iter()
        .filter_map(|disc| {
            let is_replying = view.replying_to.as_deref() == Some(disc.id.as_str());
            render_discussion_card(disc, head_sha, is_replying, &view.reply_input, &md_ctx, client, cx)
        })
        .collect();

    let theme = cx.theme();
    let dark = theme.is_dark();
    let mut body = div()
        .id("mr-overview-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_4()
        .p_4();

    if !mr.labels.is_empty() || mr.draft || !mr.reviewers.is_empty() {
        let mut meta = div().flex().flex_wrap().items_center().gap_1p5();
        if mr.draft {
            meta = meta.child(
                div()
                    .px_2()
                    .py(px(1.))
                    .rounded_full()
                    .bg(status_color("warning", dark).opacity(0.2))
                    .text_color(status_color("warning", dark))
                    .text_xs()
                    .child("Draft"),
            );
        }
        for label in &mr.labels {
            meta = meta.child(
                div()
                    .px_2()
                    .py(px(1.))
                    .rounded_full()
                    .bg(theme.secondary)
                    .text_color(theme.foreground)
                    .text_xs()
                    .child(label.clone()),
            );
        }
        if !mr.reviewers.is_empty() {
            let names = mr
                .reviewers
                .iter()
                .map(|r| format!("@{}", r.username))
                .collect::<Vec<_>>()
                .join(", ");
            meta = meta.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("Reviewers: {names}")),
            );
        }
        body = body.child(meta);
    }

    body = body.child(
        div()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(desc_block),
    );

    if let Some(err) = view.discussions_error.as_deref() {
        body = body.child(
            div()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(theme.warning)
                .bg(theme.warning.opacity(0.12))
                .text_xs()
                .text_color(theme.foreground)
                .child(format!("Discussions could not be loaded: {err}")),
        );
    }

    body.child(
        div()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(format!("Activity & discussions ({})", view.discussions.len())),
    )
    .children(cards)
    .child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(theme.border)
            .child(div().flex_1().child(Input::new(&view.comment_input)))
            .child(
                Button::new("mr-new-comment")
                    .primary()
                    .small()
                    .label("Comment")
                    .on_click(cx.listener(|this, _, window, cx| this.submit_mr_comment(window, cx))),
            ),
    )
}
