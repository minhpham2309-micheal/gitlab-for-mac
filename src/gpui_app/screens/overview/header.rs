use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::{GitLab, Project, ProjectDetail};

use super::super::super::avatar::render_entity_avatar;
use super::super::super::safe_url::safe_open_url;
use super::ProjectOverviewView;

fn segmented_counter_button(
    id: &'static str,
    icon: Icon,
    label: &'static str,
    count: u64,
    theme: &gpui_kit::component::Theme,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .h(px(30.))
        .rounded_md()
        .border_1()
        .border_color(theme.input)
        .bg(theme.secondary)
        .hover(|s| s.bg(theme.secondary_hover))
        .cursor_pointer()
        .flex()
        .items_center()
        .overflow_hidden()
        .text_xs()
        .text_color(theme.foreground)
        .child(
            div()
                .px_2p5()
                .h_full()
                .flex()
                .items_center()
                .gap_1p5()
                .child(icon.size_3p5().text_color(theme.muted_foreground))
                .child(label),
        )
        .child(
            div()
                .px_2p5()
                .h_full()
                .border_l_1()
                .border_color(theme.border)
                .bg(theme.background.opacity(0.45))
                .flex()
                .items_center()
                .text_color(theme.foreground)
                .child(count.to_string()),
        )
}

pub fn render_project_header(
    project: &Project,
    detail: Option<&ProjectDetail>,
    starred: bool,
    client: Option<&GitLab>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement + use<> {
    let name = detail
        .map(|d| d.name.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&project.name)
        .to_string();
    let av_url = detail.and_then(|d| d.avatar_url.as_deref()).or(project.avatar_url.as_deref());
    let av = render_entity_avatar(av_url, &name, px(44.), client, cx);
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

    let star_btn = segmented_counter_button(
        "ov-star",
        Icon::new(if starred { IconName::StarFill } else { IconName::Star }),
        "Star",
        stars,
        theme,
    )
    .on_click(cx.listener(|this, _, _, cx| this.toggle_star(cx)));

    let fork_btn = segmented_counter_button(
        "ov-fork",
        Icon::empty().path("icons/glab-branch.svg"),
        "Fork",
        forks,
        theme,
    )
    .on_click(move |_, _, _| {
        let url = format!("{}/-/forks/new", web_url.trim_end_matches('/'));
        let _ = safe_open_url(&url);
    });

    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .pb_4()
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
                        .child(div().text_2xl().font_bold().text_color(theme.foreground).truncate().child(name))
                        .child(
                            div()
                                .px_1p5()
                                .py(px(1.))
                                .rounded_full()
                                .bg(theme.secondary)
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
                .gap_2()
                .flex_shrink_0()
                .child(
                    Button::new("ov-bell")
                        .outline()
                        .small()
                        .icon(Icon::new(IconName::Bell).size_3p5())
                        .on_click(move |_, _, _| { let _ = safe_open_url(&web_bell); }),
                )
                .child(star_btn)
                .child(fork_btn)
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
