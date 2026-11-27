use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::Textarea,
        ActiveTheme as _, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::MergeRequest;

use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::MergeRequestsView;

pub fn render_comment_composer(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    md_ctx: &MarkdownProjectContext,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    if !view.mr_permissions(mr, cx).can_comment {
        return div().w_full().pb_4().into_any_element();
    }
    let preview = view.comment_preview;
    let draft = view.comment_input.read(cx).value().to_string();
    let preview_body = preview.then(|| {
        let src = if draft.trim().is_empty() { "_Nothing to preview._".to_string() } else { draft };
        render_markdown_with_uploads(format!("mr-comment-preview-{}", mr.iid), src, md_ctx.clone(), cx)
    });

    let theme = cx.theme();
    let tab_btn = |id: &'static str, label: &'static str, active: bool| {
        div()
            .id(id).px_2().py_1().rounded_sm().cursor_pointer().text_xs()
            .bg(if active { theme.secondary } else { gpui_kit::transparent_black() })
            .text_color(if active { theme.foreground } else { theme.muted_foreground })
            .hover(|s| s.text_color(theme.foreground))
            .child(if active { div().font_semibold().child(label) } else { div().child(label) })
    };

    let fmt_btns: [(&str, &str, &str); 5] = [
        ("mr-fmt-bold", "B", "**bold**"),
        ("mr-fmt-italic", "I", "_italic_"),
        ("mr-fmt-code", "</>", "`code`"),
        ("mr-fmt-quote", "❝", "\n> "),
        ("mr-fmt-list", "•", "\n- "),
    ];

    let toolbar = div()
        .w_full().px_2().py_1p5().border_b_1().border_color(theme.border.opacity(0.7))
        .flex().items_center().justify_between().gap_2()
        .child(
            div().flex().items_center().gap_1()
                .child(tab_btn("mr-composer-write", "Write", !preview).on_click(cx.listener(|this, _, _, cx| {
                    this.comment_preview = false;
                    this.overview_list.remeasure();
                    cx.notify();
                })).test_support())
                .child(tab_btn("mr-composer-preview", "Preview", preview).on_click(cx.listener(|this, _, _, cx| {
                    this.comment_preview = true;
                    this.overview_list.remeasure();
                    cx.notify();
                })).test_support()),
        )
        .children((!preview).then(|| {
            div().flex().items_center().gap_0p5().children(fmt_btns.into_iter().map(|(id, lbl, snippet)| {
                div()
                    .id(SharedString::from(id)).px_1p5().py_0p5().rounded_sm().cursor_pointer()
                    .text_xs().text_color(theme.muted_foreground)
                    .hover(|s| s.bg(theme.secondary_hover).text_color(theme.foreground))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.append_comment_snippet(snippet, window, cx);
                    }))
                    .child(lbl)
            }))
        }));

    let body = if let Some(prev) = preview_body {
        div().min_h(px(108.)).p_3().child(prev).into_any_element()
    } else {
        div()
            .p_2()
            .child(Textarea::new(&view.comment_input).h(px(108.)).appearance(false).bordered(false))
            .into_any_element()
    };

    div()
        .id("mr-comment-composer")
        .w_full().pt_2().pb_6()
        .flex().flex_col().gap_2()
        .child(
            div().w_full().rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
                .flex().flex_col()
                .child(toolbar)
                .child(body),
        )
        .child(
            div().flex().items_center().justify_end().child(
                Button::new("mr-new-comment").primary().small().label("Comment")
                    .on_click(cx.listener(|this, _, window, cx| this.submit_mr_comment(window, cx))),
            ),
        )
        .test_support()
        .into_any_element()
}
