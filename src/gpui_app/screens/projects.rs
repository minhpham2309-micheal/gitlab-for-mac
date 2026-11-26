use gpui_kit::{
    component::{
        button::Button,
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Task, Window,
};

use crate::gitlab::Project;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::state::{AppEvent, AppState, ScreenCommand};
use super::projects_row::render_project_row;

pub struct ProjectsView {
    state: Entity<AppState>,
    search_input: Entity<InputState>,
    search_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ProjectsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Filter projects locally, or press Enter to search GitLab…")
        });
        let sub_input = cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| {
            match ev {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.remote_search(cx),
                _ => {}
            }
        });
        let sub_state = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| {
            match ev {
                AppEvent::HostChanged { .. } => {
                    this.search_task = None;
                    this.search_input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    cx.notify();
                }
                AppEvent::Command(ScreenCommand::Refresh)
                    if this.state.read(cx).screen == Screen::Projects =>
                {
                    this.state.update(cx, |s, cx| s.refresh_host_data(cx));
                }
                _ => {}
            }
        });
        let obs_state = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            search_input,
            search_task: None,
            _subs: vec![sub_input, sub_state, obs_state],
        }
    }

    fn remote_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().trim().to_string();
        if query.len() < 2 {
            self.state.update(cx, |s, cx| s.refresh_host_data(cx));
            return;
        }
        let st = self.state.read(cx);
        let Some(client) = st.gitlab.clone() else {
            return;
        };
        let epoch = st.host_epoch;
        self.state.update(cx, |s, cx| {
            s.busy = true;
            s.set_status(format!("Searching GitLab for '{query}'…"), cx);
        });
        self.search_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.search_projects(&query).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                view.state.update(cx, |s, cx| {
                    if s.host_epoch != epoch {
                        return;
                    }
                    s.busy = false;
                    match res {
                        Ok(Ok(projects)) => {
                            let n = projects.len();
                            s.projects = projects;
                            s.set_status(format!("Found {n} projects"), cx);
                        }
                        Ok(Err(err)) | Err(err) => s.set_status(err, cx),
                    }
                });
            });
        }));
    }

    fn filtered(&self, cx: &Context<Self>) -> Vec<Project> {
        let q = self.search_input.read(cx).value().to_lowercase();
        let st = self.state.read(cx);
        st.projects
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.path_with_namespace.to_lowercase().contains(&q)
                    || p.name.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }
}

impl Render for ProjectsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let projects = self.filtered(cx);
        let active_id = self.state.read(cx).project.as_ref().map(|p| p.id);
        let total = projects.len();
        let state_reload = self.state.clone();
        let state_click = self.state.clone();
        let client_for_av = self.state.read(cx).gitlab.clone();

        let list = uniform_list("projects-uniform-list", total, move |range, _, cx| {
            range
                .map(|ix| {
                    let project = projects[ix].clone();
                    let is_active = active_id == Some(project.id);
                    render_project_row(project, is_active, client_for_av.as_ref(), state_click.clone(), cx)
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
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().text_base().text_color(theme.foreground).child(format!("Projects ({total})")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w(px(420.))
                            .child(div().flex_1().child(Input::new(&self.search_input)))
                            .child(
                                Button::new("projects-reload")
                                    .outline()
                                    .small()
                                    .icon(Icon::new(IconName::Redo2).size_3p5())
                                    .label("Reload")
                                    .on_click(move |_, _, cx| {
                                        state_reload.update(cx, |s, cx| s.refresh_host_data(cx));
                                    }),
                            ),
                    ),
            )
            .child(list)
    }
}
