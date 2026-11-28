use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, AppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::Runner;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::pajamas_theme::status_badge_palette;
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunnerTab { #[default] All, Online, Offline, Paused }

impl RunnerTab {
    const ALL: [Self; 4] = [Self::All, Self::Online, Self::Offline, Self::Paused];
    fn label(self) -> &'static str {
        match self { Self::All => "All", Self::Online => "Online", Self::Offline => "Offline", Self::Paused => "Paused" }
    }
    fn key(self) -> &'static str {
        match self { Self::All => "all", Self::Online => "online", Self::Offline => "offline", Self::Paused => "paused" }
    }
}

fn runner_status(r: &Runner) -> String {
    if r.paused.unwrap_or(false) { "paused".into() } else { r.status.clone().unwrap_or_else(|| "offline".into()) }
}

pub struct RunnersView {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    tab: RunnerTab,
    runners: Vec<Runner>,
    loading: bool,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl RunnersView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter runners by description or ID…"));
        let sub_in = cx.subscribe_in(&filter, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                (this.task, this.loading, this.tab) = (None, false, RunnerTab::All);
                this.runners.clear();
                if this.state.read(cx).screen == Screen::Runners { this.load(cx); }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Runners && this.runners.is_empty() => this.load(cx),
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Runners => this.load(cx),
            _ => {}
        });
        Self { state, filter, tab: RunnerTab::All, runners: Vec::new(), loading: false, task: None, _subs: vec![sub_in, sub_st] }
    }

    pub fn tab(&self) -> RunnerTab { self.tab }
    pub fn set_fixture_data(&mut self, runners: Vec<Runner>, cx: &mut Context<Self>) {
        (self.runners, self.loading) = (runners, false);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        (self.loading, _) = (true, cx.notify());
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.runners(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(items)) => v.runners = items,
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for RunnersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.filter.read(cx).value().trim().to_ascii_lowercase();
        let (all_n, on_n, off_n, paused_n) = (
            self.runners.len(),
            self.runners.iter().filter(|r| runner_status(r) == "online").count(),
            self.runners.iter().filter(|r| !matches!(runner_status(r).as_str(), "online" | "paused")).count(),
            self.runners.iter().filter(|r| runner_status(r) == "paused").count(),
        );
        let rows: Vec<Runner> = self.runners.iter().filter(|r| {
            let st = runner_status(r);
            let tab_ok = match self.tab {
                RunnerTab::All => true,
                RunnerTab::Online => st == "online",
                RunnerTab::Paused => st == "paused",
                RunnerTab::Offline => !matches!(st.as_str(), "online" | "paused"),
            };
            tab_ok && (q.is_empty()
                || r.description.as_deref().is_some_and(|d| d.to_ascii_lowercase().contains(&q))
                || r.id.to_string().contains(&q))
        }).cloned().collect();
        let project_web = self.state.read(cx).project.as_ref().map(|p| p.web_url.clone()).filter(|u| !u.is_empty());
        let theme = cx.theme();
        let dark = theme.is_dark();
        let (inact_bg, inact_fg): (gpui_kit::Hsla, gpui_kit::Hsla) = if dark { (rgb(0x383839).into(), rgb(0xBFBFBF).into()) } else { (theme.secondary, theme.muted_foreground) };

        let tabs = div().flex().items_center().gap_1().children(RunnerTab::ALL.into_iter().map(|t| {
            let active = self.tab == t;
            let cnt = match t { RunnerTab::All => all_n, RunnerTab::Online => on_n, RunnerTab::Offline => off_n, RunnerTab::Paused => paused_n };
            let (bbg, bfg) = if active { (theme.primary, theme.primary_foreground) } else { (inact_bg, inact_fg) };
            div().id(SharedString::from(format!("runner-tab-{}", t.key()))).px_3().py_2p5().cursor_pointer()
                .border_b_2().border_color(if active { theme.primary } else { gpui_kit::transparent_black() })
                .flex().items_center().gap_1p5().text_sm()
                .text_color(if active { theme.foreground } else { theme.muted_foreground }).hover(|s| s.text_color(theme.foreground))
                .on_click(cx.listener(move |v, _, _, cx| { v.tab = t; cx.notify(); }))
                .child(div().when(active, |d| d.font_semibold()).child(t.label()))
                .child(div().px_1p5().py(px(1.)).rounded_full().bg(bbg).text_color(bfg).text_xs().font_semibold().child(cnt.to_string()))
                .test_support()
        }));

        let body = if rows.is_empty() {
            div().id("runners-empty").py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child(if self.loading { "Loading runners…" } else { "No runners found." }).test_support().into_any_element()
        } else {
            div().w_full().rounded_md().border_1().border_color(theme.border).overflow_hidden().flex().flex_col()
                .child(div().w_full().px_4().py_2p5().bg(theme.list_head).border_b_1().border_color(theme.border)
                    .flex().items_center().gap_4().text_xs().font_semibold().text_color(theme.foreground)
                    .child(div().w(px(108.)).flex_shrink_0().child("Status"))
                    .child(div().flex_1().min_w_0().child("Runner"))
                    .child(div().w(px(96.)).flex_shrink_0().text_right().child("Type")))
                .children(rows.into_iter().map(|r| {
                    let st = runner_status(&r);
                    let pal_key = match st.as_str() { "online" => "online", "paused" => "warning", _ => "skipped" };
                    let pal = status_badge_palette(pal_key, dark);
                    let kind = if r.is_shared.unwrap_or(false) { "Shared" } else { "Project" };
                    let desc = r.description.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Runner #{}", r.id));
                    div().id(SharedString::from(format!("runner-row-{}", r.id))).w_full().px_4().py_3()
                        .border_b_1().border_color(theme.border.opacity(0.6)).hover(|el| el.bg(theme.list_hover.opacity(0.45)))
                        .flex().items_center().gap_4()
                        .child(div().w(px(108.)).flex_shrink_0().child(
                            div().px_2().py(px(1.5)).rounded_full().bg(pal.bg).text_color(pal.text).flex().items_center().gap_1p5().text_xs().font_semibold()
                                .child(div().size(px(6.)).rounded_full().bg(pal.icon))
                                .child(st)))
                        .child(div().flex_1().min_w_0().flex().items_center().gap_2()
                            .child(div().text_sm().font_medium().text_color(theme.foreground).truncate().child(desc))
                            .child(div().px_1p5().py(px(0.5)).rounded_sm().bg(theme.secondary).font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.muted_foreground).child(format!("#{}", r.id))))
                        .child(div().w(px(96.)).flex_shrink_0().flex().justify_end().child(
                            div().px_2().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border).text_xs().text_color(theme.foreground).child(kind)))
                        .test_support()
                }))
                .into_any_element()
        };

        div().id("runners-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                .child(div().pb_2().flex().items_center().justify_between().gap_4()
                    .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Runners"))
                    .child(div().flex().items_center().gap_1p5()
                        .child(Button::new("runners-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                            .on_click(cx.listener(|v, _, _, cx| v.load(cx))))
                        .children(project_web.map(|u| {
                            let url = format!("{}/-/settings/ci_cd#js-runners-settings", u.trim_end_matches('/'));
                            Button::new("runners-web").ghost().small().icon(Icon::new(IconName::ExternalLink).size_3p5())
                                .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
                        }))))
                .child(div().w_full().border_b_1().border_color(theme.border).flex().items_end().child(tabs))
                .child(div().mt_3().mb_3().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border)
                    .child(div().h(px(32.)).rounded_md().bg(theme.background).border_1().border_color(theme.input).px_2().flex().items_center().gap_1p5()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.filter).appearance(false).cleanable(true).small()))
                        .child(Icon::new(IconName::Search).size_3p5().text_color(theme.muted_foreground))))
                .child(div().id("runners-scroll").flex_1().min_h_0().overflow_y_scroll().pb_6().child(body)))
            .test_support()
    }
}
