use std::collections::HashSet;

use gpui_kit::{
    component::{
        button::Button,
        input::{InputEvent, InputState},
        ActiveTheme as _, Sizable as _,
    },
    div, px, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
};

use crate::prefs::{Prefs, DEFAULT_MONO_FONT, DEFAULT_UI_FONT};

use super::super::pajamas_theme::apply_theme_from_prefs;
use super::super::state::AppState;
use super::preferences_cards::{render_appearance_card, render_viewer_and_preview_cards};
use super::preferences_typography::render_typography_card;
use super::preferences_updates::render_updates_card;

const UI_FONT_CANDIDATES: &[&str] = &[
    "SF Pro Text",
    "SF Pro Display",
    "Inter",
    "Helvetica Neue",
    "Avenir Next",
    "Roboto",
    "Arial",
];
const MONO_FONT_CANDIDATES: &[&str] = &[
    "SF Mono",
    "Menlo",
    "Monaco",
    "Fira Code",
    "Cascadia Code",
    "MesloLGS NF",
    "Courier New",
];

pub struct PreferencesView {
    pub(crate) state: Entity<AppState>,
    pub(crate) ui_fonts: Vec<String>,
    pub(crate) mono_fonts: Vec<String>,
    pub(crate) ui_font_input: Entity<InputState>,
    pub(crate) mono_font_input: Entity<InputState>,
    _subs: Vec<Subscription>,
}

fn available_presets(default_first: &str, candidates: &[&str], installed: &HashSet<String>) -> Vec<String> {
    let mut out = vec![default_first.to_string()];
    for &name in candidates {
        if installed.contains(name) && !out.iter().any(|existing| existing == name) {
            out.push(name.to_string());
        }
    }
    out
}

impl PreferencesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let installed: HashSet<String> = cx.text_system().all_font_names().into_iter().collect();
        let ui_fonts = available_presets(DEFAULT_UI_FONT, UI_FONT_CANDIDATES, &installed);
        let mono_fonts = available_presets(DEFAULT_MONO_FONT, MONO_FONT_CANDIDATES, &installed);
        let cur_ui = state.read(cx).prefs.ui_font_family.clone().unwrap_or_default();
        let cur_mono = state.read(cx).prefs.mono_font_family.clone().unwrap_or_default();

        let ui_font_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Custom macOS UI font…");
            input.set_value(cur_ui, window, cx);
            input
        });
        let mono_font_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Custom monospace font…");
            input.set_value(cur_mono, window, cx);
            input
        });
        let sub_ui = cx.subscribe_in(&ui_font_input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.apply_custom_ui_font(cx);
            }
        });
        let sub_mono = cx.subscribe_in(&mono_font_input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.apply_custom_mono_font(cx);
            }
        });
        let obs = cx.observe(&state, |_, _, cx| cx.notify());

        Self {
            state,
            ui_fonts,
            mono_fonts,
            ui_font_input,
            mono_font_input,
            _subs: vec![sub_ui, sub_mono, obs],
        }
    }

    pub(crate) fn apply(&mut self, mutate: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        let next = self.state.update(cx, |s, cx| s.update_prefs(mutate, cx));
        apply_theme_from_prefs(&next, cx);
        cx.notify();
    }

    pub(crate) fn set_ui_font(&mut self, family: String, window: &mut Window, cx: &mut Context<Self>) {
        let stored = if family == DEFAULT_UI_FONT { None } else { Some(family) };
        let val = stored.clone().unwrap_or_default();
        self.ui_font_input.update(cx, |i, cx| i.set_value(val, window, cx));
        self.apply(|p| p.ui_font_family = stored, cx);
    }

    pub(crate) fn apply_custom_ui_font(&mut self, cx: &mut Context<Self>) {
        let raw = self.ui_font_input.read(cx).value().trim().to_string();
        let stored = (!raw.is_empty()).then_some(raw);
        self.apply(|p| p.ui_font_family = stored, cx);
    }

    pub(crate) fn set_mono_font(&mut self, family: String, window: &mut Window, cx: &mut Context<Self>) {
        let stored = if family == DEFAULT_MONO_FONT { None } else { Some(family) };
        let val = stored.clone().unwrap_or_default();
        self.mono_font_input.update(cx, |i, cx| i.set_value(val, window, cx));
        self.apply(|p| p.mono_font_family = stored, cx);
    }

    pub(crate) fn apply_custom_mono_font(&mut self, cx: &mut Context<Self>) {
        let raw = self.mono_font_input.read(cx).value().trim().to_string();
        let stored = (!raw.is_empty()).then_some(raw);
        self.apply(|p| p.mono_font_family = stored, cx);
    }

    fn reset_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui_font_input.update(cx, |i, cx| i.set_value("", window, cx));
        self.mono_font_input.update(cx, |i, cx| i.set_value("", window, cx));
        self.apply(|p| p.reset_ui_defaults(), cx);
        self.state.update(cx, |s, cx| s.set_status("Reset UI preferences to defaults", cx));
    }
}

impl Render for PreferencesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.state.read(cx).prefs.clone();
        let theme = cx.theme();
        let header = div()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(div().text_base().text_color(theme.foreground).child("Preferences"))
                    .child(div().text_xs().text_color(theme.muted_foreground).child("Saved automatically · ⌘+ / ⌘- / ⌘0 to zoom anytime")),
            )
            .child(
                Button::new("pref-reset-all")
                    .outline()
                    .small()
                    .label("Reset to defaults")
                    .on_click(cx.listener(|this, _, w, cx| this.reset_defaults(w, cx))),
            );

        let updates = render_updates_card(self, &prefs, cx);
        let appearance = render_appearance_card(&prefs, cx);
        let typography = render_typography_card(self, &prefs, cx);
        let viewer_preview = render_viewer_and_preview_cards(&prefs, cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("preferences-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .child(
                        div()
                            .max_w(px(780.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(updates)
                            .child(appearance)
                            .child(typography)
                            .child(viewer_preview),
                    ),
            )
    }
}
