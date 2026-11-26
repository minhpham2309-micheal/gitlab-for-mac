use gpui_kit::{
    component::{button::Button, ActiveTheme as _, Sizable as _},
    div, px, uniform_list, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Task, Window,
};

use crate::gitlab::Runner;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::pajamas_theme::status_color;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct RunnersView {
    state: Entity<AppState>,
    runners: Vec<Runner>,
    task: Option<Task<()>>,
    _sub: Subscription,
}

impl RunnersView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.runners.clear();
                if this.state.read(cx).screen == Screen::Runners {
                    this.load(cx);
                }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Runners && this.runners.is_empty() => {
                this.load(cx);
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Runners => {
                this.load(cx);
            }
            _ => {}
        });
        Self { state, runners: Vec::new(), task: None, _sub }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.runners(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
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
        let theme = cx.theme();
        let dark = theme.is_dark();
        let runners = self.runners.clone();

        let list = uniform_list("runners-uniform-list", runners.len(), move |range, _, cx| {
            let theme = cx.theme();
            range
                .map(|ix| {
                    let r = &runners[ix];
                    let status = if r.paused.unwrap_or(false) {
                        "paused".to_string()
                    } else {
                        r.status.clone().unwrap_or_else(|| "offline".into())
                    };
                    let color = status_color(&status, dark);
                    let kind = if r.is_shared.unwrap_or(false) { "shared" } else { "project" };
                    let desc = r.description.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Runner #{}", r.id));
                    div()
                        .id(SharedString::from(format!("runner-row-{}", r.id)))
                        .h(px(54.))
                        .px_4()
                        .border_b_1()
                        .border_color(theme.border.opacity(0.5))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div().flex().items_center().gap_2().min_w_0()
                                .child(div().size(px(8.)).rounded_full().bg(color))
                                .child(div().text_sm().text_color(theme.foreground).truncate().child(desc))
                                .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(format!("#{}", r.id))),
                        )
                        .child(
                            div().flex().items_center().gap_2().text_xs()
                                .child(div().px_1p5().rounded_sm().bg(theme.secondary).text_color(theme.muted_foreground).child(kind))
                                .child(div().text_color(color).child(status)),
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
                    .child(div().text_base().text_color(theme.foreground).child(format!("Runners ({})", self.runners.len())))
                    .child(Button::new("runners-reload").outline().small().label("Reload").on_click(cx.listener(|this, _, _, cx| this.load(cx)))),
            )
            .child(list)
    }
}
