use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::Context;

use crate::gitlab::{pipeline_is_live, MrPipelineTarget, Pipeline};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::MergeRequestsView;

const LIVE_MR_POLL_SECS: u64 = 5;

impl MergeRequestsView {
    pub(crate) fn apply_hydrated_pipelines(&mut self, map: HashMap<u64, Pipeline>) -> bool {
        let mut changed = false;
        for mr in &mut self.mrs {
            if let Some(pipe) = map.get(&mr.iid) {
                let same = mr
                    .head_pipeline
                    .as_ref()
                    .is_some_and(|cur| cur.id == pipe.id && cur.status == pipe.status);
                if !same {
                    mr.head_pipeline = Some(pipe.clone());
                    changed = true;
                }
            }
        }
        changed
    }

    pub(crate) fn hydrate_list_pipelines(&mut self, cx: &mut Context<Self>) {
        self.pipelines_hydrate_task = None;
        let st = self.state.read(cx);
        if st.offline_mode || st.screen != Screen::MergeRequests || self.selected.is_some() {
            return;
        }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let targets: Vec<MrPipelineTarget> = self
            .mrs
            .iter()
            .filter(|m| m.head_pipeline.is_none())
            .map(|m| MrPipelineTarget {
                iid: m.iid,
                sha: m.sha.clone(),
                source_branch: m.source_branch.clone(),
            })
            .collect();
        let has_live = self
            .mrs
            .iter()
            .any(|m| m.head_pipeline.as_ref().is_some_and(|p| pipeline_is_live(&p.status)));
        if targets.is_empty() && !has_live {
            return;
        }

        self.pipelines_hydrate_task = Some(cx.spawn(async move |this, cx| {
            if !targets.is_empty() {
                let c = client.clone();
                let res = spawn_tokio(async move {
                    c.hydrate_mr_head_pipelines(project.id, targets).await
                })
                .await;
                let keep = this.update(cx, |v, cx| {
                    let (same_ctx, on_list) = {
                        let cur = v.state.read(cx);
                        (
                            cur.host_epoch == h_ep && cur.project_epoch == p_ep,
                            cur.screen == Screen::MergeRequests && v.selected.is_none(),
                        )
                    };
                    if !same_ctx { return false; }
                    if let Ok(map) = res && v.apply_hydrated_pipelines(map) {
                        cx.notify();
                    }
                    on_list
                });
                if !matches!(keep, Ok(true)) { return; }
            }

            loop {
                let Ok(live_iids) = this.read_with(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch != h_ep
                        || cur.project_epoch != p_ep
                        || cur.screen != Screen::MergeRequests
                        || v.selected.is_some()
                    {
                        return Vec::new();
                    }
                    v.mrs
                        .iter()
                        .filter(|m| m.head_pipeline.as_ref().is_some_and(|p| pipeline_is_live(&p.status)))
                        .map(|m| m.iid)
                        .collect::<Vec<_>>()
                }) else { break; };
                if live_iids.is_empty() { break; }

                cx.background_executor().timer(Duration::from_secs(LIVE_MR_POLL_SECS)).await;
                let c = client.clone();
                let res = spawn_tokio(async move {
                    c.fetch_mr_latest_pipelines(project.id, live_iids).await
                })
                .await;
                let keep = this.update(cx, |v, cx| {
                    let (same_ctx, on_list) = {
                        let cur = v.state.read(cx);
                        (
                            cur.host_epoch == h_ep && cur.project_epoch == p_ep,
                            cur.screen == Screen::MergeRequests && v.selected.is_none(),
                        )
                    };
                    if !same_ctx { return false; }
                    if let Ok(map) = res && v.apply_hydrated_pipelines(map) {
                        cx.notify();
                    }
                    on_list
                });
                if !matches!(keep, Ok(true)) { break; }
            }
        }));
    }
}
