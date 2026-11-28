use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, ClipboardItem, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::gitlab::{GitLab, GroupSortField, GroupTab, SortDir};

use super::super::super::avatar::render_entity_avatar;
use super::super::super::safe_url::safe_open_url;
use super::GroupsView;

pub fn render_groups_top_header(
    view: &GroupsView,
    client: Option<&GitLab>,
    cx: &mut Context<GroupsView>,
) -> impl IntoElement + use<> {
    let Some(group) = view.active_group.as_ref() else {
        let theme = cx.theme();
        return div()
            .id("groups-root-header")
            .w_full().pb_4().flex().items_center().justify_between().gap_4()
            .child(
                div().flex().flex_col().gap_0p5()
                    .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Groups"))
                    .child(div().text_xs().text_color(theme.muted_foreground).child("Browse top-level groups, subgroups, and nested projects")),
            )
            .child(
                Button::new("groups-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5()).label("Reload")
                    .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
            )
            .test_support()
            .into_any_element();
    };

    let name = group.display_name().to_string();
    let av = render_entity_avatar(group.avatar_url.as_deref(), &name, px(48.), client, cx);
    let vis = group.visibility_label();
    let desc = group.description.clone().filter(|d| !d.trim().is_empty());
    let web_url = group.web_url.clone();
    let web_bell = web_url.clone();
    let copy_path = group.full_path.clone();
    let copy_id = group.id.to_string();
    let theme = cx.theme();

    div()
        .id("group-detail-header")
        .w_full().pb_4().flex().items_center().justify_between().gap_4()
        .child(
            div().flex().items_center().gap_3().min_w_0()
                .child(av)
                .child(
                    div().flex().flex_col().gap_0p5().min_w_0()
                        .child(
                            div().flex().items_center().gap_2().min_w_0()
                                .child(div().text_2xl().font_bold().text_color(theme.foreground).truncate().child(name))
                                .child(
                                    div().px_1p5().py(px(1.)).rounded_full().bg(theme.secondary)
                                        .border_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground)
                                        .child(vis),
                                ),
                        )
                        .children(desc.map(|d| div().text_xs().text_color(theme.muted_foreground).truncate().child(d))),
                ),
        )
        .child(
            div().flex().items_center().gap_1p5().flex_shrink_0()
                .children((!web_bell.is_empty()).then(|| {
                    Button::new("group-bell").outline().small().icon(Icon::new(IconName::Bell).size_3p5())
                        .on_click(move |_, _, _| { let _ = safe_open_url(&web_bell); })
                }))
                .child(
                    Button::new("group-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                )
                .child(
                    Button::new("group-more").ghost().small().icon(Icon::new(IconName::EllipsisVertical).size_3p5())
                        .dropdown_menu(move |mut menu, _, _| {
                            let (p, id, url) = (copy_path.clone(), copy_id.clone(), web_url.clone());
                            menu = menu
                                .item(PopupMenuItem::new(format!("Copy group path ({p})")).on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(p.clone()));
                                }))
                                .item(PopupMenuItem::new(format!("Copy group ID ({id})")).on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(id.clone()));
                                }));
                            if !url.is_empty() {
                                menu = menu.item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| {
                                    let _ = safe_open_url(&url);
                                }));
                            }
                            menu
                        }),
                ),
        )
        .test_support()
        .into_any_element()
}

pub fn render_groups_tabs_and_toolbar(
    view: &GroupsView,
    cx: &mut Context<GroupsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let in_group = view.active_group.is_some();
    let active_tab_border = theme.primary;
    let sort_field = view.sort_field;
    let sort_icon = match view.sort_dir { SortDir::Desc => IconName::SortDescending, SortDir::Asc => IconName::SortAscending };
    let weak_sort = cx.entity().downgrade();

    let tabs_bar = in_group.then(|| {
        div()
            .w_full().border_b_1().border_color(theme.border).flex().items_end().gap_1()
            .children(GroupTab::ALL.into_iter().map(|tab| {
                let active = view.tab == tab;
                div()
                    .id(SharedString::from(format!("group-tab-{}", tab.label())))
                    .px_3().py_2p5().cursor_pointer()
                    .border_b_2().border_color(if active { active_tab_border } else { gpui_kit::transparent_black() })
                    .text_sm()
                    .text_color(if active { theme.foreground } else { theme.muted_foreground })
                    .hover(|s| s.text_color(theme.foreground))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx)))
                    .child(div().when(active, |d| d.font_semibold()).child(tab.label()))
                    .test_support()
            }))
    });

    let toolbar = div()
        .id("groups-filter-bar")
        .mt_3().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border)
        .flex().items_center().gap_2()
        .child(
            div().flex_1().min_w_0().h(px(32.)).rounded_md().bg(theme.background)
                .border_1().border_color(theme.input).flex().items_center()
                .child(div().flex_1().min_w_0().px_1p5().child(Input::new(&view.search_input).appearance(false).cleanable(true).small()))
                .child(
                    div().border_l_1().border_color(theme.border).px_0p5().child(
                        Button::new("groups-search-submit").ghost().xsmall().icon(Icon::new(IconName::Search).size_3p5())
                            .on_click(cx.listener(|this, _, _, cx| this.submit_search(cx))),
                    ),
                ),
        )
        .child(
            div().flex().items_center().flex_shrink_0()
                .child(
                    Button::new("groups-sort-field").outline().small().label(format!("{} ▾", sort_field.label()))
                        .dropdown_menu(move |mut menu, _, _| {
                            for f in GroupSortField::ALL {
                                let w = weak_sort.clone();
                                menu = menu.item(PopupMenuItem::new(f.label()).checked(sort_field == f).on_click(move |_, _, cx| {
                                    let _ = w.update(cx, |v, cx| v.set_sort_field(f, cx));
                                }));
                            }
                            menu
                        }),
                )
                .child(
                    Button::new("groups-sort-dir").outline().small().icon(Icon::new(sort_icon).size_3p5())
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sort_dir(cx))),
                ),
        )
        .test_support();

    div().w_full().flex().flex_col().children(tabs_bar).child(toolbar)
}
