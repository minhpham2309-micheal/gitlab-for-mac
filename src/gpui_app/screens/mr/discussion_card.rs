use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{Discussion, GitLab};

use super::super::super::avatar::{avatar_size_sm, render_avatar};
use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::annotations::is_outdated_diff_discussion;
use super::MergeRequestsView;

#[allow(clippy::too_many_arguments)]
pub fn render_discussion_card(
    disc: &Discussion,
    mr_author_id: Option<u64>,
    head_sha: Option<&str>,
    is_replying: bool,
    can_resolve: bool,
    can_comment: bool,
    reply_input: &Entity<InputState>,
    md_ctx: &MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> Option<impl IntoElement + use<>> {
    let non_sys: Vec<_> = disc.notes.iter().filter(|n| !n.system).collect();
    if non_sys.is_empty() {
        return None;
    }
    let outdated = is_outdated_diff_discussion(disc, head_sha);
    let first = non_sys[0];
    let (resolvable, resolved) = (first.resolvable, first.resolved);
    let (disc_id, disc_id_reply) = (disc.id.clone(), disc.id.clone());

    let note_els: Vec<_> = non_sys
        .iter()
        .enumerate()
        .map(|(ix, note)| {
            let username = note.author.as_ref().map(|a| a.username.clone()).unwrap_or_else(|| "user".into());
            let display_name = note.author.as_ref().map(|a| a.name.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| username.clone());
            let is_mr_author = mr_author_id.is_some() && note.author.as_ref().map(|a| a.id) == mr_author_id;
            let rel = note.created_at.as_deref().map(format_relative).unwrap_or_default();
            let av = render_avatar(
                note.author.as_ref().and_then(|a| a.avatar_url.as_deref()),
                &username, avatar_size_sm(), client, cx,
            );
            let md_el = render_markdown_with_uploads(format!("mr-note-{}", note.id), note.body.clone(), md_ctx.clone(), cx);
            let theme = cx.theme();
            let mut row = div().flex().flex_col().gap_1p5();
            if ix > 0 {
                row = row.pt_2p5().border_t_1().border_color(theme.border.opacity(0.45));
            }
            let right_badges = div().flex().items_center().gap_1().flex_shrink_0()
                .children(is_mr_author.then(|| {
                    div().px_1p5().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border)
                        .text_xs().text_color(theme.muted_foreground).child("Author")
                }))
                .children((ix == 0 && outdated).then(|| {
                    div().px_1p5().py(px(1.)).rounded_full().bg(theme.warning.opacity(0.2))
                        .text_xs().text_color(theme.warning).child("Outdated diff")
                }))
                .children((ix == 0 && resolved).then(|| {
                    div().px_1p5().py(px(1.)).rounded_full().bg(theme.success.opacity(0.2))
                        .text_xs().text_color(theme.success).child("Resolved")
                }));
            row.child(
                div().w_full().flex().items_center().justify_between().gap_2()
                    .child(
                        div().flex().items_center().flex_wrap().gap_1p5().min_w_0()
                            .child(av)
                            .child(div().text_xs().font_semibold().text_color(theme.foreground).child(display_name))
                            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("@{username}")))
                            .children((!rel.is_empty()).then(|| div().text_xs().text_color(theme.muted_foreground).child(format!("· {rel}")))),
                    )
                    .child(right_badges),
            )
            .child(div().pl_7().child(md_el))
        })
        .collect();

    let theme = cx.theme();
    let show_resolve = resolvable && can_resolve;
    let resolve_btn = show_resolve.then(|| {
        let next_res = !resolved;
        Button::new(SharedString::from(format!("res-{}", disc.id))).outline().small()
            .icon(Icon::new(IconName::CircleCheck).size_3p5())
            .label(if resolved { "Unresolve thread" } else { "Resolve thread" })
            .on_click(cx.listener(move |this, _, _, cx| this.resolve_discussion(disc_id.clone(), next_res, cx)))
    });

    let mut card = div()
        .id(SharedString::from(format!("mr-disc-card-{}", disc.id)))
        .rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
        .flex().flex_col()
        .child(div().p_3().flex().flex_col().gap_2().children(note_els))
        .test_support();

    if is_replying && can_comment {
        card = card.child(
            div().px_3().py_2p5().border_t_1().border_color(theme.border.opacity(0.6))
                .flex().items_center().gap_2()
                .child(div().flex_1().min_w_0().child(Input::new(reply_input).small()))
                .child(
                    Button::new(SharedString::from(format!("send-rep-{}", disc.id))).primary().small().label("Reply")
                        .on_click(cx.listener(|this, _, window, cx| this.submit_reply(window, cx))),
                )
                .child(
                    Button::new(SharedString::from(format!("cancel-rep-{}", disc.id))).ghost().small().label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_reply(cx))),
                )
                .children(resolve_btn),
        );
    } else if can_comment || show_resolve {
        let reply_box = can_comment.then(|| {
            div()
                .id(SharedString::from(format!("rep-{}", disc.id)))
                .flex_1().min_w_0().px_3().py_1p5().rounded_md().cursor_pointer()
                .bg(theme.background).border_1().border_color(theme.input)
                .hover(|s| s.border_color(theme.ring))
                .text_xs().text_color(theme.muted_foreground)
                .on_click(cx.listener(move |this, _, window, cx| this.start_reply(disc_id_reply.clone(), window, cx)))
                .child("Reply…")
                .test_support()
        });
        card = card.child(
            div().px_3().py_2().border_t_1().border_color(theme.border.opacity(0.6))
                .flex().items_center().justify_between().gap_2()
                .children(reply_box)
                .children(resolve_btn),
        );
    }
    Some(card)
}
