use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        ActiveTheme as _, Selectable as _, Sizable as _,
    },
    div, px, Context, IntoElement, ParentElement as _, SharedString, Styled as _,
};

use crate::prefs::{
    Prefs, DEFAULT_MONO_FONT, DEFAULT_UI_FONT, MAX_MONO_FONT_SIZE, MAX_UI_FONT_SIZE,
    MIN_MONO_FONT_SIZE, MIN_UI_FONT_SIZE,
};

use super::preferences::PreferencesView;
use super::preferences_cards::{card_shell, setting_row};

const UI_PRESETS: &[u16] = &[13, 14, 15, 16, 18, 20];
const MONO_PRESETS: &[u16] = &[11, 12, 13, 14, 16, 18];

pub fn render_typography_card(
    view: &PreferencesView,
    prefs: &Prefs,
    cx: &mut Context<PreferencesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let ui_sz = prefs.ui_font_size();
    let ui_fam = prefs.ui_font_family().to_string();
    let mono_sz = prefs.mono_font_size();
    let mono_fam = prefs.mono_font_family().to_string();
    let mono = theme.mono_font_family.clone();
    let fg = theme.foreground;

    card_shell("Typography & Scaling", cx)
        .child(setting_row(
            "UI font size",
            "Scales all text & spacing (⌘+ / ⌘- / ⌘0)",
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(Button::new("pref-ui-dec").outline().xsmall().label("−").on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.ui_font_size = Some(ui_sz.saturating_sub(1).max(MIN_UI_FONT_SIZE)), cx))))
                .child(div().w(px(46.)).text_center().text_xs().font_family(mono.clone()).text_color(fg).child(format!("{ui_sz}px")))
                .child(Button::new("pref-ui-inc").outline().xsmall().label("+").on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.ui_font_size = Some((ui_sz + 1).min(MAX_UI_FONT_SIZE)), cx))))
                .children(UI_PRESETS.iter().map(|&sz| {
                    Button::new(SharedString::from(format!("pref-uisz-{sz}"))).ghost().xsmall().selected(ui_sz == sz).label(format!("{sz}"))
                        .on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.ui_font_size = Some(sz), cx)))
                })),
            cx,
        ))
        .child(setting_row(
            "UI font family",
            "Interface typeface (presets + custom macOS font)",
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(div().flex().flex_wrap().gap_1().children(view.ui_fonts.iter().enumerate().map(|(ix, fam)| {
                    let is_active = ui_fam == *fam;
                    let label = if fam == DEFAULT_UI_FONT { "System" } else { fam.as_str() }.to_string();
                    let chosen = fam.clone();
                    Button::new(SharedString::from(format!("pref-uif-{ix}"))).outline().xsmall().selected(is_active).label(label)
                        .on_click(cx.listener(move |v, _, w, cx| v.set_ui_font(chosen.clone(), w, cx)))
                })))
                .child(
                    div().flex().items_center().gap_1p5()
                        .child(div().w(px(220.)).child(Input::new(&view.ui_font_input)))
                        .child(Button::new("pref-uif-apply").outline().xsmall().label("Apply").on_click(cx.listener(|v, _, _, cx| v.apply_custom_ui_font(cx)))),
                ),
            cx,
        ))
        .child(setting_row(
            "Code & log font size",
            "Monospace size for MR Diff, Job Logs & Console",
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(Button::new("pref-mono-dec").outline().xsmall().label("−").on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.mono_font_size = Some(mono_sz.saturating_sub(1).max(MIN_MONO_FONT_SIZE)), cx))))
                .child(div().w(px(46.)).text_center().text_xs().font_family(mono).text_color(fg).child(format!("{mono_sz}px")))
                .child(Button::new("pref-mono-inc").outline().xsmall().label("+").on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.mono_font_size = Some((mono_sz + 1).min(MAX_MONO_FONT_SIZE)), cx))))
                .children(MONO_PRESETS.iter().map(|&sz| {
                    Button::new(SharedString::from(format!("pref-msz-{sz}"))).ghost().xsmall().selected(mono_sz == sz).label(format!("{sz}"))
                        .on_click(cx.listener(move |v, _, _, cx| v.apply(|p| p.mono_font_size = Some(sz), cx)))
                })),
            cx,
        ))
        .child(setting_row(
            "Code & log font family",
            "Monospace typeface (bundled JetBrains Mono + system)",
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(div().flex().flex_wrap().gap_1().children(view.mono_fonts.iter().enumerate().map(|(ix, fam)| {
                    let is_active = mono_fam == *fam;
                    let label = if fam == DEFAULT_MONO_FONT { "JetBrains Mono" } else { fam.as_str() }.to_string();
                    let chosen = fam.clone();
                    Button::new(SharedString::from(format!("pref-mf-{ix}"))).outline().xsmall().selected(is_active).label(label)
                        .on_click(cx.listener(move |v, _, w, cx| v.set_mono_font(chosen.clone(), w, cx)))
                })))
                .child(
                    div().flex().items_center().gap_1p5()
                        .child(div().w(px(220.)).child(Input::new(&view.mono_font_input)))
                        .child(Button::new("pref-mf-apply").outline().xsmall().label("Apply").on_click(cx.listener(|v, _, _, cx| v.apply_custom_mono_font(cx)))),
                ),
            cx,
        ))
}
