use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        spinner::Spinner,
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    },
    div, px, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
};

use crate::accounts::personal_access_token_url;

use super::super::pajamas_theme::{status_color, tanuki_color};
use super::super::safe_url::safe_open_url;
use super::super::state::{AppEvent, AppState};
use super::login_sections::{render_discovered_accounts_card, render_login_titlebar};

pub struct LoginView {
    state: Entity<AppState>,
    host_input: Entity<InputState>,
    token_input: Entity<InputState>,
    _subs: Vec<Subscription>,
}

impl LoginView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_host = state
            .read(cx)
            .prefs
            .host
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "gitlab.com".into());

        let host_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("gitlab.com or gitlab.example.com");
            input.set_value(initial_host, window, cx);
            input
        });
        let token_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Personal Access Token (glpat-…)");
            input.set_masked(true, window, cx);
            input
        });

        let sub_host = cx.subscribe_in(&host_input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit(cx);
            }
        });
        let sub_token = cx.subscribe_in(&token_input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit(cx);
            }
        });
        let sub_state = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| {
            if matches!(ev, AppEvent::HostChanged { .. }) {
                this.token_input.update(cx, |input, cx| input.set_value("", window, cx));
                cx.notify();
            }
        });
        let obs = cx.observe(&state, |_, _, cx| cx.notify());
        Self { state, host_input, token_input, _subs: vec![sub_host, sub_token, sub_state, obs] }
    }

    pub fn submit(&mut self, cx: &mut Context<Self>) {
        let host = self.host_input.read(cx).value().to_string();
        let token = self.token_input.read(cx).value().to_string();
        self.state.update(cx, |s, cx| s.sign_in_with_token(host, token, cx));
    }

    fn open_pat_page(&self, cx: &Context<Self>) {
        let host = self.host_input.read(cx).value().to_string();
        let target = if host.trim().is_empty() { "gitlab.com" } else { host.as_str() };
        if let Some(url) = personal_access_token_url(target) {
            let _ = safe_open_url(&url);
        }
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let st = self.state.read(cx);
        let busy = st.auth.busy;
        let error = st.auth.error.clone();
        let title_text = if st.auth.adding_account {
            "Add or Switch GitLab Account"
        } else {
            "Sign in to GitLab for Mac"
        };

        let title_bar = render_login_titlebar(&self.state, cx);
        let discovered = render_discovered_accounts_card(&self.state, cx);
        let failed = status_color("failed", theme.is_dark());

        let pat_card = div()
            .id("login-pat-card")
            .w_full()
            .p_5()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div().flex().flex_col().gap_1()
                    .child(div().text_xs().text_color(theme.foreground).child("GitLab host"))
                    .child(Input::new(&self.host_input)),
            )
            .child(
                div().flex().flex_col().gap_1()
                    .child(
                        div().flex().items_center().justify_between().gap_2()
                            .child(div().text_xs().text_color(theme.foreground).child("Personal Access Token"))
                            .child(
                                Button::new("login-create-pat").ghost().xsmall()
                                    .icon(Icon::new(IconName::ExternalLink).size_3())
                                    .label("Create token")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_pat_page(cx))),
                            ),
                    )
                    .child(Input::new(&self.token_input).mask_toggle())
                    .child(div().text_xs().text_color(theme.muted_foreground).child("Required scopes: api, read_user, read_repository")),
            )
            .children(error.map(|msg| {
                div().id("login-error-banner").p_2p5().rounded_md()
                    .bg(failed.opacity(0.12)).border_1().border_color(failed.opacity(0.4))
                    .text_xs().text_color(theme.foreground).child(msg)
            }))
            .child(
                div().pt_1().flex().items_center().justify_end().gap_2()
                    .children(busy.then(|| Spinner::new().xsmall()))
                    .child(
                        Button::new("login-submit-btn").primary().small().disabled(busy)
                            .label(if busy { "Signing in…" } else { "Sign in" })
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            );

        div()
            .id("login-screen")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(title_bar)
            .child(
                div().id("login-scroll").flex_1().min_h_0().overflow_y_scroll().p_6().child(
                    div().w_full().max_w(px(460.)).mx_auto().pt_6().flex().flex_col().gap_4()
                        .child(
                            div().flex().flex_col().items_center().gap_2().pb_2()
                                .child(Icon::empty().path("icons/glab-tanuki.svg").size_8().text_color(tanuki_color()))
                                .child(div().text_lg().text_color(theme.foreground).child(title_text))
                                .child(div().text_xs().text_color(theme.muted_foreground).child("Connect with a Personal Access Token or an existing glab CLI session.")),
                        )
                        .child(pat_card)
                        .child(discovered),
                ),
            )
    }
}
