use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, TitleBar,
    },
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::config::HostAuth;

use super::super::pajamas_theme::{apply_theme_from_prefs, status_color};
use super::super::state::AppState;

pub(super) fn render_login_titlebar(state: &Entity<AppState>, cx: &App) -> impl IntoElement + use<> {
    let st = state.read(cx);
    let dark = cx.theme().is_dark();
    let adding = st.auth.adding_account;
    let show_update = st.update.should_show_topbar_pill(st.prefs.dismissed_update_version.as_deref());
    let update_label = st.update.topbar_label();
    let update_busy = st.update.is_busy();

    let back_st = state.clone();
    let back_btn = adding.then(|| {
        Button::new("login-back-workspace").ghost().xsmall()
            .icon(Icon::new(IconName::ArrowLeft).size_3p5()).label("Back")
            .on_click(move |_, _, cx| back_st.update(cx, |s, cx| s.cancel_add_account(cx)))
    });

    let update_pill = if show_update
        && let Some(label) = update_label
    {
        let upd_st = state.clone();
        let accent = status_color(if update_busy { "running" } else { "success" }, dark);
        Some(
            div().id("login-update-pill").h(px(24.)).px_2().rounded_full()
                .bg(accent.opacity(0.18)).border_1().border_color(accent.opacity(0.45))
                .text_xs().text_color(accent).flex().items_center().gap_1().cursor_pointer()
                .on_click(move |_, _, cx| upd_st.update(cx, |s, cx| s.open_update_modal(cx)))
                .children(update_busy.then(|| Spinner::new().xsmall()))
                .child(SharedString::from(label)),
        )
    } else {
        None
    };

    let (check_st, theme_st) = (state.clone(), state.clone());
    TitleBar::new().child(
        div().w_full().pr_3().flex().items_center().justify_between()
            .child(div().flex().items_center().gap_1().children(back_btn))
            .child(
                div().flex().items_center().gap_1()
                    .children(update_pill)
                    .child(Button::new("login-check-updates").ghost().xsmall().label("Updates").on_click(move |_, _, cx| {
                        check_st.update(cx, |s, cx| {
                            s.open_update_modal(cx);
                            s.check_for_updates(true, cx);
                        });
                    }))
                    .child(Button::new("login-toggle-theme").ghost().xsmall()
                        .icon(Icon::new(if dark { IconName::Sun } else { IconName::Moon }).size_3p5())
                        .on_click(move |_, _, cx| {
                            let prefs = theme_st.update(cx, |s, cx| s.toggle_theme(cx));
                            apply_theme_from_prefs(&prefs, cx);
                        })),
            ),
    )
}

pub(super) fn render_discovered_accounts_card(
    state: &Entity<AppState>,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let st = state.read(cx);
    let busy = st.auth.busy;
    let saved = st.auth.saved_hosts.clone();
    let glab_only: Vec<HostAuth> = st
        .auth
        .glab_hosts
        .iter()
        .filter(|g| !saved.iter().any(|s| s.hostname == g.hostname))
        .cloned()
        .collect();
    let has_any = !saved.is_empty() || !glab_only.is_empty();
    let refresh_st = state.clone();

    div()
        .id("login-accounts-card").w_full().p_4().rounded_lg()
        .border_1().border_color(theme.border).bg(theme.sidebar)
        .flex().flex_col().gap_2p5()
        .child(
            div().flex().items_center().justify_between().gap_2()
                .child(div().text_xs().text_color(theme.foreground).child("Quick sign in (Saved & glab CLI)"))
                .child(
                    Button::new("login-refresh-glab").ghost().xsmall().disabled(busy)
                        .icon(Icon::new(IconName::RefreshCw).size_3()).label("Detect glab")
                        .on_click(move |_, _, cx| refresh_st.update(cx, |s, cx| s.reload_discovered_accounts(cx))),
                ),
        )
        .children(saved.into_iter().enumerate().map(|(ix, host)| {
            render_account_row(format!("login-saved-{ix}"), host, "Saved", true, busy, state, cx)
        }))
        .children(glab_only.into_iter().enumerate().map(|(ix, host)| {
            render_account_row(format!("login-glab-{ix}"), host, "glab CLI", false, busy, state, cx)
        }))
        .children((!has_any).then(|| {
            div().text_xs().text_color(theme.muted_foreground)
                .child("No saved accounts or glab CLI sessions detected. Run `glab auth login` or sign in above.")
        }))
}

fn render_account_row(
    id_prefix: String,
    host: HostAuth,
    source_badge: &'static str,
    removable: bool,
    busy: bool,
    state: &Entity<AppState>,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let subtitle = host
        .user
        .as_deref()
        .map(|u| format!("@{u} · {source_badge}"))
        .unwrap_or_else(|| source_badge.into());
    let hostname = host.hostname.clone();
    let (sign_in_st, remove_st) = (state.clone(), state.clone());
    let remove_host = host.hostname.clone();

    div()
        .w_full().min_w_0().pt_2().border_t_1().border_color(theme.border.opacity(0.45))
        .flex().items_center().justify_between().gap_2()
        .child(
            div().flex_1().min_w_0().flex().flex_col()
                .child(div().text_sm().text_color(theme.foreground).truncate().child(hostname))
                .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(subtitle)),
        )
        .child(
            div().flex_shrink_0().flex().items_center().gap_1()
                .child(
                    Button::new(SharedString::from(format!("{id_prefix}-use"))).outline().xsmall().disabled(busy).label("Sign in")
                        .on_click(move |_, _, cx| {
                            let h = host.clone();
                            sign_in_st.update(cx, |s, cx| s.sign_in_with_existing_host(h, cx));
                        }),
                )
                .children(removable.then(|| {
                    Button::new(SharedString::from(format!("{id_prefix}-del"))).ghost().xsmall().disabled(busy).label("×")
                        .on_click(move |_, _, cx| {
                            let h = remove_host.clone();
                            remove_st.update(cx, |s, cx| s.remove_saved_account(&h, cx));
                        })
                })),
        )
}
