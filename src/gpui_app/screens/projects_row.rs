use gpui_kit::{
    base::StyledExt as _,
    component::{ActiveTheme as _, Icon, IconName},
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{GitLab, Project};

use super::super::avatar::render_entity_avatar;
use super::super::state::AppState;

pub fn render_project_row(
    project: Project,
    is_active: bool,
    client: Option<&GitLab>,
    state: Entity<AppState>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let av = render_entity_avatar(project.avatar_url.as_deref(), &project.name, px(36.), client, cx);
    let theme = cx.theme();
    let branch = project.default_branch.clone().unwrap_or_else(|| "main".into());
    let updated = project.last_activity_at.as_deref().map(format_relative).unwrap_or_default();
    let (ns_prefix, leaf_name) = match project.path_with_namespace.rsplit_once('/') {
        Some((ns, _)) => (Some(ns.to_string()), project.name.clone()),
        None => (None, project.name.clone()),
    };
    let clicked = project.clone();
    let ns_click_state = state.clone();

    div()
        .w_full().h(px(66.)).px_6().flex().flex_col().items_center()
        .child(
            div()
                .id(SharedString::from(format!("project-row-{}", project.id)))
                .w_full().max_w(px(1200.)).h_full().px_3()
                .flex().items_center().justify_between().gap_4()
                .cursor_pointer().border_b_1().border_color(theme.border.opacity(0.6))
                .bg(if is_active { theme.list_active } else { gpui_kit::transparent_black() })
                .hover(|s| s.bg(theme.list_hover.opacity(0.5)))
                .on_click(move |_, _, cx| state.update(cx, |s, cx| s.select_project(clicked.clone(), cx)))
                .child(
                    div().flex().items_center().gap_3().min_w_0().flex_1()
                        .child(av)
                        .child(
                            div().flex().flex_col().gap_0p5().min_w_0()
                                .child(
                                    div().flex().items_center().gap_1().min_w_0()
                                        .children(ns_prefix.map(|ns| {
                                            let ns_target = ns.clone();
                                            div().id(SharedString::from(format!("proj-ns-{}", project.id)))
                                                .cursor_pointer().text_xs().text_color(theme.muted_foreground)
                                                .hover(|s| s.text_color(theme.link))
                                                .on_click(move |_, _, cx| {
                                                    cx.stop_propagation();
                                                    ns_click_state.update(cx, |s, cx| s.open_group(Some(ns_target.clone()), cx));
                                                })
                                                .child(format!("{ns} /"))
                                                .test_support()
                                        }))
                                        .child(div().text_sm().font_semibold().text_color(theme.foreground).truncate().child(leaf_name)),
                                )
                                .child(
                                    div().flex().items_center().gap_2().text_xs().text_color(theme.muted_foreground)
                                        .child(
                                            div().px_1p5().py(px(0.5)).rounded_sm().bg(theme.secondary)
                                                .font_family(theme.mono_font_family.clone()).text_color(theme.foreground)
                                                .child(branch),
                                        ),
                                ),
                        ),
                )
                .child(
                    div().flex().items_center().gap_3().flex_shrink_0().text_xs().text_color(theme.muted_foreground)
                        .child(
                            div().flex().items_center().gap_1()
                                .child(Icon::new(IconName::Star).size_3().text_color(theme.muted_foreground))
                                .child(project.star_count.to_string()),
                        )
                        .children((!updated.is_empty()).then(|| div().child(format!("updated {updated}")))),
                )
                .test_support(),
        )
}
