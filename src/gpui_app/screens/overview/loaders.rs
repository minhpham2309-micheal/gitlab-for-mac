use gpui_kit::Context;

use crate::gitlab::repo_tree_util::merge_last_commits;
use crate::runtime::spawn_tokio;

use super::super::super::repo_cache::{CachedProjectMeta, CachedTree, RepoCache};
use super::ProjectOverviewView;

impl ProjectOverviewView {
    pub(super) fn load_project_meta(&mut self, force: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let origin = client.origin().ascii_serialization();
        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);
        if !force
            && let Some(meta) = cache.as_ref().and_then(|c| c.get_meta(&origin, project.id)) {
                self.detail = Some(meta.detail);
                self.languages = meta.languages;
                self.counts = meta.counts;
                cx.notify();
            }
        self.meta_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (det, langs, counts, branches, tags) = tokio::join!(
                    client.project_detail(project.id),
                    client.project_languages(project.id),
                    client.project_overview_counts(project.id),
                    client.branches(project.id),
                    client.repo_tags(project.id),
                );
                (det.ok(), langs.ok(), counts, branches.ok(), tags.ok())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                if let Ok((det, langs, counts, branches, tags)) = res {
                    if let Some(l) = langs { v.languages = l; }
                    if let Some(b) = branches { v.branches = b; }
                    if let Some(t) = tags { v.tags = t; }
                    if counts.branches.is_some() { v.counts.branches = counts.branches; }
                    if counts.tags.is_some() { v.counts.tags = counts.tags; }
                    if counts.releases.is_some() { v.counts.releases = counts.releases; }
                    if let Some(d) = det {
                        v.detail = Some(d.clone());
                        if let Some(c) = cache.as_ref() {
                            c.insert_meta(cache_gen, &origin, project.id, CachedProjectMeta {
                                detail: d, languages: v.languages.clone(), counts: v.counts.clone(),
                            });
                        }
                    }
                    v.refresh_special_files();
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn load_directory(&mut self, force: bool, cx: &mut Context<Self>) {
        self.ref_epoch = self.ref_epoch.wrapping_add(1);
        let r_ep = self.ref_epoch;
        self.tree_task = None;
        self.hydrate_task = None;
        self.readme_task = None;
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let origin = client.origin().ascii_serialization();
        let (git_ref, path) = (self.current_ref.clone(), self.current_path.clone());
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);

        self.readme_text = None;
        self.readme_entry = None;
        self.commit_expanded = false;
        self.load_latest_commit(cx);

        if !force
            && let Some(hit) = cache.as_ref().and_then(|c| c.get_tree(&origin, project.id, &git_ref, &path)) {
                self.entries = hit.entries;
                self.tree_loading = false;
                self.commits_loading = !hit.commits_hydrated;
                self.server_supports_commits = hit.server_supports_commits;
                self.refresh_special_files();
                self.prefetch_readme(cx);
                cx.notify();
                if !hit.commits_hydrated { self.hydrate_last_commits(r_ep, cx); }
                return;
            }

        self.entries.clear();
        self.tree_loading = true;
        self.commits_loading = true;
        self.server_supports_commits = true;
        cx.notify();
        self.tree_task = Some(cx.spawn(async move |this, cx| {
            let (rc, pc) = (git_ref.clone(), path.clone());
            let res = spawn_tokio(async move {
                client.repo_tree(project.id, &rc, &pc).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.ref_epoch != r_ep { return; }
                v.tree_loading = false;
                match res {
                    Ok(Ok(entries)) => {
                        let empty = entries.is_empty();
                        if let Some(c) = cache.as_ref() {
                            c.insert_tree(cache_gen, &origin, project.id, &git_ref, &path, CachedTree {
                                entries: entries.clone(), commits_hydrated: empty, server_supports_commits: true,
                            });
                        }
                        v.entries = entries;
                        v.refresh_special_files();
                        v.prefetch_readme(cx);
                        if empty { v.commits_loading = false; } else { v.hydrate_last_commits(r_ep, cx); }
                    }
                    Ok(Err(err)) | Err(err) => {
                        v.commits_loading = false;
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                    }
                }
                cx.notify();
            });
        }));
    }

    fn hydrate_last_commits(&mut self, r_ep: u64, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let origin = client.origin().ascii_serialization();
        let (git_ref, path) = (self.current_ref.clone(), self.current_path.clone());
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);
        self.hydrate_task = Some(cx.spawn(async move |this, cx| {
            let (rc, pc) = (git_ref.clone(), path.clone());
            let res = spawn_tokio(async move {
                client.repo_tree_with_last_commit(project.id, &rc, &pc).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.ref_epoch != r_ep { return; }
                v.commits_loading = false;
                if let Ok(Ok(hydrated)) = res {
                    let supported = v.entries.is_empty() || merge_last_commits(&mut v.entries, &hydrated);
                    v.server_supports_commits = supported;
                    if let Some(c) = cache.as_ref() {
                        c.insert_tree(cache_gen, &origin, project.id, &git_ref, &path, CachedTree {
                            entries: v.entries.clone(), commits_hydrated: true, server_supports_commits: supported,
                        });
                    }
                }
                cx.notify();
            });
        }));
    }
}
