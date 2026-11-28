use std::collections::HashMap;

use gpui_kit::{
    component::{
        diff::{DiffMode, DiffState},
        input::{InputEvent, InputState},
        resizable::{h_resizable, resizable_panel},
    },
    px, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Subscription,
    Task, Window,
};

use crate::gitlab::{tags_by_commit_sha, Branch, Pipeline, RawDiff, RepoCommit, RepoTag};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::diff_hl::{new_diff_state, parse_raw_diffs_individually};
use super::super::super::ref_switcher::RefSwitcherState;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};
use super::commit_diff::render_commit_diff_pane;
use super::commits_list::{render_commits_split_list, render_commits_timeline_view};

pub struct CommitsView {
    pub(super) state: Entity<AppState>,
    pub(super) ref_switcher: RefSwitcherState,
    pub(super) filter_input: Entity<InputState>,
    pub(super) diff_state: Entity<DiffState>,
    pub(super) git_ref: String,
    pub(super) path_filter: Option<String>,
    pub(super) commits: Vec<RepoCommit>,
    pub(super) branches: Vec<Branch>,
    pub(super) tags: Vec<RepoTag>,
    pub(super) sha_pipelines: HashMap<String, (u64, String)>,
    pub(super) sha_tags: HashMap<String, Vec<String>>,
    pub(super) selected_sha: Option<String>,
    pub(super) split_mode: bool,
    pub(super) page: usize,
    pub(super) loading: bool,
    pub(super) diff_loading: bool,
    pub(super) diff_error: Option<String>,
    pub(super) fallback_notices: Vec<(String, String)>,
    pub(super) has_truncated: bool,
    pub(super) raw_diff_count: usize,
    list_task: Option<Task<()>>,
    diff_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl CommitsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = if state.read(cx).prefs.default_diff_split { DiffMode::Split } else { DiffMode::Unified };
        let ref_switcher = RefSwitcherState::new(window, cx);
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search or filter results…"));
        let diff_state = cx.new(|cx| new_diff_state(Vec::new(), mode, cx));
        let sub_ref = cx.subscribe_in(&ref_switcher.search_input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_in = cx.subscribe_in(&filter_input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.ref_switcher.reset(window, cx);
                this.reset(cx);
                if this.state.read(cx).screen == Screen::Commits { this.load_page(1, false, cx); }
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Commits => {
                if this.commits.is_empty() && !this.loading { this.load_page(1, false, cx); }
            }
            AppEvent::Command(ScreenCommand::OpenCommitHistory { git_ref, path }) => {
                if let Some(r) = git_ref.clone().filter(|s| !s.is_empty()) { this.git_ref = r; }
                this.path_filter = path.clone().filter(|s| !s.is_empty());
                (this.selected_sha, this.split_mode) = (None, false);
                this.clear_diff(cx);
                this.load_page(1, false, cx);
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Commits => { this.load_page(1, false, cx); }
            AppEvent::PrefsChanged => {
                let m = if this.state.read(cx).prefs.default_diff_split { DiffMode::Split } else { DiffMode::Unified };
                this.diff_state.update(cx, |ds, cx| ds.set_mode(m, cx));
            }
            _ => {}
        });
        Self {
            state, ref_switcher, filter_input, diff_state, git_ref: "main".into(), path_filter: None,
            commits: Vec::new(), branches: Vec::new(), tags: Vec::new(), sha_pipelines: HashMap::new(),
            sha_tags: HashMap::new(), selected_sha: None, split_mode: false, page: 1, loading: false,
            diff_loading: false, diff_error: None, fallback_notices: Vec::new(), has_truncated: false,
            raw_diff_count: 0, list_task: None, diff_task: None, _subs: vec![sub_in, sub_st, sub_ref],
        }
    }

    pub(super) fn selected_commit(&self) -> Option<&RepoCommit> {
        self.selected_sha.as_deref().and_then(|sha| self.commits.iter().find(|c| c.id == sha))
    }

    pub fn diff_file_count(&self, cx: &gpui_kit::App) -> usize { self.diff_state.read(cx).files().len() }
    pub fn is_split_mode(&self) -> bool { self.split_mode }

    pub fn set_fixture_data(
        &mut self, commits: Vec<RepoCommit>, selected_sha: Option<String>, raw_diffs: Vec<RawDiff>,
        diff_error: Option<String>, cx: &mut Context<Self>,
    ) {
        self.split_mode = selected_sha.is_some();
        (self.commits, self.selected_sha, self.loading, self.diff_loading, self.diff_error) =
            (commits, selected_sha, false, false, diff_error);
        self.apply_raw_diffs(&raw_diffs, cx);
        cx.notify();
    }

    pub fn set_fixture_enrichments(
        &mut self, branches: Vec<Branch>, tags: Vec<RepoTag>, pipelines: Vec<Pipeline>, cx: &mut Context<Self>,
    ) {
        self.branches = branches;
        self.sha_tags = tags_by_commit_sha(&tags);
        self.tags = tags;
        self.sha_pipelines.clear();
        for p in pipelines { self.sha_pipelines.entry(p.sha).or_insert((p.id, p.status)); }
        cx.notify();
    }

    pub(super) fn clear_diff(&mut self, cx: &mut Context<Self>) {
        (self.diff_task, self.diff_loading, self.diff_error, self.has_truncated, self.raw_diff_count) =
            (None, false, None, false, 0);
        self.fallback_notices.clear();
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
    }

    fn apply_raw_diffs(&mut self, raw: &[RawDiff], cx: &mut Context<Self>) {
        let parsed = parse_raw_diffs_individually(raw);
        (self.raw_diff_count, self.fallback_notices, self.has_truncated) =
            (raw.len(), parsed.fallback_notices, parsed.has_truncated);
        self.diff_state.update(cx, |ds, cx| ds.set_files(parsed.files, cx));
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        (self.list_task, self.path_filter, self.selected_sha, self.split_mode, self.page, self.loading) =
            (None, None, None, false, 1, false);
        self.git_ref = self.state.read(cx).project.as_ref()
            .and_then(|p| p.default_branch.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| "main".into());
        self.commits.clear(); self.branches.clear(); self.tags.clear(); self.sha_pipelines.clear(); self.sha_tags.clear();
        self.clear_diff(cx);
        cx.notify();
    }

    pub(super) fn switch_ref(&mut self, next_ref: String, cx: &mut Context<Self>) {
        if next_ref.is_empty() || self.git_ref == next_ref { return; }
        (self.git_ref, self.selected_sha) = (next_ref, None);
        self.clear_diff(cx); self.load_page(1, false, cx);
    }

    pub(super) fn close_diff_pane(&mut self, cx: &mut Context<Self>) {
        (self.split_mode, self.selected_sha) = (false, None);
        self.clear_diff(cx); cx.notify();
    }

    pub(super) fn toggle_split_mode(&mut self, cx: &mut Context<Self>) {
        self.split_mode = !self.split_mode;
        if self.split_mode && self.selected_sha.is_none()
            && let Some(first) = self.commits.first().map(|c| c.id.clone())
        { self.select_commit(first, cx); return; }
        cx.notify();
    }

    pub(super) fn load_page(&mut self, page: usize, append: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let ((h_ep, p_ep), (git_ref, path)) = ((st.host_epoch, st.project_epoch), (self.git_ref.clone(), self.path_filter.clone()));
        (self.loading, _) = (true, cx.notify());
        let fetch_meta = page == 1 && !append;
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let commits = client.repo_commits(project.id, &git_ref, path.as_deref(), page, 40).await.map_err(|e| e.to_string())?;
                let meta = if fetch_meta {
                    let (p, t, b) = tokio::join!(client.pipelines(project.id), client.repo_tags(project.id), client.branches(project.id));
                    Some((p.unwrap_or_default(), t.unwrap_or_default(), b.unwrap_or_default()))
                } else { None };
                Ok::<_, String>((commits, meta))
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok((items, meta))) => {
                        if !items.is_empty() || !append { v.page = page; }
                        if append { v.commits.extend(items); } else { v.commits = items; }
                        if let Some((pipes, tags, branches)) = meta {
                            (v.branches, v.sha_tags, v.tags) = (branches, tags_by_commit_sha(&tags), tags);
                            v.sha_pipelines.clear();
                            for p in pipes { v.sha_pipelines.entry(p.sha).or_insert((p.id, p.status)); }
                        }
                        if v.split_mode && v.selected_sha.as_deref().is_none_or(|sha| !v.commits.iter().any(|c| c.id == sha)) {
                            if let Some(first) = v.commits.first().map(|c| c.id.clone()) { v.select_commit(first, cx); }
                            else { v.selected_sha = None; v.clear_diff(cx); }
                        }
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn select_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        (self.split_mode, self.selected_sha) = (true, Some(sha.clone()));
        self.clear_diff(cx);
        let st = self.state.read(cx);
        if st.offline_mode { cx.notify(); return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        (self.diff_loading, _) = (true, cx.notify());
        self.diff_task = Some(cx.spawn(async move |this, cx| {
            let sha_req = sha.clone();
            let res = spawn_tokio(async move { client.repo_commit_diff(project.id, &sha_req).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.selected_sha.as_deref() != Some(sha.as_str()) { return; }
                v.diff_loading = false;
                match res {
                    Ok(Ok(raw)) => v.apply_raw_diffs(&raw, cx),
                    Ok(Err(err)) | Err(err) => {
                        v.diff_error = Some(err.clone());
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                    }
                }
                cx.notify();
            });
        }));
    }
}

impl Render for CommitsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.split_mode {
            h_resizable("commits-split")
                .child(resizable_panel().size(px(390.)).size_range(px(280.)..px(560.)).child(render_commits_split_list(self, cx)))
                .child(resizable_panel().child(render_commit_diff_pane(self, cx)))
                .into_any_element()
        } else {
            render_commits_timeline_view(self, cx).into_any_element()
        }
    }
}
