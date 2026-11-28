use std::collections::{HashMap, HashSet};

use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, uniform_list, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::RepoTreeEntry;

use super::super::overview::file_icons::render_entry_icon;
use super::super::super::state::ScreenCommand;
use super::RepositoryView;

#[derive(Clone)]
pub struct FlatTreeRow {
    pub entry: RepoTreeEntry,
    pub depth: usize,
    pub expanded: bool,
}

pub fn flatten_tree(
    dirs: &HashMap<String, Vec<RepoTreeEntry>>,
    expanded: &HashSet<String>,
    filter: &str,
) -> Vec<FlatTreeRow> {
    let q = filter.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    if !q.is_empty() {
        let mut all: Vec<RepoTreeEntry> = dirs.values().flatten().cloned().collect();
        all.sort_by(|a, b| a.path.cmp(&b.path));
        all.dedup_by(|a, b| a.path == b.path);
        for entry in all {
            if entry.path.to_ascii_lowercase().contains(&q) {
                out.push(FlatTreeRow { entry, depth: 0, expanded: false });
            }
        }
        return out;
    }
    walk_dir("", 0, dirs, expanded, &mut out);
    out
}

fn walk_dir(
    dir_path: &str,
    depth: usize,
    dirs: &HashMap<String, Vec<RepoTreeEntry>>,
    expanded: &HashSet<String>,
    out: &mut Vec<FlatTreeRow>,
) {
    let Some(children) = dirs.get(dir_path) else { return; };
    for entry in children {
        let is_exp = entry.is_dir() && expanded.contains(&entry.path);
        out.push(FlatTreeRow { entry: entry.clone(), depth, expanded: is_exp });
        if is_exp {
            walk_dir(&entry.path, depth + 1, dirs, expanded, out);
        }
    }
}

pub fn render_tree_panel(view: &RepositoryView, cx: &mut Context<RepositoryView>) -> impl IntoElement {
    let theme = cx.theme();
    let active_path = view.active_path.clone();
    let filter_q = view.tree_filter.read(cx).value().to_string();
    let rows = flatten_tree(&view.dirs, &view.expanded_dirs, &filter_q);
    let count = rows.len();
    let current_ref = view.active_ref.clone();
    let find_state = view.state.clone();
    let list_el = uniform_list(
        "repo-file-tree-list",
        count,
        cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
            range
                .map(|ix| {
                    let row = &rows[ix];
                    let is_dir = row.entry.is_dir();
                    let is_sel = active_path.as_deref() == Some(row.entry.path.as_str());
                    let entry = row.entry.clone();
                    let indent = px(8.0 + (row.depth as f32) * 14.0);
                    let icon = render_entry_icon(&row.entry.name, is_dir, cx);
                    let theme = cx.theme();
                    let chevron = if is_dir {
                        Icon::new(if row.expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                            .size_3()
                            .text_color(theme.muted_foreground)
                            .into_any_element()
                    } else {
                        div().w(px(12.)).into_any_element()
                    };
                    div()
                        .id(SharedString::from(format!("repo-tree-row-{ix}")))
                        .h(px(26.))
                        .pl(indent)
                        .pr_2()
                        .cursor_pointer()
                        .bg(if is_sel { theme.list_active } else { gpui_kit::transparent_black() })
                        .hover(|s| s.bg(theme.list_hover))
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if is_dir {
                                this.toggle_dir(entry.path.clone(), cx);
                            } else {
                                this.open_entry(&entry, window, cx);
                            }
                        }))
                        .child(chevron)
                        .child(icon)
                        .child(div().text_xs().text_color(theme.foreground).truncate().child(row.entry.name.clone()))
                        .into_any_element()
                })
                .collect()
        }),
    )
    .flex_1()
    .size_full();

    div()
        .size_full()
        .bg(theme.background)
        .border_r_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .child(
            div()
                .px_2p5()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .min_w_0()
                                .child(Icon::empty().path("icons/glab-branch.svg").size_3().text_color(theme.muted_foreground))
                                .child(div().text_xs().text_color(theme.foreground).truncate().child(current_ref)),
                        )
                        .child(
                            Button::new("repo-tree-find")
                                .ghost()
                                .xsmall()
                                .label("⌘T")
                                .on_click(move |_, _, cx| {
                                    find_state.update(cx, |s, cx| s.dispatch_command(ScreenCommand::OpenFindFile, cx));
                                }),
                        ),
                )
                .child(Input::new(&view.tree_filter).xsmall()),
        )
        .child(div().flex_1().min_h_0().child(list_el))
}
