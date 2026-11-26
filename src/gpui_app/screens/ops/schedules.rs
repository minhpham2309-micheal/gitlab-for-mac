use gpui_kit::{
    component::{button::Button, ActiveTheme as _, Sizable as _},
    div, px, uniform_list, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, Window,
};

use crate::gitlab::Schedule;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::super::super::pajamas_theme::status_color;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct SchedulesView {
    state: Entity<AppState>,
    schedules: Vec<Schedule>,
    selected_id: Option<u64>,
    task: Option<Task<()>>,
    _sub: Subscription,
}

impl SchedulesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.schedules.clear();
                this.selected_id = None;
                if this.state.read(cx).screen == Screen::Schedules {
                    this.load(cx);
                }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Schedules && this.schedules.is_empty() => {
                this.load(cx);
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Schedules => {
                this.load(cx);
            }
            _ => {}
        });
        Self { state, schedules: Vec::new(), selected_id: None, task: None, _sub }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.schedules(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
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
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
        let weak = cx.entity().downgrade();
        open_destructive_confirm(
            DestructiveConfirm {
                title: "Play pipeline schedule".into(),
                action_label: "Play schedule".into(),
                host,
                project: project.path_with_namespace.clone(),
                target: format!("{} ({})", sched.description, sched.git_ref),
                detail: format!("Immediately trigger scheduled pipeline '{}' on {}?", sched.description, sched.git_ref),
            },
            move |_, cx| {
                let client = client.clone();
                let _ = weak.update(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                    v.task = Some(cx.spawn(async move |this, cx| {
                        let res = spawn_tokio(async move {
                            client.play_schedule(project.id, sched.id).await.map_err(|e| e.to_string())
                        })
                        .await;
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
            window,
            cx,
        );
    }
}

impl Render for SchedulesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let dark = theme.is_dark();
        let items = self.schedules.clone();
        let selected = self.selected_id;
        let weak = cx.entity().downgrade();

        let list = uniform_list("schedules-uniform-list", items.len(), move |range, _, cx| {
            let theme = cx.theme();
            range
                .map(|ix| {
                    let s = items[ix].clone();
                    let is_sel = selected == Some(s.id);
                    let w_sel = weak.clone();
                    let w_play = weak.clone();
                    let s_play = s.clone();
                    let badge_status = if s.active { "active" } else { "skipped" };
                    let color = status_color(badge_status, dark);
                    div()
                        .id(SharedString::from(format!("sched-row-{}", s.id)))
                        .h(px(58.))
                        .px_4()
                        .border_b_1()
                        .border_color(theme.border.opacity(0.5))
                        .cursor_pointer()
                        .bg(if is_sel { theme.list_active } else { theme.colors.list })
                        .hover(|el| el.bg(theme.list_hover))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .on_click(move |_, _, cx| {
                            let _ = w_sel.update(cx, |v, cx| {
                                v.selected_id = Some(s.id);
                                cx.notify();
                            });
                        })
                        .child(
                            div().flex().flex_col().min_w_0().gap_1()
                                .child(div().text_sm().text_color(theme.foreground).truncate().child(s.description))
                                .child(
                                    div().flex().items_center().gap_2().text_xs()
                                        .child(div().px_1p5().rounded_sm().bg(color.opacity(0.18)).text_color(color).child(if s.active { "Active" } else { "Paused" }))
                                        .child(div().text_color(theme.muted_foreground).child(s.git_ref))
                                        .child(div().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(s.cron)),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("sched-play-{}", s.id)))
                                .outline()
                                .xsmall()
                                .label("Play")
                                .on_click(move |_, w, cx| {
                                    cx.stop_propagation();
                                    let _ = w_play.update(cx, |v, cx| v.confirm_play(s_play.clone(), w, cx));
                                }),
                        )
                })
                .collect()
        })
        .flex_1()
        .size_full();

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_base().text_color(theme.foreground).child(format!("Pipeline schedules ({})", self.schedules.len())))
                    .child(Button::new("sched-reload").outline().small().label("Reload").on_click(cx.listener(|this, _, _, cx| this.load(cx)))),
            )
            .child(list)
    }
}
