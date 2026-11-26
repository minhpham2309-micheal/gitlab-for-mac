use gpui_kit::{
    component::ActiveTheme as _, div, px, App, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::{GitLab, Project};

use super::super::avatar::render_avatar;
use super::super::state::AppState;

pub fn render_project_row(
    project: Project,
    is_active: bool,
    client: Option<&GitLab>,
    state: Entity<AppState>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let av = render_avatar(
        project.avatar_url.as_deref(),
        &project.name,
        px(32.),
        client,
        cx,
    );
    let theme = cx.theme();
    let branch = project
        .default_branch
        .clone()
        .unwrap_or_else(|| "main".into());
    let activity = project
        .last_activity_at
        .as_deref()
        .and_then(|s| s.get(..10))
        .unwrap_or("")
        .to_string();
    let clicked = project.clone();

    div()
        .id(SharedString::from(format!("project-row-{}", project.id)))
        .h(px(56.))
        .px_4()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .cursor_pointer()
        .border_b_1()
        .border_color(theme.border.opacity(0.5))
        .bg(if is_active {
            theme.list_active
        } else {
            theme.colors.list
        })
        .hover(|s| s.bg(theme.list_hover))
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| s.select_project(clicked.clone(), cx));
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .min_w_0()
                .child(av)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .truncate()
                                .child(project.path_with_namespace),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("★ {} · {branch}", project.star_count)),
                        ),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .flex_shrink_0()
                .child(activity),
        )
}
