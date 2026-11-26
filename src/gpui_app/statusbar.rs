use gpui_kit::{
    component::{spinner::Spinner, status_bar::StatusBar, ActiveTheme as _, Sizable as _},
    div, px, App, IntoElement, ParentElement as _, Styled as _,
};

use super::pajamas_theme::status_color;
use super::state::AppState;

pub fn render_status_bar(state: &AppState, cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let host = state
        .active_host
        .as_ref()
        .map(|h| h.hostname.clone())
        .unwrap_or_else(|| "no host".into());
    let user = state
        .current_user
        .as_ref()
        .map(|u| format!("@{}", u.username))
        .or_else(|| {
            state
                .active_host
                .as_ref()
                .and_then(|h| h.user.as_ref().map(|u| format!("@{u}")))
        })
        .unwrap_or_else(|| "signed out".into());
    let live_dot = if state.summary.following_log {
        Some(
            div()
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py(px(1.))
                .rounded_full()
                .bg(status_color("running", dark).opacity(0.18))
                .text_color(status_color("running", dark))
                .child("● Live log"),
        )
    } else {
        None
    };

    let mut bar = StatusBar::new().left(
        div()
            .flex()
            .items_center()
            .gap_2()
            .children(state.busy.then(|| Spinner::new().xsmall()))
            .child(state.status.clone()),
    );

    if let Some(pill) = live_dot {
        bar = bar.right(pill);
    }

    bar.right(div().text_color(theme.foreground).child(user))
        .right(div().text_color(theme.muted_foreground).child("·"))
        .right(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_color(theme.muted_foreground)
                .child(host),
        )
}
