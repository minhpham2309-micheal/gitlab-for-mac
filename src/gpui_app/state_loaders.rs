use gpui_kit::Context;

use crate::config::{self, GlabConfig};
use crate::gitlab::GitLab;
use crate::prefs::{self, Prefs};
use crate::runtime::spawn_tokio;

use super::state::AppState;
use super::state_types::AppEvent;

impl AppState {
    pub fn update_prefs(&mut self, f: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) -> Prefs {
        f(&mut self.prefs);
        self.theme_dark = self.prefs.is_dark();
        self.sidebar_collapsed = self.prefs.sidebar_collapsed;
        self.save_prefs();
        cx.emit(AppEvent::PrefsChanged);
        cx.notify();
        self.prefs.clone()
    }

    pub fn step_ui_font_size(&mut self, delta: i16, cx: &mut Context<Self>) -> Prefs {
        let cur = self.prefs.ui_font_size() as i16;
        let next = (cur + delta)
            .clamp(prefs::MIN_UI_FONT_SIZE as i16, prefs::MAX_UI_FONT_SIZE as i16)
            as u16;
        let prefs = self.update_prefs(|p| p.ui_font_size = Some(next), cx);
        self.status = format!("UI font size: {next}px");
        prefs
    }

    pub fn reset_ui_font_size(&mut self, cx: &mut Context<Self>) -> Prefs {
        let prefs = self.update_prefs(|p| p.ui_font_size = Some(prefs::DEFAULT_UI_FONT_SIZE), cx);
        self.status = format!("UI font size: {}px", prefs::DEFAULT_UI_FONT_SIZE);
        prefs
    }

    pub fn boot(cx: &mut Context<Self>) -> Self {
        let prefs = prefs::load();
        match config::load() {
            Ok(cfg) => Self::from_config(cfg, prefs, cx),
            Err(err) => {
                let mut state =
                    Self::from_parts(Vec::new(), None, Vec::new(), prefs, err.to_string());
                state.persist_prefs = true;
                state.offline_mode = false;
                state
            }
        }
    }

    fn from_config(cfg: GlabConfig, prefs: Prefs, cx: &mut Context<Self>) -> Self {
        let picked = config::pick_host(&cfg, prefs.host.as_deref());
        let mut state = Self::from_parts(cfg.hosts, picked.clone(), Vec::new(), prefs, "Connecting…");
        state.persist_prefs = true;
        state.offline_mode = false;
        if let Some(host) = picked {
            match GitLab::new(host) {
                Ok(client) => {
                    state.status = format!("Signed in on {}", client.host());
                    state.gitlab = Some(client);
                    state.refresh_host_data(cx);
                }
                Err(err) => state.status = err.to_string(),
            }
        }
        state
    }

    pub fn refresh_sidebar_counts(&mut self, cx: &mut Context<Self>) {
        if self.offline_mode {
            return;
        }
        let Some(client) = self.gitlab.clone() else {
            return;
        };
        let host_epoch = self.host_epoch;
        let project_epoch = self.project_epoch;
        let project_id = self.project.as_ref().map(|p| p.id);
        self.counts_task = Some(cx.spawn(async move |this, cx| {
            let fetched = spawn_tokio(async move {
                let todos_fut = client.fetch_total_count("/todos?state=pending");
                let mrs_fut = async {
                    match project_id {
                        Some(id) => client
                            .fetch_total_count(&format!("/projects/{id}/merge_requests?state=opened"))
                            .await
                            .ok()
                            .flatten(),
                        None => None,
                    }
                };
                let issues_fut = async {
                    match project_id {
                        Some(id) => client
                            .fetch_total_count(&format!("/projects/{id}/issues?state=opened"))
                            .await
                            .ok()
                            .flatten(),
                        None => None,
                    }
                };
                let perms_fut = async {
                    match project_id {
                        Some(id) => client.project_permissions(id).await.ok(),
                        None => None,
                    }
                };
                let (todos, open_mrs, open_issues, perms) =
                    tokio::join!(todos_fut, mrs_fut, issues_fut, perms_fut);
                (todos.ok().flatten(), open_mrs, open_issues, perms)
            })
            .await;
            let _ = this.update(cx, |state, cx| {
                if state.host_epoch != host_epoch {
                    return;
                }
                if let Ok((todos, open_mrs, open_issues, perms)) = fetched {
                    state.counts.todos = todos;
                    if state.project_epoch == project_epoch {
                        state.counts.open_mrs = open_mrs;
                        state.counts.open_issues = open_issues;
                        if let Some(p) = perms {
                            state.set_project_permissions(p, cx);
                        }
                    }
                    cx.notify();
                }
            });
        }));
    }

    pub fn refresh_host_data(&mut self, cx: &mut Context<Self>) {
        if self.offline_mode {
            return;
        }
        let Some(client) = self.gitlab.clone() else {
            return;
        };
        let epoch = self.host_epoch;
        self.busy = true;
        self.host_load_task = Some(cx.spawn(async move |this, cx| {
            let fetched = spawn_tokio(async move {
                let (user_res, projects_res) =
                    tokio::join!(client.current_user(), client.projects());
                (
                    user_res.map_err(|e| e.to_string()),
                    projects_res.map_err(|e| e.to_string()),
                )
            })
            .await;
            let _ = this.update(cx, |state, cx| {
                if state.host_epoch != epoch {
                    return;
                }
                state.busy = false;
                match fetched {
                    Ok((user_res, projects_res)) => {
                        if let Ok(user) = user_res {
                            state.status = format!("Signed in as @{}", user.username);
                            state.current_user = Some(user);
                        }
                        match projects_res {
                            Ok(projects) => {
                                state.projects = projects;
                                if state.project.is_none()
                                    && let Some(saved) = state.prefs.project.clone()
                                        && let Some(found) = state
                                            .projects
                                            .iter()
                                            .find(|p| p.path_with_namespace == saved)
                                            .cloned()
                                        {
                                            state.select_project(found, cx);
                                            return;
                                        }
                                state.refresh_sidebar_counts(cx);
                            }
                            Err(err) => state.status = err,
                        }
                    }
                    Err(err) => state.status = err,
                }
                cx.notify();
            });
        }));
    }
}
