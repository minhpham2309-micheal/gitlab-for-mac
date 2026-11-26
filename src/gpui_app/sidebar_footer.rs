use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        sidebar::SidebarToggleButton,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use super::pajamas_theme::apply_theme_from_prefs;
use super::state::AppState;

pub fn render_sidebar_footer(
    state: &Entity<AppState>,
    collapsed: bool,
    cx: &mut App,
) -> impl IntoElement {
    let dark = cx.theme().is_dark();
    let (accent, accent_fg, border, muted_fg) = (
        cx.theme().sidebar_accent,
        cx.theme().sidebar_accent_foreground,
        cx.theme().sidebar_border,
        cx.theme().muted_foreground,
    );
    let toggle_st = state.clone();
    let theme_st = state.clone();
    let theme_btn = Button::new("toggle-theme")
        .ghost()
        .small()
        .accessibility_label(if dark { "Switch to light theme" } else { "Switch to dark theme" })
        .icon(Icon::new(if dark { IconName::Sun } else { IconName::Moon }).size_3p5())
        .on_click(move |_, _, cx| {
            let prefs = theme_st.update(cx, |s, cx| s.toggle_theme(cx));
            apply_theme_from_prefs(&prefs, cx);
        });

    let base = div()
        .id("sidebar-footer-controls")
        .w_full()
        .pt_2()
        .border_t_1()
        .border_color(border.opacity(0.6))
        .flex()
        .items_center();

    if collapsed {
        return base
            .flex_col()
            .gap_1p5()
            .child(theme_btn)
            .child(
                SidebarToggleButton::new()
                    .collapsed(true)
                    .accessibility_label("Expand sidebar")
                    .on_click(move |_, _, cx| toggle_st.update(cx, |s, cx| s.toggle_sidebar(cx))),
            )
            .test_support()
            .into_any_element();
    }

    base.justify_between()
        .gap_2()
        .child(
            div()
                .id("sidebar-collapse-button")
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(muted_fg)
                .hover(move |s| s.bg(accent).text_color(accent_fg))
                .on_click(move |_, _, cx| toggle_st.update(cx, |s, cx| s.toggle_sidebar(cx)))
                .child(Icon::new(IconName::PanelLeftClose).size_4())
                .child(div().text_xs().child("Collapse sidebar"))
                .test_support(),
        )
        .child(theme_btn)
        .test_support()
        .into_any_element()
}
