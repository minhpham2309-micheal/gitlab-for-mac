use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Selectable as _, Sizable as _,
    },
    div, px, rgb, Context, IntoElement, ParentElement as _, SharedString, Styled as _,
};

use crate::prefs::Prefs;

use super::super::pajamas_theme::status_color;
use super::preferences::PreferencesView;

const RADIUS_PRESETS: &[(u16, &str)] = &[(0, "Square"), (4, "Compact"), (6, "Default"), (10, "Rounded")];

pub fn render_appearance_card(prefs: &Prefs, cx: &mut Context<PreferencesView>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = prefs.is_dark();
    let radius = prefs.radius();
    let sw = prefs.sidebar_width();
    let collapsed = prefs.sidebar_collapsed;

    card_shell("Appearance & Layout", cx)
        .child(setting_row(
            "Theme",
            "GitLab Pajamas color palette (⇧⌘T)",
            div()
                .flex()
                .gap_1p5()
                .child(
                    Button::new("pref-theme-dark").outline().small().selected(dark).label("Dark")
                        .on_click(cx.listener(|v, _, _, cx| v.apply(|p| p.theme_mode = Some("dark".into()), cx))),
                )
                .child(
                    Button::new("pref-theme-light").outline().small().selected(!dark).label("Light")
                        .on_click(cx.listener(|v, _, _, cx| v.apply(|p| p.theme_mode = Some("light".into()), cx))),
                ),
            cx,
        ))
        .child(setting_row(
            "Corner radius",
            "Roundness of cards, inputs, and buttons",
            div().flex().flex_wrap().gap_1().children(RADIUS_PRESETS.iter().map(|&(r, label)| {
                Button::new(SharedString::from(format!("pref-rad-{r}")))
                    .outline().xsmall().selected(radius == r).label(format!("{label} ({r}px)"))
                    .on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.radius = Some(r), cx)))
            })),
            cx,
        ))
        .child(setting_row(
            "Sidebar width",
            "Expanded Super Sidebar width (200–340px)",
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .child(
                    Button::new("pref-sw-dec").outline().xsmall().label("−")
                        .on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.sidebar_width = Some(sw.saturating_sub(16).max(200)), cx))),
                )
                .child(div().w(px(56.)).text_center().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.foreground).child(format!("{sw}px")))
                .child(
                    Button::new("pref-sw-inc").outline().xsmall().label("+")
                        .on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.sidebar_width = Some((sw + 16).min(340)), cx))),
                )
                .child(
                    Button::new("pref-sw-col").ghost().xsmall().selected(collapsed).label(if collapsed { "Collapsed" } else { "Expanded" })
                        .on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.sidebar_collapsed = !collapsed, cx))),
                ),
            cx,
        ))
}

pub fn render_viewer_and_preview_cards(prefs: &Prefs, cx: &mut Context<PreferencesView>) -> impl IntoElement {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let split = prefs.default_diff_split;
    let wrap = prefs.default_log_wrap;

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            card_shell("Editor & Viewer Defaults", cx)
                .child(setting_row(
                    "Default MR diff layout",
                    "Initial display mode on Merge Request Changes",
                    div()
                        .flex()
                        .gap_1p5()
                        .child(Button::new("pref-diff-uni").outline().small().selected(!split).label("Unified").on_click(cx.listener(|v, _, _, cx| v.apply(|p| p.default_diff_split = false, cx))))
                        .child(Button::new("pref-diff-spl").outline().small().selected(split).label("Side-by-side").on_click(cx.listener(|v, _, _, cx| v.apply(|p| p.default_diff_split = true, cx)))),
                    cx,
                ))
                .child(setting_row(
                    "Job log long lines",
                    "Default overflow behavior in virtualized CI logs",
                    div()
                        .flex()
                        .gap_1p5()
                        .child(Button::new("pref-log-hscroll").outline().small().selected(!wrap).label("Horizontal scroll").on_click(cx.listener(|v, _, _, cx| v.apply(|p| p.default_log_wrap = false, cx))))
                        .child(Button::new("pref-log-compact").outline().small().selected(wrap).label("Compact truncate").on_click(cx.listener(|v, _, _, cx| v.apply(|p| p.default_log_wrap = true, cx)))),
                    cx,
                )),
        )
        .child(
            card_shell("Live Preview", cx)
                .child(
                    div().flex().items_center().justify_between().gap_2()
                        .child(div().text_sm().text_color(theme.foreground).child("!142 feat: upgrade GPUI native GitLab client"))
                        .child(div().px_2().py(px(1.)).rounded_full().bg(status_color("success", dark).opacity(0.2)).text_color(status_color("success", dark)).text_xs().child("passed")),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).child(format!("UI: {} {}px · Code: {} {}px", prefs.ui_font_family(), prefs.ui_font_size(), prefs.mono_font_family(), prefs.mono_font_size())))
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0x121117))
                        .font_family(theme.mono_font_family.clone())
                        .text_size(theme.mono_font_size)
                        .text_color(rgb(0x52B87A))
                        .child("$ cargo test --locked  # 62 passed; 0 failed"),
                ),
        )
}

pub(crate) fn card_shell(title: &'static str, cx: &Context<PreferencesView>) -> gpui_kit::Div {
    let theme = cx.theme();
    div()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.sidebar)
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_sm().text_color(theme.foreground).child(title))
}

pub(crate) fn setting_row(
    label: &'static str,
    hint: &'static str,
    control: impl IntoElement,
    cx: &Context<PreferencesView>,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .pt_2()
        .border_t_1()
        .border_color(theme.border.opacity(0.45))
        .flex()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(div().text_sm().text_color(theme.foreground).child(label))
                .child(div().text_xs().text_color(theme.muted_foreground).child(hint)),
        )
        .child(control)
}
