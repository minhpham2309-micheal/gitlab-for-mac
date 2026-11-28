use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        spinner::Spinner,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, App, ClipboardItem, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, WeakEntity,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{GitLab, GroupTreeRow};

use super::super::super::avatar::render_entity_avatar;
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::AppState;
use super::GroupsView;

pub fn render_group_tree_row(
    row: &GroupTreeRow,
    client: Option<&GitLab>,
    view_weak: WeakEntity<GroupsView>,
    app_state: Entity<AppState>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    match row {
        GroupTreeRow::Subgroup { group, depth, expanded, loading, cached_subgroups, cached_projects } => {
            let gid = group.id;
            let name = group.display_name().to_string();
            let vis = group.visibility_label();
            let desc = group.description.clone().filter(|d| !d.trim().is_empty());
            let created_rel = group.created_at.as_deref().map(format_relative).unwrap_or_default();
            let av = render_entity_avatar(group.avatar_url.as_deref(), &name, px(32.), client, cx);
            let theme = cx.theme();
            let indent = px((*depth as f32) * 24.0);
            let (g_expand, g_enter) = (group.clone(), group.clone());
            let (w_expand, w_enter) = (view_weak.clone(), view_weak);
            let (copy_path, web_url) = (group.full_path.clone(), group.web_url.clone());

            let chevron = if *loading {
                div().w(px(22.)).flex().items_center().justify_center().child(Spinner::new().xsmall()).into_any_element()
            } else {
                div()
                    .id(SharedString::from(format!("group-expand-{gid}")))
                    .w(px(22.)).h(px(22.)).rounded_sm().cursor_pointer()
                    .hover(|s| s.bg(theme.secondary_hover))
                    .flex().items_center().justify_center()
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = w_expand.update(cx, |v, cx| v.toggle_expand(g_expand.clone(), cx));
                    })
                    .child(
                        Icon::new(if *expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                            .size_3p5().text_color(theme.muted_foreground),
                    )
                    .test_support()
                    .into_any_element()
            };

            let counts_meta = cached_subgroups.zip(*cached_projects).map(|(s, p)| {
                div().flex().items_center().gap_2p5().text_xs().text_color(theme.muted_foreground)
                    .child(div().flex().items_center().gap_1().child(Icon::new(IconName::FolderOpen).size_3()).child(s.to_string()))
                    .child(div().flex().items_center().gap_1().child(Icon::new(IconName::Folder).size_3()).child(p.to_string()))
            });

            div()
                .w_full().h(px(66.)).px_6().flex().flex_col().items_center()
                .child(
                    div()
                        .id(SharedString::from(format!("group-row-{gid}")))
                        .w_full().max_w(px(1120.)).h_full().pl(indent).pr_2()
                        .border_b_1().border_color(theme.border.opacity(0.6))
                        .cursor_pointer().hover(|s| s.bg(theme.list_hover.opacity(0.55)))
                        .flex().items_center().justify_between().gap_4()
                        .on_click(move |_, _, cx| {
                            let _ = w_enter.update(cx, |v, cx| v.enter_group(g_enter.clone(), true, cx));
                        })
                        .child(
                            div().flex_1().min_w_0().flex().items_center().gap_2p5()
                                .child(chevron)
                                .child(Icon::new(IconName::FolderOpen).size_4().text_color(theme.muted_foreground))
                                .child(av)
                                .child(
                                    div().flex_1().min_w_0().flex().flex_col().gap_0p5()
                                        .child(
                                            div().flex().items_center().gap_2().min_w_0()
                                                .child(div().text_sm().font_semibold().text_color(theme.foreground).truncate().child(name))
                                                .child(div().px_1p5().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground).child(vis)),
                                        )
                                        .children(desc.map(|d| div().text_xs().text_color(theme.muted_foreground).truncate().child(d))),
                                ),
                        )
                        .child(
                            div().flex().items_center().gap_3().flex_shrink_0()
                                .child(
                                    div().flex().flex_col().items_end().gap_0p5()
                                        .children(counts_meta)
                                        .children((!created_rel.is_empty()).then(|| {
                                            div().text_xs().text_color(theme.muted_foreground).child(format!("Created {created_rel}"))
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("group-row-more-{gid}")))
                                        .ghost().xsmall().icon(Icon::new(IconName::EllipsisVertical).size_3p5())
                                        .dropdown_menu(move |mut menu, _, _| {
                                            let (p, u) = (copy_path.clone(), web_url.clone());
                                            menu = menu.item(PopupMenuItem::new("Copy full path").on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(p.clone()));
                                            }));
                                            if !u.is_empty() {
                                                menu = menu.item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| {
                                                    let _ = safe_open_url(&u);
                                                }));
                                            }
                                            menu
                                        }),
                                ),
                        )
                        .test_support(),
                )
                .into_any_element()
        }
        GroupTreeRow::Project { project, depth } => {
            let pid = project.id;
            let name = project.name.clone();
            let branch = project.default_branch.clone().unwrap_or_else(|| "main".into());
            let updated_rel = project.last_activity_at.as_deref().map(format_relative).unwrap_or_default();
            let av = render_entity_avatar(project.avatar_url.as_deref(), &name, px(32.), client, cx);
            let theme = cx.theme();
            let indent = px((*depth as f32) * 24.0);
            let clicked = project.clone();

            div()
                .w_full().h(px(66.)).px_6().flex().flex_col().items_center()
                .child(
                    div()
                        .id(SharedString::from(format!("group-proj-row-{pid}")))
                        .w_full().max_w(px(1120.)).h_full().pl(indent).pr_2()
                        .border_b_1().border_color(theme.border.opacity(0.6))
                        .cursor_pointer().hover(|s| s.bg(theme.list_hover.opacity(0.55)))
                        .flex().items_center().justify_between().gap_4()
                        .on_click(move |_, _, cx| {
                            app_state.update(cx, |s, cx| s.select_project(clicked.clone(), cx));
                        })
                        .child(
                            div().flex_1().min_w_0().flex().items_center().gap_2p5()
                                .child(div().w(px(22.)).flex_shrink_0())
                                .child(Icon::new(IconName::Folder).size_4().text_color(theme.muted_foreground))
                                .child(av)
                                .child(
                                    div().flex_1().min_w_0().flex().flex_col().gap_0p5()
                                        .child(
                                            div().flex().items_center().gap_2().min_w_0()
                                                .child(div().text_sm().font_semibold().text_color(theme.foreground).truncate().child(name))
                                                .child(
                                                    div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary)
                                                        .font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.muted_foreground)
                                                        .child(branch),
                                                ),
                                        )
                                        .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(project.path_with_namespace.clone())),
                                ),
                        )
                        .child(
                            div().flex().flex_col().items_end().gap_0p5().flex_shrink_0().text_xs().text_color(theme.muted_foreground)
                                .child(format!("★ {}", project.star_count))
                                .children((!updated_rel.is_empty()).then(|| div().child(format!("Updated {updated_rel}")))),
                        )
                        .test_support(),
                )
                .into_any_element()
        }
    }
}
