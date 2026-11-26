use gpui_kit::{component::WindowExt as _, Context, Window};

use crate::gitlab::{pipeline_is_live, Job};
use crate::runtime::spawn_tokio;

use super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::pipelines::PipelinesView;

impl PipelinesView {
    pub(crate) fn trigger_run_pipeline(&mut self, git_ref: String, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.action_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.run_pipeline(project.id, &git_ref).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(p)) => {
                        v.state.update(cx, |s, cx| s.set_status(format!("Started pipeline #{}", p.id), cx));
                        v.load_pipelines(cx);
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }

    pub(crate) fn confirm_cancel_pipeline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let Some(pipe) = self.selected_pipeline.clone() else { return; };
        if !pipeline_is_live(&pipe.status) { return; }
        let (h_ep, p_ep, pid) = (st.host_epoch, st.project_epoch, pipe.id);
        let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
        let project = st.project.as_ref().map(|p| p.path_with_namespace.clone()).unwrap_or_default();
        let weak = cx.entity().downgrade();
        open_destructive_confirm(
            DestructiveConfirm {
                title: "Cancel pipeline".into(),
                action_label: "Cancel pipeline".into(),
                host,
                project,
                target: format!("#{} ({})", pipe.id, pipe.git_ref),
                detail: format!("Cancel pipeline #{} on {}?", pipe.id, pipe.git_ref),
            },
            move |_, cx| {
                let _ = weak.update(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch == h_ep
                        && cur.project_epoch == p_ep
                        && v.selected_pipeline.as_ref().map(|p| p.id) == Some(pid)
                    {
                        v.run_pipeline_action("cancel", cx);
                    }
                });
            },
            window,
            cx,
        );
    }

    pub fn confirm_play_job(&mut self, job: Job, window: &mut Window, cx: &mut Context<Self>) {
        if job.status != "manual" { return; }
        let st = self.state.read(cx);
        let (h_ep, p_ep, job_id) = (st.host_epoch, st.project_epoch, job.id);
        let pid = self.selected_pipeline.as_ref().map(|p| p.id);
        let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default();
        let project = st.project.as_ref().map(|p| p.path_with_namespace.clone()).unwrap_or_default();
        let weak = cx.entity().downgrade();
        open_destructive_confirm(
            DestructiveConfirm {
                title: "Play manual job".into(),
                action_label: "Play job".into(),
                host,
                project,
                target: format!("{} ({})", job.name, job.stage),
                detail: format!("Run manual job '{}' (#{job_id})?", job.name),
            },
            move |_, cx| {
                let _ = weak.update(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch == h_ep
                        && cur.project_epoch == p_ep
                        && v.selected_pipeline.as_ref().map(|p| p.id) == pid
                    {
                        v.run_job_action(job_id, "play", cx);
                    }
                });
            },
            window,
            cx,
        );
    }

    pub fn run_job_action(&mut self, job_id: u64, kind: &'static str, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let sel_pid = self.selected_pipeline.as_ref().map(|p| p.id);
        self.action_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                match kind {
                    "retry" => client.retry_job(project.id, job_id).await,
                    "cancel" => client.cancel_job(project.id, job_id).await,
                    _ => client.play_job(project.id, job_id).await,
                }
                .map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(updated)) => {
                        v.state.update(cx, |s, cx| s.set_status(format!("Job {} ({kind})", updated.name), cx));
                        if v.selected_pipeline.as_ref().map(|p| p.id) == sel_pid {
                            if let Some(slot) = v.jobs.iter_mut().find(|j| j.id == updated.id) {
                                *slot = updated.clone();
                            }
                            if matches!(kind, "retry" | "play") {
                                v.select_job(updated, cx);
                            } else if v.selected_job.as_ref().map(|j| j.id) == Some(job_id) {
                                v.selected_job = Some(updated);
                                v.sync_summary(cx);
                                cx.notify();
                            } else {
                                cx.notify();
                            }
                        }
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }

    pub fn run_pipeline_action(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project), Some(pipe)) =
            (st.gitlab.clone(), st.project.clone(), self.selected_pipeline.clone())
        else { return; };
        if kind == "cancel" && !pipeline_is_live(&pipe.status) { return; }
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.action_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                if kind == "retry" {
                    client.retry_pipeline(project.id, pipe.id).await.map(|_| "Pipeline retried".to_string())
                } else {
                    client.cancel_pipeline(project.id, pipe.id).await.map(|_| "Pipeline canceled".to_string())
                }
                .map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(msg)) => {
                        v.state.update(cx, |s, cx| s.set_status(msg, cx));
                        v.load_pipelines(cx);
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }

    pub fn download_artifacts(&mut self, job_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        window.push_notification(format!("Downloading artifacts for job #{job_id}…"), cx);
        self.download_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.download_artifacts(project.id, job_id).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(path)) => v.state.update(cx, |s, cx| s.set_status(format!("Saved artifacts to {path}"), cx)),
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
            });
        }));
    }
}
