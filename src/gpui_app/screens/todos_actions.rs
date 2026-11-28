use gpui_kit::{Context, Window};

use crate::gitlab::{Todo, TodoTab};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::super::safe_url::safe_open_url;
use super::super::state::ScreenCommand;
use super::todos::TodosView;

impl TodosView {
    pub(crate) fn load(&mut self, cx: &mut Context<Self>) {
        if self.tab == TodoTab::Done {
            self.load_done(cx);
            return;
        }
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let Some(client) = st.gitlab.clone() else { return; };
        let h_ep = st.host_epoch;
        self.loading = true;
        cx.notify();
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.todos_by_state("pending").await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |view, cx| {
                if view.state.read(cx).host_epoch != h_ep { return; }
                view.loading = false;
                match res {
                    Ok(Ok(items)) => {
                        view.pending_todos = items;
                        view.selected_ids.retain(|id| view.pending_todos.iter().any(|t| t.id == *id));
                        view.state.update(cx, |s, cx| s.refresh_sidebar_counts(cx));
                    }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn load_done(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let Some(client) = st.gitlab.clone() else { return; };
        let h_ep = st.host_epoch;
        self.loading = true;
        cx.notify();
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.todos_by_state("done").await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |view, cx| {
                if view.state.read(cx).host_epoch != h_ep { return; }
                view.loading = false;
                match res {
                    Ok(Ok(items)) => view.done_todos = items,
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn toggle_select_id(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.selected_ids.remove(&id) { self.selected_ids.insert(id); }
        cx.notify();
    }

    pub(crate) fn toggle_select_all(&mut self, visible_ids: &[u64], cx: &mut Context<Self>) {
        let all_sel = !visible_ids.is_empty() && visible_ids.iter().all(|id| self.selected_ids.contains(id));
        if all_sel { self.selected_ids.clear(); } else { self.selected_ids.extend(visible_ids.iter().copied()); }
        cx.notify();
    }

    pub(crate) fn snooze_todo(&mut self, id: u64, cx: &mut Context<Self>) {
        self.snoozed_ids.insert(id);
        self.selected_ids.remove(&id);
        cx.notify();
    }

    pub(crate) fn unsnooze_todo(&mut self, id: u64, cx: &mut Context<Self>) {
        self.snoozed_ids.remove(&id);
        self.selected_ids.remove(&id);
        cx.notify();
    }

    pub(crate) fn mark_done(&mut self, id: u64, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let Some(client) = st.gitlab.clone().filter(|_| !st.offline_mode) else {
            self.finish_done_ids(&[id], cx);
            return;
        };
        let h_ep = st.host_epoch;
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.mark_todo_done(id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |view, cx| {
                if view.state.read(cx).host_epoch != h_ep { return; }
                match res {
                    Ok(Ok(())) => view.finish_done_ids(&[id], cx),
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }

    pub(crate) fn mark_selected_done(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.selected_ids.iter().copied().collect();
        if ids.is_empty() { return; }
        let st = self.state.read(cx);
        let Some(client) = st.gitlab.clone().filter(|_| !st.offline_mode) else {
            self.finish_done_ids(&ids, cx);
            return;
        };
        let h_ep = st.host_epoch;
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let ids_req = ids.clone();
            let res = spawn_tokio(async move {
                let mut completed = Vec::new();
                for id in ids_req {
                    if client.mark_todo_done(id).await.is_ok() { completed.push(id); }
                }
                completed
            }).await;
            let _ = this.update(cx, |view, cx| {
                if view.state.read(cx).host_epoch != h_ep { return; }
                if let Ok(completed) = res { view.finish_done_ids(&completed, cx); }
            });
        }));
    }

    fn finish_done_ids(&mut self, ids: &[u64], cx: &mut Context<Self>) {
        for id in ids {
            if let Some(pos) = self.pending_todos.iter().position(|t| t.id == *id) {
                let mut item = self.pending_todos.remove(pos);
                item.state = "done".into();
                self.done_todos.insert(0, item);
            }
            self.snoozed_ids.remove(id);
            self.selected_ids.remove(id);
        }
        self.state.update(cx, |s, cx| s.refresh_sidebar_counts(cx));
        cx.notify();
    }

    pub(crate) fn mark_all_confirmed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let Some(client) = st.gitlab.clone() else { return; };
        let h_ep = st.host_epoch;
        let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
        let count = self.pending_todos.len();
        let weak = cx.entity().downgrade();
        open_destructive_confirm(
            DestructiveConfirm {
                title: "Mark all To-Dos as done".into(), action_label: "Mark all done".into(),
                host, project: "All projects".into(), target: format!("{count} pending items"),
                detail: "Mark every pending To-Do item on this GitLab host as done?".into(),
            },
            move |_, cx| {
                let client = client.clone();
                let _ = weak.update(cx, |view, cx| {
                    if view.state.read(cx).host_epoch != h_ep { return; }
                    view.load_task = Some(cx.spawn(async move |this, cx| {
                        let res = spawn_tokio(async move { client.mark_all_todos_done().await.map_err(|e| e.to_string()) }).await;
                        let _ = this.update(cx, |v, cx| {
                            if v.state.read(cx).host_epoch != h_ep { return; }
                            match res {
                                Ok(Ok(())) => {
                                    v.pending_todos.clear();
                                    v.snoozed_ids.clear();
                                    v.selected_ids.clear();
                                    v.state.update(cx, |s, cx| {
                                        s.set_status("All todos marked done", cx);
                                        s.refresh_sidebar_counts(cx);
                                    });
                                }
                                Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                            }
                            cx.notify();
                        });
                    }));
                });
            },
            window, cx,
        );
    }

    pub(crate) fn open_todo(&mut self, todo: Todo, cx: &mut Context<Self>) {
        if let (Some(proj), Some(kind), Some(iid)) = (
            todo.project.as_ref(), todo.target_type.as_deref(), todo.target.as_ref().and_then(|t| t.iid),
        ) {
            let screen = match kind {
                "MergeRequest" => Some((Screen::MergeRequests, ScreenCommand::OpenMr { project_id: proj.id, iid })),
                "Issue" => Some((Screen::Issues, ScreenCommand::OpenIssue { project_id: proj.id, iid })),
                _ => None,
            };
            if let Some((target_screen, cmd)) = screen {
                let st = self.state.read(cx);
                let h_ep = st.host_epoch;
                if let Some(existing) = st.projects.iter().find(|p| p.id == proj.id).cloned() {
                    self.state.update(cx, |s, cx| { s.select_project_on_screen(existing, target_screen, cx); s.dispatch_command(cmd, cx); });
                    return;
                }
                if let Some(client) = st.gitlab.clone().filter(|_| !st.offline_mode) {
                    let pid = proj.id;
                    self.load_task = Some(cx.spawn(async move |this, cx| {
                        match spawn_tokio(async move { client.project(pid).await.map_err(|e| e.to_string()) }).await {
                            Ok(Ok(fetched)) => {
                                let _ = this.update(cx, |v, cx| {
                                    if v.state.read(cx).host_epoch == h_ep {
                                        v.state.update(cx, |s, cx| { s.select_project_on_screen(fetched, target_screen, cx); s.dispatch_command(cmd, cx); });
                                    }
                                });
                            }
                            Ok(Err(err)) | Err(err) => {
                                let _ = this.update(cx, |v, cx| {
                                    if v.state.read(cx).host_epoch == h_ep { v.state.update(cx, |s, cx| s.set_status(err, cx)); }
                                });
                            }
                        }
                    }));
                    return;
                }
            }
        }
        if let Some(url) = todo.target_url.as_deref() { let _ = safe_open_url(url); }
    }
}
