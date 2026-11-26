use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        spinner::Spinner,
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, TitleBar,
    },
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window,
};

use crate::nav::Screen;

use super::actions::TogglePalette;
use super::pajamas_theme::tanuki_color;
use super::state::AppState;
use super::topbar_right::render_top_bar_right;

pub fn render_top_bar(
    state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let st = state.read(cx);
    let active_host = st
        .active_host
        .as_ref()
        .map(|h| h.hostname.clone())
        .unwrap_or_else(|| "No host".into());
    let hosts: Vec<String> = st.hosts.iter().map(|h| h.hostname.clone()).collect();
    let busy = st.busy;
    let can_back = st.can_go_back();
    let can_fwd = st.can_go_forward();
    let right = render_top_bar_right(state, cx);
    let theme = cx.theme();

    let host_state = state.clone();
    let active_cmp = active_host.clone();
    let host_dropdown = Button::new("topbar-host-select")
        .ghost()
        .xsmall()
        .label(active_host)
        .icon(Icon::new(IconName::Globe).size_3p5())
        .dropdown_menu(move |mut menu, _, _| {
            for hostname in &hosts {
                let h = hostname.clone();
                let entity = host_state.clone();
                let is_active = *hostname == active_cmp;
                menu = menu.item(
                    PopupMenuItem::new(hostname.clone())
                        .checked(is_active)
                        .on_click(move |_, _, cx| entity.update(cx, |s, cx| s.select_host(&h, cx))),
                );
            }
            menu
        });

    let logo_state = state.clone();
    let logo_btn = div()
        .id("topbar-tanuki-home")
        .flex()
        .items_center()
        .justify_center()
        .px_1()
        .py_0p5()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme.secondary_hover))
        .on_click(move |_, _, cx| {
            logo_state.update(cx, |s, cx| {
                let target = if s.project.is_some() { Screen::ProjectOverview } else { Screen::Projects };
                s.set_screen(target, cx);
            });
        })
        .child(Icon::empty().path("icons/glab-tanuki.svg").size_4().text_color(tanuki_color()));

    let back_state = state.clone();
    let back_btn = Button::new("topbar-nav-back")
        .ghost()
        .xsmall()
        .disabled(!can_back)
        .icon(Icon::new(IconName::ArrowLeft).size_3p5())
        .on_click(move |_, _, cx| back_state.update(cx, |s, cx| s.go_back(cx)));

    let fwd_state = state.clone();
    let fwd_btn = Button::new("topbar-nav-forward")
        .ghost()
        .xsmall()
        .disabled(!can_fwd)
        .icon(Icon::new(IconName::ArrowRight).size_3p5())
        .on_click(move |_, _, cx| fwd_state.update(cx, |s, cx| s.go_forward(cx)));

    let search_bar = div()
        .id("topbar-global-search")
        .flex_1()
        .max_w(px(460.))
        .min_w(px(180.))
        .h(px(26.))
        .px_2p5()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .hover(|s| s.border_color(theme.input))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .on_click(|_, window, cx| window.dispatch_action(Box::new(TogglePalette), cx))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(Icon::new(IconName::Search).size_3p5().text_color(theme.muted_foreground))
                .child(div().text_xs().text_color(theme.muted_foreground).truncate().child("Search or go to…")),
        )
        .child(
            div()
                .px_1p5()
                .py(px(1.))
                .rounded_sm()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("/"),
        );

    TitleBar::new().child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .pr_3()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .flex_shrink_0()
                    .child(logo_btn)
                    .child(div().flex().items_center().gap_0p5().child(back_btn).child(fwd_btn))
                    .child(host_dropdown)
                    .children(busy.then(|| Spinner::new().xsmall())),
            )
            .child(search_bar)
            .child(right),
    )
}
