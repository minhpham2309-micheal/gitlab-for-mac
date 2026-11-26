use gpui_kit::Context;

use crate::gitlab::repo_tree_util::detect_special_files;
use crate::gitlab::{ProjectOverviewCounts, RepoBlobKind, RepoTreeEntry, SpecialRepoFiles};
use crate::runtime::spawn_tokio;

use super::super::super::repo_cache::RepoCache;
use super::ProjectOverviewView;

impl ProjectOverviewView {
    pub(super) fn reset_for_project(&mut self, cx: &mut Context<Self>) {
        self.meta_task = None;
        self.tree_task = None;
        self.hydrate_task = None;
        self.commit_task = None;
        self.readme_task = None;
        self.star_task = None;
        self.current_ref = self
            .state
            .read(cx)
            .project
            .as_ref()
            .and_then(|p| p.default_branch.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main".into());
        self.current_path.clear();
        self.detail = None;
        self.languages.clear();
        self.counts = ProjectOverviewCounts::default();
        self.branches.clear();
        self.tags.clear();
        self.entries.clear();
        self.root_special = SpecialRepoFiles::default();
        self.latest_commit = None;
        self.commit_loading = false;
        self.readme_entry = None;
        self.readme_text = None;
        self.commit_expanded = false;
        self.starred = false;
        self.tree_loading = false;
        self.commits_loading = false;
        self.server_supports_commits = true;
        cx.notify();
    }

    pub(super) fn load_latest_commit(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep, r_ep) = (st.host_epoch, st.project_epoch, self.ref_epoch);
        let (git_ref, path) = (self.current_ref.clone(), self.current_path.clone());
        self.latest_commit = None;
        self.commit_loading = true;
        self.commit_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let path_opt = (!path.is_empty()).then_some(path.as_str());
                let list = client.repo_commits(project.id, &git_ref, path_opt, 1, 1).await.ok()?;
                let head = list.into_iter().next()?;
                Some(client.repo_commit_detail(project.id, &head.id).await.unwrap_or(head))
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.ref_epoch != r_ep { return; }
                v.commit_loading = false;
                v.latest_commit = res.ok().flatten();
                cx.notify();
            });
        }));
    }

    pub(super) fn prefetch_readme(&mut self, cx: &mut Context<Self>) {
        let special = detect_special_files(
            &self.entries,
            self.detail.as_ref().and_then(|d| d.ci_config_path.as_deref()),
        );
        let Some(readme) = special.readme else { return; };
        self.readme_entry = Some(readme.clone());
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let origin = client.origin().ascii_serialization();
        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);
        if let Some(blob) = cache.as_ref().and_then(|c| c.get_blob(&origin, project.id, &readme.id)) {
            if let RepoBlobKind::Text(txt) = blob.kind { self.readme_text = Some(txt); }
            return;
        }
        let (h_ep, p_ep, r_ep) = (st.host_epoch, st.project_epoch, self.ref_epoch);
        self.readme_task = Some(cx.spawn(async move |this, cx| {
            let (sha, path) = (readme.id.clone(), readme.path.clone());
            let res = spawn_tokio(async move { client.repo_blob_raw(project.id, &sha, &path).await.ok() }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.ref_epoch != r_ep { return; }
                if let Ok(Some(blob)) = res {
                    if let Some(c) = cache.as_ref() { c.insert_blob(cache_gen, &origin, project.id, &readme.id, blob.clone()); }
                    if let RepoBlobKind::Text(txt) = blob.kind { v.readme_text = Some(txt); }
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn refresh_special_files(&mut self) {
        if self.current_path.is_empty() {
            self.root_special = detect_special_files(
                &self.entries,
                self.detail.as_ref().and_then(|d| d.ci_config_path.as_deref()),
            );
        }
    }

    pub fn open_file(&self, entry: &RepoTreeEntry, cx: &mut Context<Self>) {
        let Some(pid) = self.state.read(cx).project.as_ref().map(|p| p.id) else { return; };
        let (git_ref, path, sha) = (Some(self.current_ref.clone()), entry.path.clone(), Some(entry.id.clone()));
        self.state.update(cx, |s, cx| s.open_repo_file(pid, git_ref, path, sha, None, cx));
    }

    pub fn toggle_star(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let prev_stars = self.detail.as_ref().map(|d| d.star_count);
        let want_star = !self.starred;
        self.starred = want_star;
        if let Some(d) = self.detail.as_mut() {
            d.star_count = if want_star { d.star_count.saturating_add(1) } else { d.star_count.saturating_sub(1) };
        }
        cx.notify();
        self.star_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                if want_star { client.star_project(project.id).await } else { client.unstar_project(project.id).await }
                    .map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(detail)) => v.detail = Some(detail),
                    Ok(Err(err)) | Err(err) => {
                        v.starred = !want_star;
                        if let (Some(d), Some(old)) = (v.detail.as_mut(), prev_stars) { d.star_count = old; }
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                    }
                }
                cx.notify();
            });
        }));
    }
}
