use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, AnyElement, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{GitLab, Issue, Note};

use super::super::avatar::{avatar_size_sm, render_avatar};
use super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::super::pajamas_theme::status_badge_palette;
use super::super::safe_url::safe_open_url;
use super::issues::IssuesView;

#[allow(clippy::too_many_arguments)]
pub fn render_issue_detail(
    issue: Option<Issue>,
    notes: &[Note],
    comment_input: &Entity<InputState>,
    md_ctx: MarkdownProjectContext,
    client: Option<&GitLab>,
    can_close: bool,
    can_comment: bool,
    cx: &mut Context<IssuesView>,
) -> AnyElement {
    let Some(issue) = issue else {
        return div().size_full().flex().items_center().justify_center()
            .text_color(cx.theme().muted_foreground)
            .child("Select a work item to view details and discussion")
            .into_any_element();
    };

    let is_open = issue.state == "opened";
    let web_url = issue.web_url.clone();
    let desc = issue.description.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "_No description._".into());
    let desc_el = render_markdown_with_uploads(format!("iss-desc-{}", issue.iid), desc, md_ctx.clone(), cx);
    let author_el = issue.author.as_ref().map(|a| {
        let av = render_avatar(a.avatar_url.as_deref(), &a.name, avatar_size_sm(), client, cx);
        let rel = issue.created_at.as_deref().map(format_relative).unwrap_or_default();
        let meta = if rel.is_empty() { format!("opened by {}", a.name) } else { format!("opened {rel} by {}", a.name) };
        (av, meta)
    });

    let note_els: Vec<_> = notes.iter().map(|note| {
        let username = note.author.as_ref().map(|a| a.username.clone()).unwrap_or_else(|| "user".into());
        let av = render_avatar(note.author.as_ref().and_then(|a| a.avatar_url.as_deref()), &username, avatar_size_sm(), client, cx);
        let rel = note.created_at.as_deref().map(format_relative).unwrap_or_default();
        let md_el = render_markdown_with_uploads(format!("iss-note-{}", note.id), note.body.clone(), md_ctx.clone(), cx);
        let theme = cx.theme();
        div().p_3().rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
            .flex().flex_col().gap_1p5()
            .child(div().flex().items_center().gap_2()
                .child(av)
                .child(div().text_xs().font_semibold().text_color(theme.foreground).child(format!("@{username}")))
                .children((!rel.is_empty()).then(|| div().text_xs().text_color(theme.muted_foreground).child(rel))))
            .child(md_el)
    }).collect();

    let theme = cx.theme();
    let pal = status_badge_palette(&issue.state, theme.is_dark());
    div().id("issue-detail-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
        .child(
            div().w_full().max_w(px(1080.)).flex_1().min_h_0().flex().flex_col()
                .child(
                    div().pb_2p5().border_b_1().border_color(theme.border).flex().items_center().justify_between().gap_3()
                        .child(Button::new("issue-back-list").ghost().small().icon(Icon::new(IconName::ArrowLeft).size_3p5()).label("Work items")
                            .on_click(cx.listener(|v, _, _, cx| v.back_to_list(cx))))
                        .child(div().flex().items_center().gap_1p5()
                            .children((is_open && can_close).then(|| {
                                Button::new("issue-close").danger().small().label("Close item")
                                    .on_click(cx.listener(|this, _, w, cx| this.close_selected(w, cx)))
                            }))
                            .child(Button::new("issue-web").outline().small().icon(Icon::new(IconName::ExternalLink).size_3p5()).label("Open web")
                                .on_click(move |_, _, _| { let _ = safe_open_url(&web_url); }))),
                )
                .child(
                    div().id("issue-detail-scroll").flex_1().min_h_0().overflow_y_scroll().py_4().pr_1().flex().flex_col().gap_4()
                        .child(div().flex().flex_col().gap_2()
                            .child(div().text_xl().font_bold().text_color(theme.foreground).child(format!("#{} {}", issue.iid, issue.title)))
                            .child(div().flex().items_center().gap_2()
                                .child(div().px_2().py(px(1.5)).rounded_full().bg(pal.bg).text_color(pal.text).text_xs().font_semibold()
                                    .child(if is_open { "Open" } else { "Closed" }))
                                .children(author_el.map(|(av, meta)| div().flex().items_center().gap_1p5().child(av)
                                    .child(div().text_xs().text_color(theme.muted_foreground).child(meta))))))
                        .child(div().p_4().rounded_md().border_1().border_color(theme.border).bg(theme.list_head).child(desc_el))
                        .children(note_els)
                        .children(can_comment.then(|| {
                            div().flex().items_center().gap_2().pt_2()
                                .child(div().flex_1().child(Input::new(comment_input)))
                                .child(Button::new("issue-post-note").primary().small().label("Comment")
                                    .on_click(cx.listener(|this, _, w, cx| this.post_comment(w, cx))))
                        })),
                ),
        )
        .test_support().into_any_element()
}
