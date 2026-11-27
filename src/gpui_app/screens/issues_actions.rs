use gpui_kit::{Context, Window};

use crate::gitlab::Issue;
use crate::runtime::spawn_tokio;

use super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::issues::IssuesView;

impl IssuesView {
    pub(crate) fn load_list(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let api_state = self.filter.api_state().to_string();
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.issues_by_state(project.id, &api_state).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(items)) => {
                        view.issues = items;
                        if let Some(iid) = view.pending_iid.take()
                            && let Some(found) = view.issues.iter().find(|i| i.iid == iid).cloned() {
                                view.select_issue(found, cx);
                                return;
                            }
                    }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn select_issue(&mut self, issue: Issue, cx: &mut Context<Self>) {
        let iid = issue.iid;
        self.selected = Some(issue.clone());
        self.notes.clear();
        self.state.update(cx, |s, _| s.summary.active_web_url = Some(issue.web_url.clone()));
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.notes_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.issue_notes(project.id, iid).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                if view.selected.as_ref().is_some_and(|i| i.iid == iid) {
                    match res {
                        Ok(Ok(notes)) => view.notes = notes,
                        Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn post_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.comment_input.read(cx).value().trim().to_string();
        if body.is_empty() { return; }
        let st = self.state.read(cx);
        if !st.project_permissions.can_comment() { return; }
        let (Some(issue), Some(client), Some(project)) =
            (self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, issue.iid);
        self.comment_input.update(cx, |i, cx| i.set_value("", window, cx));
        self.notes_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.create_issue_note(project.id, iid, &body).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || view.selected.as_ref().map(|i| i.iid) != Some(iid) {
                    return;
                }
                match res {
                    Ok(Ok(note)) => view.notes.push(note),
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn close_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(issue), Some(client), Some(project)) =
            (self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        let me_id = st.current_user.as_ref().map(|u| u.id);
        let is_author = me_id.is_some() && issue.author.as_ref().map(|a| a.id) == me_id;
        if !st.project_permissions.can_manage_issue(is_author) { return; }
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, issue.iid);
        let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
        let weak = cx.entity().downgrade();
        open_destructive_confirm(
            DestructiveConfirm {
                title: "Close issue".into(),
                action_label: "Close issue".into(),
                host,
                project: project.path_with_namespace.clone(),
                target: format!("#{} {}", issue.iid, issue.title),
                detail: format!("Close issue #{}?", issue.iid),
            },
            move |_, cx| {
                let client = client.clone();
                let _ = weak.update(cx, |view, cx| {
                    let cur = view.state.read(cx);
                    if cur.host_epoch != h_ep || cur.project_epoch != p_ep || view.selected.as_ref().map(|i| i.iid) != Some(iid) {
                        return;
                    }
                    view.notes_task = Some(cx.spawn(async move |this, cx| {
                        let res = spawn_tokio(async move {
                            client.close_issue(project.id, iid).await.map_err(|e| e.to_string())
                        })
                        .await;
                        let _ = this.update(cx, |v, cx| {
                            let cur = v.state.read(cx);
                            if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                            match res {
                                Ok(Ok(closed)) => {
                                    v.selected = Some(closed);
                                    v.load_list(cx);
                                }
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
