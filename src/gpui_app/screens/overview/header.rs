use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, Entity, IntoElement, ParentElement as _, SharedString,
    Styled as _,
};

use crate::gitlab::{GitLab, Project, ProjectDetail};
use crate::nav::Screen;

use super::super::super::avatar::render_avatar;
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::AppState;
use super::ProjectOverviewView;

pub fn render_namespace_bar(
    project: &Project,
    app_state: &Entity<AppState>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let segments: Vec<&str> = project.path_with_namespace.split('/').collect();
    let last_ix = segments.len().saturating_sub(1);
    let mut row = div()
        .w_full()
        .px_6()
        .py_2p5()
        .border_b_1()
        .border_color(theme.border.opacity(0.6))
        .flex()
        .items_center()
        .flex_wrap()
        .gap_1p5()
        .text_xs();

    for (ix, seg) in segments.into_iter().enumerate() {
        if ix > 0 {
            row = row.child(div().text_color(theme.muted_foreground).child("/"));
        }
        let is_last = ix == last_ix;
        let label = if is_last { project.name.clone() } else { seg.to_string() };
        if is_last {
            row = row.child(div().text_color(theme.foreground).child(label));
        } else {
            let state = app_state.clone();
            row = row.child(
                Button::new(SharedString::from(format!("ns-seg-{ix}")))
                    .ghost()
                    .xsmall()
                    .label(label)
                    .on_click(move |_, _, cx| state.update(cx, |s, cx| s.set_screen(Screen::Projects, cx))),
            );
        }
    }
    row
}

pub fn render_project_header(
    project: &Project,
    detail: Option<&ProjectDetail>,
    starred: bool,
    client: Option<&GitLab>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement + use<> {
    let name = detail.map(|d| d.name.as_str()).filter(|s| !s.is_empty()).unwrap_or(&project.name).to_string();
    let av_url = detail.and_then(|d| d.avatar_url.as_deref()).or(project.avatar_url.as_deref());
    let av = render_avatar(av_url, &name, px(40.), client, cx);
    let theme = cx.theme();
    let stars = detail.map(|d| d.star_count).unwrap_or(project.star_count);
    let forks = detail.map(|d| d.forks_count).unwrap_or(0);
    let visibility = detail.and_then(|d| d.visibility.as_deref()).unwrap_or("private");
    let vis_badge = match visibility {
        "public" => "Public",
        "internal" => "Internal",
        _ => "Private",
    };

    let web_url = project.web_url.clone();
    let web_bell = format!("{}/-/settings/integrations", web_url.trim_end_matches('/'));
    let web_open = web_url.clone();
    let project_id_str = project.id.to_string();
    let copy_web = web_url.clone();

    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .pb_3()
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
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(div().text_xl().text_color(theme.foreground).truncate().child(name))
                        .child(
                            div()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_sm()
                                .border_1()
                                .border_color(theme.border)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(vis_badge),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .flex_shrink_0()
                .child(
                    Button::new("ov-bell")
                        .outline()
                        .small()
                        .icon(Icon::new(IconName::Bell).size_3p5())
                        .on_click(move |_, _, _| { let _ = safe_open_url(&web_bell); }),
                )
                .child(
                    Button::new("ov-star")
                        .outline()
                        .small()
                        .icon(Icon::new(if starred { IconName::StarFill } else { IconName::Star }).size_3p5())
                        .label(format!("Star  {stars}"))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_star(cx))),
                )
                .child(
                    Button::new("ov-fork")
                        .outline()
                        .small()
                        .icon(Icon::empty().path("icons/glab-branch.svg").size_3p5())
                        .label(format!("Fork  {forks}"))
                        .on_click(move |_, _, _| {
                            let url = format!("{}/-/forks/new", web_url.trim_end_matches('/'));
                            let _ = safe_open_url(&url);
                        }),
                )
                .child(
                    Button::new("ov-more")
                        .ghost()
                        .small()
                        .icon(Icon::new(IconName::EllipsisVertical).size_3p5())
                        .dropdown_menu(move |menu, _, _| {
                            let id_copy = project_id_str.clone();
                            let url_copy = copy_web.clone();
                            let url_open = web_open.clone();
                            menu.item(PopupMenuItem::new(format!("Copy project ID ({id_copy})")).on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(id_copy.clone()));
                            }))
                            .item(PopupMenuItem::new("Copy project URL").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(url_copy.clone()));
                            }))
                            .item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| {
                                let _ = safe_open_url(&url_open);
                            }))
                        }),
                ),
        )
}
