use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, ClipboardItem, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::gitlab::{GitLab, MrFilter, MrSortField, Project, SortDir};

use super::super::super::safe_url::safe_open_url;
use super::list_row::render_mr_list_row;
use super::MergeRequestsView;

pub fn render_mr_list_view(
    view: &MergeRequestsView,
    project: &Project,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let rows: Vec<_> = view.mrs.iter().map(|mr| render_mr_list_row(mr, client, cx)).collect();
    let theme = cx.theme();
    let dark = theme.is_dark();
    let web_base = project.web_url.trim_end_matches('/').to_string();
    let new_mr_url = format!("{web_base}/-/merge_requests/new");
    let bulk_url = format!("{web_base}/-/merge_requests?state={}", view.filter.api_state());
    let list_url = format!("{web_base}/-/merge_requests");
    let copy_url = list_url.clone();
    let active_tab_border = if dark { rgb(0x63A6E9).into() } else { theme.primary };
    let badge_bg = if dark { rgb(0xBFBFBF).into() } else { theme.secondary };
    let badge_fg = if dark { rgb(0x18171D).into() } else { theme.foreground };
    let primary_cta_bg = if dark { rgb(0x63A6E9).into() } else { theme.primary };
    let primary_cta_fg = if dark { rgb(0x18171D).into() } else { theme.primary_foreground };

    let tabs = div().flex().items_center().gap_1().children(MrFilter::ALL.into_iter().map(|f| {
        let active = view.filter == f;
        let count = view.counts.for_filter(f).or_else(|| active.then_some(view.mrs.len() as u32));
        div()
            .id(SharedString::from(format!("mr-tab-{}", f.api_state())))
            .px_3().py_2p5().cursor_pointer()
            .border_b_2().border_color(if active { active_tab_border } else { gpui_kit::transparent_black() })
            .flex().items_center().gap_1p5()
            .text_sm()
            .text_color(if active { theme.foreground } else { theme.muted_foreground })
            .hover(|s| s.text_color(theme.foreground))
            .on_click(cx.listener(move |this, _, _, cx| this.set_filter(f, cx)))
            .child(div().when_some(active.then_some(()), |d, _| d.font_semibold()).child(f.label()))
            .children(count.map(|n| {
                div().px_1p5().py(px(1.)).rounded_full().bg(badge_bg).text_color(badge_fg).text_xs().font_semibold()
                    .child(n.to_string())
            }))
            .test_support()
    }));

    let view_weak = cx.entity().downgrade();
    let actions = div()
        .flex().items_center().gap_2().pb_1p5()
        .child(
            div().id("mr-bulk-edit").px_3().py_1().rounded_md().cursor_pointer()
                .bg(theme.secondary).border_1().border_color(theme.border)
                .hover(|s| s.bg(theme.secondary_hover))
                .text_xs().text_color(theme.foreground)
                .on_click(move |_, _, _| { let _ = safe_open_url(&bulk_url); })
                .child("Bulk edit")
                .test_support(),
        )
        .child(
            div().id("mr-new-btn").px_3().py_1().rounded_md().cursor_pointer()
                .bg(primary_cta_bg).hover(|s| s.opacity(0.9))
                .text_xs().font_semibold().text_color(primary_cta_fg)
                .on_click(move |_, _, _| { let _ = safe_open_url(&new_mr_url); })
                .child("New merge request")
                .test_support(),
        )
        .child(
            Button::new("mr-list-more").ghost().xsmall().icon(Icon::new(IconName::EllipsisVertical).size_3p5())
                .dropdown_menu(move |menu, _, _| {
                    let w = view_weak.clone();
                    let u_open = list_url.clone();
                    let u_copy = copy_url.clone();
                    menu.item(PopupMenuItem::new("Refresh list").on_click(move |_, _, cx| {
                        let _ = w.update(cx, |v, cx| v.load_list(cx));
                    }))
                    .item(PopupMenuItem::new("Open in GitLab Web").on_click(move |_, _, _| { let _ = safe_open_url(&u_open); }))
                    .item(PopupMenuItem::new("Copy list URL").on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(u_copy.clone()));
                    }))
                }),
        );

    let history = view.search_history.clone();
    let weak_hist = cx.entity().downgrade();
    let sort_field = view.sort_field;
    let weak_sort = cx.entity().downgrade();
    let sort_icon = match view.sort_dir {
        SortDir::Desc => IconName::SortDescending,
        SortDir::Asc => IconName::SortAscending,
    };

    let filter_bar = div()
        .id("mr-filter-bar").mt_3().p_2().rounded_md()
        .bg(theme.list_head).border_1().border_color(theme.border)
        .flex().items_center().gap_2()
        .child(
            div().flex_1().min_w_0().h(px(32.)).rounded_md()
                .bg(theme.background).border_1().border_color(theme.input)
                .flex().items_center()
                .child(
                    div().border_r_1().border_color(theme.border).px_0p5().child(
                        Button::new("mr-search-history").ghost().xsmall()
                            .icon(Icon::new(IconName::Undo2).size_3p5()).label("▾")
                            .dropdown_menu(move |mut menu, _, _| {
                                if history.is_empty() {
                                    menu = menu.item(PopupMenuItem::new("No recent searches").disabled(true));
                                } else {
                                    for term in &history {
                                        let t = term.clone();
                                        let w = weak_hist.clone();
                                        menu = menu.item(PopupMenuItem::new(term.clone()).on_click(move |_, window, cx| {
                                            let _ = w.update(cx, |v, cx| v.apply_history_search(t.clone(), window, cx));
                                        }));
                                    }
                                    let w = weak_hist.clone();
                                    menu = menu.separator().item(PopupMenuItem::new("Clear recent searches").on_click(move |_, _, cx| {
                                        let _ = w.update(cx, |v, cx| { v.search_history.clear(); cx.notify(); });
                                    }));
                                }
                                menu
                            }),
                    ),
                )
                .child(div().flex_1().min_w_0().px_1().child(Input::new(&view.search_input).appearance(false).cleanable(true).small()))
                .child(
                    div().border_l_1().border_color(theme.border).px_0p5().child(
                        Button::new("mr-search-submit").ghost().xsmall().icon(Icon::new(IconName::Search).size_3p5())
                            .on_click(cx.listener(|this, _, _, cx| this.submit_search(cx))),
                    ),
                ),
        )
        .child(
            div().flex().items_center().flex_shrink_0().child(
                Button::new("mr-sort-field").outline().small()
                    .label(format!("{} ▾", sort_field.label()))
                    .dropdown_menu(move |mut menu, _, _| {
                        for f in MrSortField::ALL {
                            let w = weak_sort.clone();
                            menu = menu.item(PopupMenuItem::new(f.label()).checked(sort_field == f).on_click(move |_, _, cx| {
                                let _ = w.update(cx, |v, cx| v.set_sort_field(f, cx));
                            }));
                        }
                        menu
                    }),
            )
            .child(
                Button::new("mr-sort-dir").outline().small().icon(Icon::new(sort_icon).size_3p5())
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sort_dir(cx))),
            ),
        )
        .test_support();

    let list_body = if view.list_loading && view.mrs.is_empty() {
        div().py_12().flex().justify_center().text_sm().text_color(theme.muted_foreground)
            .child("Loading merge requests…").into_any_element()
    } else if rows.is_empty() {
        div().py_12().flex().flex_col().items_center().gap_1()
            .text_sm().text_color(theme.foreground).child("No merge requests match your current filter.")
            .into_any_element()
    } else {
        div().w_full().flex().flex_col().children(rows).into_any_element()
    };

    let (page, per_page, has_next) = (view.page, view.per_page, view.next_page.is_some());
    let weak_pp = cx.entity().downgrade();
    let footer = div()
        .py_4().flex().items_center().justify_between()
        .child(
            div().flex().items_center().gap_2()
                .children((page > 1 || has_next).then(|| {
                    div().flex().items_center().gap_1p5()
                        .child(Button::new("mr-prev-page").outline().xsmall().label("Prev").disabled(page <= 1)
                            .on_click(cx.listener(|this, _, _, cx| this.go_page(this.page.saturating_sub(1), cx))))
                        .child(div().text_xs().text_color(theme.muted_foreground).child(format!("Page {page}")))
                        .child(Button::new("mr-next-page").outline().xsmall().label("Next").disabled(!has_next)
                            .on_click(cx.listener(|this, _, _, cx| this.go_page(this.page + 1, cx))))
                })),
        )
        .child(
            Button::new("mr-per-page").outline().small().label(format!("Show {per_page} items ▾"))
                .dropdown_menu(move |mut menu, _, _| {
                    for n in [10usize, 20, 50, 100] {
                        let w = weak_pp.clone();
                        menu = menu.item(PopupMenuItem::new(format!("Show {n} items")).checked(per_page == n)
                            .on_click(move |_, _, cx| { let _ = w.update(cx, |v, cx| v.set_per_page(n, cx)); }));
                    }
                    menu
                }),
        );

    div()
        .id("mr-list-scroll").flex_1().min_h_0().overflow_y_scroll().px_6().py_3()
        .child(
            div().w_full().max_w(px(1120.)).mx_auto().flex().flex_col()
                .child(div().w_full().border_b_1().border_color(theme.border).flex().items_end().justify_between().gap_4().child(tabs).child(actions))
                .child(filter_bar)
                .child(div().mt_2().child(list_body))
                .child(footer),
        )
}
