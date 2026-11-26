use gpui_kit::{
    component::{ActiveTheme as _, Icon, IconName},
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::RepoTreeEntry;

use super::file_icons::render_entry_icon;
use super::ProjectOverviewView;

pub fn parent_dir(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

pub fn render_file_table(
    entries: &[RepoTreeEntry],
    current_path: &str,
    tree_loading: bool,
    commits_loading: bool,
    show_commits: bool,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let mut table = div()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .px_3p5()
                .py_2()
                .bg(theme.list_head)
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .text_color(theme.foreground)
                .child(div().flex_grow(2.0).flex_basis(px(0.)).min_w_0().child("Name"))
                .children(show_commits.then(|| div().flex_grow(3.0).flex_basis(px(0.)).min_w_0().child("Last commit")))
                .children(show_commits.then(|| {
                    div().w(px(116.)).flex_shrink_0().text_right().child("Last update")
                })),
        );

    if !current_path.is_empty() {
        let up = parent_dir(current_path);
        table = table.child(
            div()
                .id("ov-row-parent")
                .w_full()
                .px_3p5()
                .py_2()
                .border_b_1()
                .border_color(theme.border.opacity(0.5))
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(theme.foreground)
                .on_click(cx.listener(move |this, _, _, cx| this.navigate_dir(up.clone(), cx)))
                .child(Icon::new(IconName::FolderOpen).size_4().text_color(theme.muted_foreground))
                .child(".."),
        );
    }

    if tree_loading && entries.is_empty() {
        return table.child(
            div()
                .px_4()
                .py_6()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Loading repository tree…"),
        );
    }

    if entries.is_empty() {
        return table.child(
            div()
                .px_4()
                .py_6()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Empty directory"),
        );
    }

    for (ix, entry) in entries.iter().enumerate() {
        let is_dir = entry.is_dir();
        let entry_clone = entry.clone();
        let icon = render_entry_icon(&entry.name, is_dir, cx);
        let theme = cx.theme();
        let commit_cell = show_commits.then(|| {
            if let Some(c) = entry.last_commit.as_ref() {
                div()
                    .flex_grow(3.0)
                    .flex_basis(px(0.))
                    .min_w_0()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .truncate()
                    .child(c.title.clone())
                    .into_any_element()
            } else if commits_loading {
                div()
                    .flex_grow(3.0)
                    .flex_basis(px(0.))
                    .min_w_0()
                    .child(div().h(px(10.)).w(px(150.)).rounded_sm().bg(theme.secondary))
                    .into_any_element()
            } else {
                div().flex_grow(3.0).flex_basis(px(0.)).min_w_0().into_any_element()
            }
        });
        let time_cell = show_commits.then(|| {
            let txt = entry
                .last_commit
                .as_ref()
                .and_then(|c| c.timestamp())
                .map(format_relative)
                .unwrap_or_default();
            div()
                .w(px(116.))
                .flex_shrink_0()
                .text_right()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(txt)
        });

        table = table.child(
            div()
                .id(SharedString::from(format!("ov-tree-row-{ix}")))
                .w_full()
                .px_3p5()
                .py_2()
                .border_b_1()
                .border_color(theme.border.opacity(0.45))
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .flex()
                .items_center()
                .gap_3()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if is_dir {
                        this.navigate_dir(entry_clone.path.clone(), cx);
                    } else {
                        this.open_file(&entry_clone, cx);
                    }
                }))
                .child(
                    div()
                        .flex_grow(2.0)
                        .flex_basis(px(0.))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(icon)
                        .child(div().text_xs().text_color(theme.foreground).truncate().child(entry.name.clone())),
                )
                .children(commit_cell)
                .children(time_cell),
        );
    }

    table
}
