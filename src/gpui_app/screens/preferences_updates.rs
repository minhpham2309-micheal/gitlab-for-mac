use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
    },
    div, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::prefs::Prefs;
use crate::updater::{current_macos_arch, UpdateChannel, BUILD_SHORT_SHA, CURRENT_VERSION};

use super::super::pajamas_theme::status_color;
use super::super::state::UpdatePhase;
use super::preferences::PreferencesView;
use super::preferences_cards::{card_shell, setting_row};

pub fn render_updates_card(
    view: &PreferencesView,
    prefs: &Prefs,
    cx: &mut Context<PreferencesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let st = view.state.read(cx);
    let phase = st.update.phase.clone();
    let target = st.update.install_target.clone();
    let busy = st.update.is_busy();
    let auto_check = prefs.auto_check_updates();
    let channel = prefs.update_channel;

    let (status_text, status_fg) = match &phase {
        UpdatePhase::Idle => (
            format!("v{CURRENT_VERSION} ({}) · Click to check GitHub Releases", current_macos_arch()),
            theme.muted_foreground,
        ),
        UpdatePhase::Checking => ("Checking GitHub Releases…".into(), theme.muted_foreground),
        UpdatePhase::UpToDate { note: Some(note), .. } => (note.clone(), theme.muted_foreground),
        UpdatePhase::UpToDate { note: None, .. } => (
            format!("v{CURRENT_VERSION} ({}) · Up to date", current_macos_arch()),
            status_color("success", dark),
        ),
        UpdatePhase::Available(info) => (
            format!("v{} available (installed v{CURRENT_VERSION})", info.version),
            status_color("success", dark),
        ),
        UpdatePhase::Downloading { info, .. } => (
            format!("Downloading v{}…", info.version),
            status_color("running", dark),
        ),
        UpdatePhase::Verifying { info } => (
            format!("Verifying v{}…", info.version),
            status_color("running", dark),
        ),
        UpdatePhase::Installing { info } => (
            format!("Installing v{} and restarting…", info.version),
            status_color("running", dark),
        ),
        UpdatePhase::Error { message, .. } => (
            format!("Last check failed: {message}"),
            status_color("failed", dark),
        ),
    };

    let has_actionable = st.update.info().is_some();
    let version_controls = div()
        .id("pref-update-controls")
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_1p5()
        .children(busy.then(|| Spinner::new().xsmall()))
        .children(has_actionable.then(|| {
            Button::new("pref-update-open-modal")
                .primary()
                .small()
                .label("View & Install")
                .on_click(cx.listener(|v, _, _, cx| {
                    v.state.update(cx, |s, cx| s.open_update_modal(cx));
                }))
        }))
        .child(
            Button::new("pref-update-check")
                .outline()
                .small()
                .disabled(busy)
                .label("Check for Updates")
                .on_click(cx.listener(|v, _, _, cx| {
                    v.state.update(cx, |s, cx| s.check_for_updates(true, cx));
                })),
        )
        .test_support();

    let version_heading = match BUILD_SHORT_SHA {
        Some(sha) => format!("Version v{CURRENT_VERSION} ({sha})"),
        None => format!("Version v{CURRENT_VERSION}"),
    };
    let blocker_note = target.info_note().map(|note| {
        div()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(note)
    });

    card_shell("Software Updates", cx)
        .id("pref-updates-card")
        .child(
            div()
                .w_full()
                .min_w_0()
                .pt_2()
                .border_t_1()
                .border_color(theme.border.opacity(0.45))
                .flex()
                .items_start()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(div().text_sm().text_color(theme.foreground).child(version_heading))
                        .child(div().min_w_0().text_xs().text_color(status_fg).child(status_text)),
                )
                .child(version_controls),
        )
        .child(setting_row(
            "Automatic update checks",
            "Check GitHub Releases in the background on startup",
            div()
                .flex()
                .gap_1p5()
                .child(
                    Button::new("pref-update-auto-on")
                        .outline()
                        .small()
                        .selected(auto_check)
                        .label("Enabled")
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.state.update(cx, |s, cx| s.set_auto_check_updates(true, cx));
                        })),
                )
                .child(
                    Button::new("pref-update-auto-off")
                        .outline()
                        .small()
                        .selected(!auto_check)
                        .label("Disabled")
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.state.update(cx, |s, cx| s.set_auto_check_updates(false, cx));
                        })),
                ),
            cx,
        ))
        .child(setting_row(
            "Release channel",
            "Stable tagged releases or rolling post-merge Canary builds",
            div()
                .flex()
                .gap_1p5()
                .child(
                    Button::new("pref-update-ch-stable")
                        .outline()
                        .small()
                        .selected(channel == UpdateChannel::Stable)
                        .label("Stable")
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.state.update(cx, |s, cx| s.set_update_channel(UpdateChannel::Stable, cx));
                        })),
                )
                .child(
                    Button::new("pref-update-ch-canary")
                        .outline()
                        .small()
                        .selected(channel == UpdateChannel::Canary)
                        .label("Canary")
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.state.update(cx, |s, cx| s.set_update_channel(UpdateChannel::Canary, cx));
                        })),
                ),
            cx,
        ))
        .children(blocker_note)
        .test_support()
}
