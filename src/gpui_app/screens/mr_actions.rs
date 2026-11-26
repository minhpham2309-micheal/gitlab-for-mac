use gpui_kit::{Context, Window};

use crate::gitlab::{mr_can_review, GitLabError, MergeRequest};
use crate::runtime::spawn_tokio;

use super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::super::diff_hl::parse_raw_diffs_individually;
use super::merge_requests::MergeRequestsView;

impl MergeRequestsView {
    pub fn load_list(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let filter_state = self.filter.api_state().to_string();
        self.state.update(cx, |s, cx| {
            s.busy = true;
            s.set_status("Loading merge requests…", cx);
        });
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.merge_requests(project.id, &filter_state).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                view.state.update(cx, |s, _| s.busy = false);
                match res {
                    Ok(Ok(items)) => {
                        let n = items.len();
                        view.mrs = items;
                        view.state.update(cx, |s, cx| s.set_status(format!("{n} merge requests"), cx));
                        if let Some(pending) = view.pending_iid.take()
                            && let Some(found) = view.mrs.iter().find(|m| m.iid == pending).cloned() {
                                view.select_mr(found, cx);
                                return;
                            }
                    }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn select_mr(&mut self, mr: MergeRequest, cx: &mut Context<Self>) {
        let iid = mr.iid;
        self.mutation_task = None;
        self.selected = Some(mr);
        self.raw_diffs.clear();
        self.fallback_notices.clear();
        self.has_truncated = false;
        self.discussions.clear();
        self.discussions_error = None;
        self.inline_target = None;
        self.replying_to = None;
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
        self.sync_summary(cx);

        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.detail_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (detail, diffs, discussions) = tokio::join!(
                    client.merge_request(project.id, iid),
                    client.merge_request_diffs(project.id, iid),
                    client.mr_discussions(project.id, iid),
                );
                (
                    detail.map_err(|e| e.to_string()),
                    diffs.map_err(|e| e.to_string()),
                    discussions.map_err(|e| e.to_string()),
                )
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                let still_selected = view.selected.as_ref().is_some_and(|m| m.iid == iid);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || !still_selected { return; }
                if let Ok((detail_res, diffs_res, disc_res)) = res {
                    if let Ok(full_mr) = detail_res {
                        view.selected = Some(full_mr);
                        view.sync_summary(cx);
                    }
                    match diffs_res {
                        Ok(raw) => {
                            let parsed = parse_raw_diffs_individually(&raw);
                            view.raw_diffs = raw;
                            view.fallback_notices = parsed.fallback_notices;
                            view.has_truncated = parsed.has_truncated;
                            view.diff_state.update(cx, |ds, cx| ds.set_files(parsed.files, cx));
                        }
                        Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                    }
                    match disc_res {
                        Ok(items) => view.discussions = items,
                        Err(err) => view.discussions_error = Some(err),
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn perform_mr_mutation(
        &mut self,
        kind: &'static str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let st = self.state.read(cx);
        let (Some(mr), Some(project)) = (self.selected.clone(), st.project.clone()) else { return; };
        if !mr_can_review(&mr.state) {
            self.state.update(cx, |s, cx| s.set_status("Only open merge requests can be mutated", cx));
            return;
        }
        if matches!(kind, "merge" | "close")
            && let Some(window) = window {
                let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
                let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
                let weak = cx.entity().downgrade();
                open_destructive_confirm(
                    DestructiveConfirm {
                        title: if kind == "merge" { "Merge merge request".into() } else { "Close merge request".into() },
                        action_label: if kind == "merge" { "Merge".into() } else { "Close MR".into() },
                        host,
                        project: project.path_with_namespace.clone(),
                        target: format!("!{} {}", mr.iid, mr.title),
                        detail: format!("Confirm {kind} on !{}?", mr.iid),
                    },
                    move |_, cx| {
                        let _ = weak.update(cx, |v, cx| {
                            let cur = v.state.read(cx);
                            if cur.host_epoch == h_ep
                                && cur.project_epoch == p_ep
                                && v.selected.as_ref().map(|m| m.iid) == Some(iid)
                            {
                                v.exec_mr_mutation(kind, cx);
                            }
                        });
                    },
                    window,
                    cx,
                );
                return;
            }
        self.exec_mr_mutation(kind, cx);
    }

    pub(crate) fn exec_mr_mutation(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) =
            (self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        if !mr_can_review(&mr.state) {
            self.state.update(cx, |s, cx| s.set_status("Only open merge requests can be mutated", cx));
            return;
        }
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                match kind {
                    "approve" => client.approve_mr(project.id, mr.iid).await.map(|_| "MR approved".to_string()),
                    "merge" => client.merge_mr(project.id, mr.iid).await.map(|_| "MR merged".to_string()),
                    _ => client.close_mr(project.id, mr.iid).await.map(|_| "MR closed".to_string()),
                }
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(msg)) => {
                        view.state.update(cx, |s, cx| {
                            s.set_status(msg, cx);
                            s.refresh_sidebar_counts(cx);
                        });
                        view.load_list(cx);
                    }
                    Ok(Err(GitLabError::Http { status: 403 | 404, .. })) if kind == "approve" => {
                        view.state.update(cx, |s, cx| {
                            s.set_status("MR approvals unavailable on this GitLab instance (CE/permissions)", cx);
                        });
                    }
                    Ok(Err(err)) => view.state.update(cx, |s, cx| s.set_status(err.to_string(), cx)),
                    Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }
}
