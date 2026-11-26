use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, Context, IntoElement, ParentElement as _, Styled as _,
};

use crate::gitlab::RepoTreeEntry;

use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::ProjectOverviewView;

pub fn render_readme_card(
    readme_entry: &RepoTreeEntry,
    readme_text: &str,
    md_ctx: MarkdownProjectContext,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement {
    let entry = readme_entry.clone();
    let md = render_markdown_with_uploads(
        format!("ov-readme-{}", readme_entry.id),
        readme_text.to_string(),
        md_ctx,
        cx,
    );
    let theme = cx.theme();
    div()
        .w_full()
        .mt_4()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .px_3p5()
                .py_2()
                .bg(theme.list_head)
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(theme.foreground)
                        .child(Icon::new(IconName::BookOpen).size_3p5().text_color(theme.muted_foreground))
                        .child(readme_entry.name.clone()),
                )
                .child(
                    Button::new("ov-readme-open-src")
                        .ghost()
                        .xsmall()
                        .label("View source")
                        .on_click(cx.listener(move |this, _, _, cx| this.open_file(&entry, cx))),
                ),
        )
        .child(div().p_5().bg(theme.background).child(md))
}
