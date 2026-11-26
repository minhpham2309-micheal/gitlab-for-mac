use std::time::Duration;

use gpui_kit::{Context, ScrollStrategy};

use crate::gitlab::{job_is_live, pipeline_is_live, Job, Pipeline};
use crate::runtime::spawn_tokio;

use super::pipelines::PipelinesView;

const MAX_TRACE_RETRIES: u8 = 5;

impl PipelinesView {
    pub fn load_pipelines(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.pipelines(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(items)) => {
                        let live = items.iter().filter(|p| pipeline_is_live(&p.status)).count();
                        v.pipelines = items;
                        v.state.update(cx, |s, cx| {
                            s.counts.live_pipelines = live;
                            s.set_status(format!("{} pipelines", v.pipelines.len()), cx);
                        });
                        if let Some(found) = v.pending_pipeline_id.take().and_then(|id| v.pipelines.iter().find(|p| p.id == id).cloned()) {
                            v.select_pipeline(found, cx);
                        }
                        v.ensure_ci_poll(cx);
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn select_pipeline(&mut self, pipeline: Pipeline, cx: &mut Context<Self>) {
        let pid = pipeline.id;
        self.selected_pipeline = Some(pipeline);
        self.selected_job = None;
        self.trace_poll_task = None;
        self.jobs.clear();
        self.bridges.clear();
        self.wipe_log();
        self.sync_summary(cx);

        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.jobs_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (jobs, bridges) = tokio::join!(client.pipeline_jobs_all(project.id, pid), client.pipeline_bridges(project.id, pid));
                (jobs.map_err(|e| e.to_string()), bridges.unwrap_or_default())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                if v.selected_pipeline.as_ref().map(|p| p.id) != Some(pid) { return; }
                if let Ok((Ok(jobs), bridges)) = res { v.jobs = jobs; v.bridges = bridges; }
                cx.notify();
            });
        }));
    }

    pub fn select_job(&mut self, job: Job, cx: &mut Context<Self>) {
        let live = job_is_live(&job.status);
        self.selected_job = Some(job);
        self.wipe_log();
        self.auto_scroll_tail = live;
        self.sync_summary(cx);
        self.start_trace_stream(live, cx);
        cx.notify();
    }

    pub(crate) fn start_trace_stream(&mut self, keep_polling: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project), Some(job)) =
            (st.gitlab.clone(), st.project.clone(), self.selected_job.clone())
        else { return; };
        let (h_ep, p_ep, job_id) = (st.host_epoch, st.project_epoch, job.id);
        let start_offset = self.log_offset;
        self.trace_poll_task = Some(cx.spawn(async move |this, cx| {
            let mut offset = start_offset;
            let mut errors: u8 = 0;
            loop {
                let c = client.clone();
                let res = spawn_tokio(async move {
                    c.trace_raw(project.id, job_id, offset).await.map_err(|e| e.to_string())
                })
                .await;
                let mut sleep_secs: u64 = 1;
                let mut should_continue = false;
                let ok = this.update(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                    if v.selected_job.as_ref().map(|j| j.id) != Some(job_id) { return; }
                    let still_live = v.selected_job.as_ref().is_some_and(|j| job_is_live(&j.status));
                    match res {
                        Ok(Ok(chunk)) => {
                            errors = 0;
                            offset = chunk.next_offset;
                            v.log_offset = offset;
                            if !chunk.bytes.is_empty() { v.push_log_bytes(&chunk.bytes, cx); }
                            if !keep_polling {
                                v.log_model.flush_partial();
                                v.rebuild_visible_rows();
                                if v.auto_scroll_tail && !v.visible_rows.is_empty() {
                                    v.log_scroll.scroll_to_item(v.visible_rows.len() - 1, ScrollStrategy::Bottom);
                                }
                                cx.notify();
                                return;
                            }
                            should_continue = keep_polling && still_live;
                        }
                        Ok(Err(err)) | Err(err) => {
                            errors = errors.saturating_add(1);
                            if keep_polling && still_live && errors < MAX_TRACE_RETRIES {
                                sleep_secs = (1u64 << errors.min(3)).min(8);
                                should_continue = true;
                            } else {
                                v.state.update(cx, |s, cx| s.set_status(format!("Trace stream error: {err}"), cx));
                            }
                        }
                    }
                });
                if ok.is_err() || !should_continue { break; }
                cx.background_executor().timer(Duration::from_secs(sleep_secs)).await;
            }
        }));
    }

    pub(crate) fn ensure_ci_poll(&mut self, cx: &mut Context<Self>) {
        if self.ci_poll_task.is_some() || !self.visible { return; }
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.ci_poll_task = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let Ok(pid) = this.read_with(cx, |v, _| v.selected_pipeline.as_ref().map(|p| p.id)) else { break; };
            let c = client.clone();
            let res = spawn_tokio(async move {
                let pipes = c.pipelines(project.id).await.ok()?;
                let jobs_bridges = match pid {
                    Some(id) => {
                        let (j, b) = tokio::join!(c.pipeline_jobs_all(project.id, id), c.pipeline_bridges(project.id, id));
                        Some((j.ok()?, b.unwrap_or_default()))
                    }
                    None => None,
                };
                Some((pipes, jobs_bridges))
            })
            .await;
            let keep = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if !v.visible || cur.host_epoch != h_ep || cur.project_epoch != p_ep { return false; }
                if let Ok(Some((pipes, jb))) = res {
                    let live = pipes.iter().filter(|p| pipeline_is_live(&p.status)).count();
                    v.pipelines = pipes;
                    v.state.update(cx, |s, _| s.counts.live_pipelines = live);
                    if let Some(sel_pid) = v.selected_pipeline.as_ref().map(|p| p.id)
                        && let Some(fresh) = v.pipelines.iter().find(|p| p.id == sel_pid).cloned() {
                            v.selected_pipeline = Some(fresh);
                        }
                    if v.selected_pipeline.as_ref().map(|p| p.id) == pid
                        && let Some((jobs, bridges)) = jb {
                            let was_live = v.selected_job.as_ref().is_some_and(|j| job_is_live(&j.status));
                            v.jobs = jobs;
                            v.bridges = bridges;
                            if let Some(sel_id) = v.selected_job.as_ref().map(|j| j.id)
                                && let Some(updated) = v.jobs.iter().find(|j| j.id == sel_id).cloned() {
                                    let now_live = job_is_live(&updated.status);
                                    v.selected_job = Some(updated);
                                    if was_live && !now_live {
                                        v.start_trace_stream(false, cx);
                                    }
                                }
                        }
                    v.sync_summary(cx);
                    cx.notify();
                }
                true
            });
            if !matches!(keep, Ok(true)) { break; }
        }));
    }
}
