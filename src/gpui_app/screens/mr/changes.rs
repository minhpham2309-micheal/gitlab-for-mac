use std::collections::HashMap;

use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        diff::{Diff, DiffAnnotation, DiffLinePosition},
        input::Input,
        resizable::{h_resizable, resizable_panel},
        ActiveTheme as _, Sizable as _,
    },
    div, px, Context, IntoElement, ParentElement as _, SharedString, Styled as _,
};

use crate::gitlab::{Discussion, MergeRequest};

use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::annotations::discussion_line_position;
use super::changes_chrome::{render_mr_diff_toolbar, render_mr_file_tree};
use super::MergeRequestsView;

pub fn render_mr_changes(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    md_ctx: MarkdownProjectContext,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement {
    let head_sha = mr.diff_refs.as_ref().and_then(|r| r.head_sha.as_deref());

    let mut annotations = Vec::new();
    let mut disc_map: HashMap<String, Discussion> = HashMap::new();
    for disc in &view.discussions {
        if let Some(pos) = discussion_line_position(disc, head_sha) {
            let key = format!("disc-{}", disc.id);
            disc_map.insert(key.clone(), disc.clone());
            annotations.push(DiffAnnotation::line(SharedString::from(key), pos));
        }
    }
    let can_comment = view.mr_permissions(mr, cx).can_comment;
    if can_comment && let Some(target) = view.inline_target.as_ref() {
        annotations.push(DiffAnnotation::line("inline-composer", target.clone()));
    }

    let view_ref = head_sha
        .map(str::to_owned)
        .unwrap_or_else(|| mr.source_branch.clone());
    let file_tree = render_mr_file_tree(view, &view_ref, cx);
    let toolbar = render_mr_diff_toolbar(view, cx);
    let view_weak = cx.entity().downgrade();
    let view_add = cx.entity().downgrade();
    let composer_input = view.inline_input.clone();

    let mut diff_el = Diff::new(&view.diff_state)
        .size_full()
        .line_number(true)
        .syntax_highlight(true)
        .annotations(annotations);
    if can_comment {
        diff_el = diff_el.on_add_annotation(move |range, window, cx| {
            let pos = DiffLinePosition::new(range.path().clone(), range.end_side(), range.end());
            let _ = view_add.update(cx, |this, cx| this.open_inline_composer(pos, window, cx));
        });
    }
    let diff_el = diff_el
        .render_annotation(move |ann, _, cx| {
            let theme = cx.theme();
            let id_str = format!("{}", ann.id());
            if id_str == "inline-composer" {
                let weak_submit = view_weak.clone();
                let weak_cancel = view_weak.clone();
                return div()
                    .p_2()
                    .my_1()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.primary)
                    .bg(theme.popover)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&composer_input)))
                    .child(
                        Button::new("inline-post")
                            .primary()
                            .xsmall()
                            .label("Add comment")
                            .on_click(move |_, window, cx| {
                                let _ = weak_submit
                                    .update(cx, |this, cx| this.submit_inline_comment(window, cx));
                            }),
                    )
                    .child(
                        Button::new("inline-cancel")
                            .ghost()
                            .xsmall()
                            .label("Cancel")
                            .on_click(move |_, _, cx| {
                                let _ = weak_cancel
                                    .update(cx, |this, cx| this.cancel_inline_composer(cx));
                            }),
                    )
                    .into_any_element();
            }
            if let Some(disc) = disc_map.get(&id_str) {
                let mut card = div()
                    .p_2()
                    .my_1()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.list_head)
                    .flex()
                    .flex_col()
                    .gap_1();
                for note in &disc.notes {
                    let author = note
                        .author
                        .as_ref()
                        .map(|a| format!("@{}", a.username))
                        .unwrap_or_else(|| "reviewer".into());
                    let md_el = render_markdown_with_uploads(
                        format!("diff-ann-{}", note.id),
                        note.body.clone(),
                        md_ctx.clone(),
                        cx,
                    );
                    let muted = cx.theme().muted_foreground;
                    card = card.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(div().text_xs().text_color(muted).child(author))
                            .child(md_el),
                    );
                }
                return card.into_any_element();
            }
            div().into_any_element()
        });

    h_resizable("mr-changes-split")
        .child(
            resizable_panel()
                .size(px(230.))
                .size_range(px(160.)..px(420.))
                .child(file_tree),
        )
        .child(
            resizable_panel().child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .child(div().flex_1().min_h_0().child(diff_el)),
            ),
        )
}
