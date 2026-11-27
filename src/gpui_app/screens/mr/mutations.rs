use gpui_kit::{Context, Window};
use serde_json::json;

use crate::gitlab::{mr_can_review, GitLabError};
use crate::runtime::spawn_tokio;

use super::super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::MergeRequestsView;

impl MergeRequestsView {
    pub fn perform_mr_mutation(
        &mut self,
        kind: &'static str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let st = self.state.read(cx);
        let (Some(mr), Some(project)) = (self.selected.clone(), st.project.clone()) else { return; };
        if kind != "reopen" && !mr_can_review(&mr.state) {
            self.state.update(cx, |s, cx| s.set_status("Only open merge requests can be mutated", cx));
            return;
        }
        if matches!(kind, "merge" | "close") && let Some(window) = window {
            let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
            let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
            let weak = cx.entity().downgrade();
            open_destructive_confirm(
                DestructiveConfirm {
                    title: if kind == "merge" { "Merge merge request".into() } else { "Close merge request".into() },
                    action_label: if kind == "merge" { "Merge".into() } else { "Close MR".into() },
                    host, project: project.path_with_namespace.clone(),
                    target: format!("!{} {}", mr.iid, mr.title),
                    detail: format!("Confirm {kind} on !{}?", mr.iid),
                },
                move |_, cx| {
                    let _ = weak.update(cx, |v, cx| {
                        let cur = v.state.read(cx);
                        if cur.host_epoch == h_ep && cur.project_epoch == p_ep && v.selected.as_ref().map(|m| m.iid) == Some(iid) {
                            v.exec_mr_mutation(kind, cx);
                        }
                    });
                },
                window, cx,
            );
            return;
        }
        self.exec_mr_mutation(kind, cx);
    }

    pub(crate) fn exec_mr_mutation(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) = (self.selected.clone(), st.gitlab.clone(), st.project.clone()) else { return; };
        if kind != "reopen" && !mr_can_review(&mr.state) {
            self.state.update(cx, |s, cx| s.set_status("Only open merge requests can be mutated", cx));
            return;
        }
        let (h_ep, p_ep, remove_br, squash) = (st.host_epoch, st.project_epoch, self.remove_source_branch, self.squash_commits);
        let mr_clone = mr.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                match kind {
                    "approve" => client.approve_mr(project.id, mr.iid).await.map(|_| "MR approved".to_string()),
                    "unapprove" => client.unapprove_mr(project.id, mr.iid).await.map(|_| "Approval revoked".to_string()),
                    "merge" => client.merge_mr_with_options(project.id, mr.iid, remove_br, squash).await.map(|_| "MR merged".to_string()),
                    "rebase" => client.rebase_mr(project.id, mr.iid).await.map(|_| "Rebase started".to_string()),
                    "reopen" => client.reopen_mr(project.id, mr.iid).await.map(|_| "MR reopened".to_string()),
                    _ => client.close_mr(project.id, mr.iid).await.map(|_| "MR closed".to_string()),
                }
            }).await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(msg)) => {
                        view.state.update(cx, |s, cx| { s.set_status(msg, cx); s.refresh_sidebar_counts(cx); });
                        view.select_mr(mr_clone, false, cx);
                    }
                    Ok(Err(GitLabError::Http { status: 403 | 404, .. })) if matches!(kind, "approve" | "unapprove") => {
                        view.state.update(cx, |s, cx| s.set_status("MR approvals unavailable on this GitLab instance", cx));
                    }
                    Ok(Err(err)) => view.state.update(cx, |s, cx| s.set_status(err.to_string(), cx)),
                    Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }

    pub fn add_mr_todo(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) = (self.selected.clone(), st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.create_mr_todo(project.id, iid).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(())) => {
                        v.todo_added = true;
                        v.state.update(cx, |s, cx| { s.set_status("Added a to-do item", cx); s.refresh_sidebar_counts(cx); });
                        cx.notify();
                    }
                    Ok(Err(e)) | Err(e) => v.state.update(cx, |s, cx| s.set_status(e, cx)),
                }
            });
        }));
    }

    pub fn toggle_award_emoji(&mut self, name: &'static str, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) = (self.selected.clone(), st.gitlab.clone(), st.project.clone()) else { return; };
        let me = st.current_user.as_ref().map(|u| u.id);
        let existing_id = me.and_then(|uid| {
            self.award_emojis.iter().find(|a| a.name == name && a.user.as_ref().is_some_and(|u| u.id == uid)).map(|a| a.id)
        });
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                if let Some(aid) = existing_id {
                    client.delete_mr_award_emoji(project.id, iid, aid).await.map(|_| (Some(aid), None)).map_err(|e| e.to_string())
                } else {
                    client.add_mr_award_emoji(project.id, iid, name).await.map(|a| (None, Some(a))).map_err(|e| e.to_string())
                }
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.selected.as_ref().map(|m| m.iid) != Some(iid) { return; }
                match res {
                    Ok(Ok((Some(removed), None))) => v.award_emojis.retain(|a| a.id != removed),
                    Ok(Ok((None, Some(added)))) => v.award_emojis.push(added),
                    Ok(Err(e)) | Err(e) => v.state.update(cx, |s, cx| s.set_status(e, cx)),
                    _ => {}
                }
                cx.notify();
            });
        }));
    }

    pub fn assign_self(&mut self, as_reviewer: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project), Some(me)) =
            (self.selected.clone(), st.gitlab.clone(), st.project.clone(), st.current_user.clone())
        else {
            self.state.update(cx, |s, cx| s.set_status("Current user unknown; sign in first", cx));
            return;
        };
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        let body = if as_reviewer {
            let mut ids: Vec<u64> = mr.reviewers.iter().map(|u| u.id).collect();
            if !ids.contains(&me.id) { ids.push(me.id); }
            json!({ "reviewer_ids": ids })
        } else {
            let mut ids: Vec<u64> = mr.assignees.iter().map(|u| u.id).collect();
            if !ids.contains(&me.id) { ids.push(me.id); }
            json!({ "assignee_ids": ids })
        };
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.update_mr_fields(project.id, iid, body).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.selected.as_ref().map(|m| m.iid) != Some(iid) { return; }
                match res {
                    Ok(Ok(updated)) => { v.selected = Some(updated); v.sync_summary(cx); cx.notify(); }
                    Ok(Err(e)) | Err(e) => v.state.update(cx, |s, cx| s.set_status(e, cx)),
                }
            });
        }));
    }

    pub fn save_mr_edit(&mut self, title: String, description: String, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) = (self.selected.clone(), st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        let body = json!({ "title": title, "description": description });
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.update_mr_fields(project.id, iid, body).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.selected.as_ref().map(|m| m.iid) != Some(iid) { return; }
                match res {
                    Ok(Ok(updated)) => {
                        v.selected = Some(updated);
                        v.sync_summary(cx);
                        v.state.update(cx, |s, cx| s.set_status("Merge request updated", cx));
                        cx.notify();
                    }
                    Ok(Err(e)) | Err(e) => v.state.update(cx, |s, cx| s.set_status(e, cx)),
                }
            });
        }));
    }
}
