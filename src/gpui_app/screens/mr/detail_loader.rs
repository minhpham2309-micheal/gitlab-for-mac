use gpui_kit::Context;

use crate::gitlab::{
    AwardEmoji, Discussion, Job, MergeRequest, MrApprovals, MrListCounts, Pipeline, RawDiff,
    RepoCommit, User,
};
use crate::runtime::spawn_tokio;

use super::super::super::diff_hl::parse_raw_diffs_individually;
use super::cache::{build_activity_entries, compute_stage_statuses};
use super::MergeRequestsView;

impl MergeRequestsView {
    pub(crate) fn rebuild_activity_cache(&mut self) {
        self.cached_activity = build_activity_entries(&self.discussions, self.activity_filter, self.activity_desc);
        self.overview_list.reset(4 + self.cached_activity.len());
    }

    pub(crate) fn rebuild_stage_cache(&mut self) {
        self.cached_stages = compute_stage_statuses(&self.head_pipeline_jobs, &self.head_pipeline_bridges);
        self.overview_list.remeasure_items(1..2);
    }
    pub fn open_mr_by_iid(&mut self, iid: u64, cx: &mut Context<Self>) {
        if let Some(found) = self.mrs.iter().find(|m| m.iid == iid).cloned() {
            self.select_mr(found, false, cx);
            return;
        }
        self.pending_iid = Some(iid);
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.detail_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.merge_request_detail(project.id, iid).await.map_err(|e| e.to_string())
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch == h_ep && cur.project_epoch == p_ep && let Ok(Ok(mr)) = res {
                    v.pending_iid = None;
                    v.select_mr(mr, false, cx);
                }
            });
        }));
    }

    pub fn select_mr(&mut self, mr: MergeRequest, record_history: bool, cx: &mut Context<Self>) {
        let iid = mr.iid;
        let remove_default = mr.force_remove_source_branch.or(mr.should_remove_source_branch).unwrap_or(false);
        let squash_default = mr.squash.unwrap_or(false);
        self.pipelines_hydrate_task = None;
        self.clear_detail(cx);
        self.selected = Some(mr);
        self.remove_source_branch = remove_default;
        self.squash_commits = squash_default;
        if record_history {
            self.state.update(cx, |s, cx| s.record_mr_detail(iid, cx));
        }
        self.sync_summary(cx);
        cx.notify();

        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.detail_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (detail, diffs, discussions, approvals, commits, pipelines, participants, awards) = tokio::join!(
                    client.merge_request_detail(project.id, iid),
                    client.merge_request_diffs(project.id, iid),
                    client.mr_discussions(project.id, iid),
                    client.mr_approvals(project.id, iid),
                    client.mr_commits(project.id, iid),
                    client.mr_pipelines(project.id, iid),
                    client.mr_participants(project.id, iid),
                    client.mr_award_emojis(project.id, iid),
                );
                let head_pid = detail.as_ref().ok().and_then(|m| m.head_pipeline.as_ref().map(|p| p.id))
                    .or_else(|| pipelines.as_ref().ok().and_then(|ps| ps.first().map(|p| p.id)));
                let (jobs, bridges) = match head_pid {
                    Some(pid) => {
                        let (j, b) = tokio::join!(
                            client.pipeline_jobs_all(project.id, pid),
                            client.pipeline_bridges(project.id, pid),
                        );
                        (j.unwrap_or_default(), b.unwrap_or_default())
                    }
                    None => (Vec::new(), Vec::new()),
                };
                (
                    detail.map_err(|e| e.to_string()), diffs.map_err(|e| e.to_string()),
                    discussions.map_err(|e| e.to_string()), approvals.ok().flatten(),
                    commits.unwrap_or_default(), pipelines.unwrap_or_default(),
                    participants.unwrap_or_default(), awards.unwrap_or_default(),
                    jobs, bridges,
                )
            }).await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                let still_sel = view.selected.as_ref().is_some_and(|m| m.iid == iid);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || !still_sel { return; }
                if let Ok((detail_res, diffs_res, disc_res, appr, commits, pipes, parts, awards, jobs, bridges)) = res {
                    if let Ok(full_mr) = detail_res {
                        view.remove_source_branch = full_mr.force_remove_source_branch.or(full_mr.should_remove_source_branch).unwrap_or(false);
                        view.squash_commits = full_mr.squash.unwrap_or(false);
                        let latest_pipe = full_mr.head_pipeline.clone().or_else(|| pipes.first().cloned());
                        if let Some(hp) = latest_pipe
                            && let Some(row) = view.mrs.iter_mut().find(|m| m.iid == iid)
                        {
                            row.head_pipeline = Some(hp);
                        }
                        view.selected = Some(full_mr);
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
                    view.approvals = appr;
                    view.commits = commits;
                    view.mr_pipelines = pipes;
                    view.participants = parts;
                    view.award_emojis = awards;
                    view.head_pipeline_jobs = jobs;
                    view.head_pipeline_bridges = bridges;
                    view.rebuild_stage_cache();
                    view.rebuild_activity_cache();
                    view.sync_summary(cx);
                }
                cx.notify();
            });
        }));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_fixture_data(
        &mut self, mrs: Vec<MergeRequest>, counts: MrListCounts, selected: Option<MergeRequest>,
        discussions: Vec<Discussion>, commits: Vec<RepoCommit>, pipelines: Vec<Pipeline>,
        jobs: Vec<Job>, diffs: Vec<RawDiff>, approvals: Option<MrApprovals>,
        participants: Vec<User>, awards: Vec<AwardEmoji>, cx: &mut Context<Self>,
    ) {
        self.mrs = mrs;
        self.counts = counts;
        self.selected = selected;
        self.discussions = discussions;
        self.commits = commits;
        self.mr_pipelines = pipelines;
        self.head_pipeline_jobs = jobs;
        let parsed = parse_raw_diffs_individually(&diffs);
        self.raw_diffs = diffs;
        self.diff_state.update(cx, |ds, cx| ds.set_files(parsed.files, cx));
        self.approvals = approvals;
        self.participants = participants;
        self.award_emojis = awards;
        self.rebuild_stage_cache();
        self.rebuild_activity_cache();
        self.sync_summary(cx);
        cx.notify();
    }
}
