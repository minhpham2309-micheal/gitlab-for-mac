use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        ActiveTheme as _, Sizable as _,
    },
    div, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _,
};

use crate::gitlab::{Discussion, GitLab};

use super::super::avatar::{avatar_size_sm, render_avatar};
use super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::merge_requests::MergeRequestsView;
use super::mr_annotations::is_outdated_diff_discussion;

#[allow(clippy::too_many_arguments)]
pub fn render_discussion_card(
    disc: &Discussion,
    head_sha: Option<&str>,
    is_replying: bool,
    reply_input: &Entity<InputState>,
    md_ctx: &MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> Option<impl IntoElement> {
    let non_sys: Vec<_> = disc.notes.iter().filter(|n| !n.system).collect();
    if non_sys.is_empty() {
        return None;
    }
    let outdated = is_outdated_diff_discussion(disc, head_sha);
    let first = non_sys[0];
    let resolvable = first.resolvable;
    let resolved = first.resolved;
    let disc_id = disc.id.clone();
    let disc_id_reply = disc.id.clone();

    let note_els: Vec<_> = non_sys
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
                format!("mr-note-{}", note.id),
                note.body.clone(),
                md_ctx.clone(),
                cx,
            );
            let theme = cx.theme();
            div()
                .flex()
                .flex_col()
                .gap_1()
                .pt_1()
                .border_t_1()
                .border_color(theme.border.opacity(0.4))
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
    let mut left = div().flex().items_center().gap_2();
    if outdated {
        left = left.child(
            div()
                .px_1p5()
                .rounded_sm()
                .bg(theme.warning.opacity(0.2))
                .text_color(theme.warning)
                .text_xs()
                .child("Outdated diff"),
        );
    }
    if resolved {
        left = left.child(
            div()
                .px_1p5()
                .rounded_sm()
                .bg(theme.success.opacity(0.2))
                .text_color(theme.success)
                .text_xs()
                .child("Resolved"),
        );
    }

    let mut right = div().flex().items_center().gap_1();
    if resolvable {
        let next_res = !resolved;
        right = right.child(
            Button::new(SharedString::from(format!("res-{}", disc.id)))
                .ghost()
                .xsmall()
                .label(if resolved { "Unresolve" } else { "Resolve" })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.resolve_discussion(disc_id.clone(), next_res, cx);
                })),
        );
    }
    right = right.child(
        Button::new(SharedString::from(format!("rep-{}", disc.id)))
            .ghost()
            .xsmall()
            .label("Reply")
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_reply(disc_id_reply.clone(), window, cx);
            })),
    );

    let mut card = div()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.colors.list)
        .flex()
        .flex_col()
        .gap_2()
        .child(div().flex().items_center().justify_between().gap_2().child(left).child(right))
        .children(note_els);

    if is_replying {
        card = card.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_2()
                .child(div().flex_1().child(Input::new(reply_input)))
                .child(
                    Button::new(SharedString::from(format!("send-rep-{}", disc.id)))
                        .primary()
                        .xsmall()
                        .label("Send")
                        .on_click(cx.listener(|this, _, window, cx| this.submit_reply(window, cx))),
                ),
        );
    }
    Some(card)
}
