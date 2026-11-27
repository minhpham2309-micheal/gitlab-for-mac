use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Sizable as _,
    },
    div, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    TestSupportExt as _,
};

use super::preferences::PreferencesView;
use super::preferences_cards::card_shell;

pub fn render_account_card(
    view: &PreferencesView,
    cx: &mut Context<PreferencesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let st = view.state.read(cx);
    let hostname = st
        .active_host
        .as_ref()
        .map(|h| h.hostname.clone())
        .unwrap_or_else(|| "Not signed in".into());
    let username = st
        .current_user
        .as_ref()
        .map(|u| format!("@{}", u.username))
        .or_else(|| st.active_host.as_ref().and_then(|h| h.user.as_ref().map(|u| format!("@{u}"))))
        .unwrap_or_else(|| "Authenticated session".into());
    let is_saved = st
        .active_host
        .as_ref()
        .is_some_and(|h| st.auth.is_saved_host(&h.hostname));
    let source_label = if is_saved {
        "Saved in GitLab for Mac"
    } else {
        "glab CLI session (signing out leaves glab config untouched)"
    };

    let controls = div()
        .id("pref-account-controls")
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_1p5()
        .child(
            Button::new("pref-account-switch")
                .outline()
                .small()
                .label("Add / Switch")
                .on_click(cx.listener(|v, _, _, cx| {
                    v.state.update(cx, |s, cx| s.open_add_account(cx));
                })),
        )
        .children(is_saved.then(|| {
            Button::new("pref-account-forget")
                .ghost()
                .small()
                .label("Forget token")
                .on_click(cx.listener(|v, _, _, cx| {
                    v.state.update(cx, |s, cx| s.sign_out(true, cx));
                }))
        }))
        .child(
            Button::new("pref-account-logout")
                .danger()
                .small()
                .label("Sign out")
                .on_click(cx.listener(|v, _, _, cx| {
                    v.state.update(cx, |s, cx| s.sign_out(false, cx));
                })),
        )
        .test_support();

    card_shell("Account", cx)
        .id("pref-account-card")
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
                        .child(div().text_sm().text_color(theme.foreground).child(format!("{username} · {hostname}")))
                        .child(div().min_w_0().text_xs().text_color(theme.muted_foreground).child(source_label)),
                )
                .child(controls),
        )
        .test_support()
}
