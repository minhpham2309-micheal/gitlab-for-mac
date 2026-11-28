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

use crate::gitlab::Schedule;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::super::super::pajamas_theme::status_badge_palette;
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScheduleTab { #[default] All, Active, Inactive }

impl ScheduleTab {
    const ALL: [Self; 3] = [Self::All, Self::Active, Self::Inactive];
    fn label(self) -> &'static str { match self { Self::All => "All", Self::Active => "Active", Self::Inactive => "Inactive" } }
    fn key(self) -> &'static str { match self { Self::All => "all", Self::Active => "active", Self::Inactive => "inactive" } }
}

pub struct SchedulesView {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    tab: ScheduleTab,
    schedules: Vec<Schedule>,
    loading: bool,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl SchedulesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter pipeline schedules…"));
        let sub_in = cx.subscribe_in(&filter, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                (this.task, this.loading, this.tab) = (None, false, ScheduleTab::All);
                this.schedules.clear();
                if this.state.read(cx).screen == Screen::Schedules { this.load(cx); }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Schedules && this.schedules.is_empty() => this.load(cx),
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Schedules => this.load(cx),
            AppEvent::PermissionsChanged => cx.notify(),
            _ => {}
        });
        Self { state, filter, tab: ScheduleTab::All, schedules: Vec::new(), loading: false, task: None, _subs: vec![sub_in, sub_st] }
    }

    pub fn tab(&self) -> ScheduleTab { self.tab }
    pub fn set_fixture_data(&mut self, schedules: Vec<Schedule>, cx: &mut Context<Self>) {
        (self.schedules, self.loading) = (schedules, false);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        (self.loading, _) = (true, cx.notify());
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.schedules(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(items)) => v.schedules = items,
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    fn confirm_play(&mut self, sched: Schedule, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if !st.project_permissions.can_run_pipelines() { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
        let weak = cx.entity().downgrade();
        open_destructive_confirm(
            DestructiveConfirm {
                title: "Play pipeline schedule".into(), action_label: "Play schedule".into(),
                host, project: project.path_with_namespace.clone(),
                target: format!("{} ({})", sched.description, sched.git_ref),
                detail: format!("Immediately trigger scheduled pipeline '{}' on {}?", sched.description, sched.git_ref),
            },
            move |_, cx| {
                let client = client.clone();
                let _ = weak.update(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                    v.task = Some(cx.spawn(async move |this, cx| {
                        let res = spawn_tokio(async move { client.play_schedule(project.id, sched.id).await.map_err(|e| e.to_string()) }).await;
                        let _ = this.update(cx, |v, cx| {
                            let cur = v.state.read(cx);
                            if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                            match res {
                                Ok(Ok(())) => v.state.update(cx, |s, cx| s.set_status("Pipeline schedule triggered", cx)),
                                Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                            }
                        });
                    }));
                });
            },
            window, cx,
        );
    }
}

impl Render for SchedulesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let q = self.filter.read(cx).value().trim().to_ascii_lowercase();
        let (all_n, active_n, inactive_n) = (
            self.schedules.len(),
            self.schedules.iter().filter(|s| s.active).count(),
            self.schedules.iter().filter(|s| !s.active).count(),
        );
        let rows: Vec<Schedule> = self.schedules.iter().filter(|s| {
            let tab_ok = match self.tab { ScheduleTab::All => true, ScheduleTab::Active => s.active, ScheduleTab::Inactive => !s.active };
            tab_ok && (q.is_empty() || s.description.to_ascii_lowercase().contains(&q) || s.git_ref.to_ascii_lowercase().contains(&q))
        }).cloned().collect();
        let (can_run_ci, project_web) = {
            let st = self.state.read(cx);
            (st.project_permissions.can_run_pipelines(), st.project.as_ref().map(|p| p.web_url.clone()).filter(|u| !u.is_empty()))
        };
        let theme = cx.theme();
        let dark = theme.is_dark();
        let (inact_bg, inact_fg): (gpui_kit::Hsla, gpui_kit::Hsla) = if dark { (rgb(0x383839).into(), rgb(0xBFBFBF).into()) } else { (theme.secondary, theme.muted_foreground) };

        let tabs = div().flex().items_center().gap_1().children(ScheduleTab::ALL.into_iter().map(|t| {
            let active = self.tab == t;
            let cnt = match t { ScheduleTab::All => all_n, ScheduleTab::Active => active_n, ScheduleTab::Inactive => inactive_n };
            let (bbg, bfg) = if active { (theme.primary, theme.primary_foreground) } else { (inact_bg, inact_fg) };
            div().id(SharedString::from(format!("sched-tab-{}", t.key()))).px_3().py_2p5().cursor_pointer()
                .border_b_2().border_color(if active { theme.primary } else { gpui_kit::transparent_black() })
                .flex().items_center().gap_1p5().text_sm()
                .text_color(if active { theme.foreground } else { theme.muted_foreground }).hover(|s| s.text_color(theme.foreground))
                .on_click(cx.listener(move |v, _, _, cx| { v.tab = t; cx.notify(); }))
                .child(div().when(active, |d| d.font_semibold()).child(t.label()))
                .child(div().px_1p5().py(px(1.)).rounded_full().bg(bbg).text_color(bfg).text_xs().font_semibold().child(cnt.to_string()))
                .test_support()
        }));

        let body = if rows.is_empty() {
            div().id("schedules-empty").py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child(if self.loading { "Loading pipeline schedules…" } else { "No pipeline schedules found." }).test_support().into_any_element()
        } else {
            div().w_full().rounded_md().border_1().border_color(theme.border).overflow_hidden().flex().flex_col()
                .child(div().w_full().px_4().py_2p5().bg(theme.list_head).border_b_1().border_color(theme.border)
                    .flex().items_center().gap_4().text_xs().font_semibold().text_color(theme.foreground)
                    .child(div().flex_1().min_w_0().child("Description"))
                    .child(div().w(px(150.)).flex_shrink_0().child("Target"))
                    .child(div().w(px(130.)).flex_shrink_0().child("Cron"))
                    .child(div().w(px(90.)).flex_shrink_0().child("Status"))
                    .child(div().w(px(80.)).flex_shrink_0().text_right().child("Actions")))
                .children(rows.into_iter().map(|s| {
                    let pal = status_badge_palette(if s.active { "active" } else { "skipped" }, dark);
                    let s_play = s.clone();
                    div().id(SharedString::from(format!("sched-row-{}", s.id))).w_full().px_4().py_3()
                        .border_b_1().border_color(theme.border.opacity(0.6)).hover(|el| el.bg(theme.list_hover.opacity(0.45)))
                        .flex().items_center().gap_4()
                        .child(div().flex_1().min_w_0().text_sm().font_medium().text_color(theme.foreground).truncate().child(s.description))
                        .child(div().w(px(150.)).flex_shrink_0().child(
                            div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.foreground).truncate().child(s.git_ref)))
                        .child(div().w(px(130.)).flex_shrink_0().font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.muted_foreground).truncate().child(s.cron))
                        .child(div().w(px(90.)).flex_shrink_0().child(
                            div().px_2().py(px(1.5)).rounded_full().bg(pal.bg).text_color(pal.text).text_xs().font_semibold().child(if s.active { "Active" } else { "Inactive" })))
                        .child(div().w(px(80.)).flex_shrink_0().flex().justify_end().children(can_run_ci.then(|| {
                            Button::new(SharedString::from(format!("sched-play-{}", s.id))).outline().xsmall().label("Play")
                                .on_click(cx.listener(move |v, _, w, cx| v.confirm_play(s_play.clone(), w, cx)))
                        })))
                        .test_support()
                }))
                .into_any_element()
        };

        div().id("schedules-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                .child(div().pb_2().flex().items_center().justify_between().gap_4()
                    .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Pipeline schedules"))
                    .child(div().flex().items_center().gap_1p5()
                        .child(Button::new("sched-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                            .on_click(cx.listener(|v, _, _, cx| v.load(cx))))
                        .children(project_web.map(|u| {
                            let url = format!("{}/-/pipeline_schedules", u.trim_end_matches('/'));
                            Button::new("sched-web").ghost().small().icon(Icon::new(IconName::ExternalLink).size_3p5())
                                .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
                        }))))
                .child(div().w_full().border_b_1().border_color(theme.border).flex().items_end().child(tabs))
                .child(div().mt_3().mb_3().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border)
                    .child(div().h(px(32.)).rounded_md().bg(theme.background).border_1().border_color(theme.input).px_2().flex().items_center().gap_1p5()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.filter).appearance(false).cleanable(true).small()))
                        .child(Icon::new(IconName::Search).size_3p5().text_color(theme.muted_foreground))))
                .child(div().id("schedules-scroll").flex_1().min_h_0().overflow_y_scroll().pb_6().child(body)))
            .test_support()
    }
}
