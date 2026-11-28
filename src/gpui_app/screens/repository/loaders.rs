use std::sync::Arc;

use gpui_kit::{component::input::Position, Context, Image, ImageFormat, Window};

use crate::gitlab::{RepoBlob, RepoBlobKind};
use crate::runtime::spawn_tokio;

use super::super::super::diff_hl::language_from_path;
use super::super::super::repo_cache::{CachedTree, RepoCache};
use super::viewer_header::is_markdown_path;
use super::RepositoryView;

fn decode_image(ct: &str, bytes: &[u8]) -> Option<Arc<Image>> {
    let fmt = if ct.contains("png") || bytes.starts_with(&[0x89, b'P', b'N', b'G']) { ImageFormat::Png }
    else if ct.contains("jpeg") || ct.contains("jpg") || bytes.starts_with(&[0xFF, 0xD8]) { ImageFormat::Jpeg }
    else if ct.contains("gif") || bytes.starts_with(b"GIF8") { ImageFormat::Gif }
    else if ct.contains("webp") { ImageFormat::Webp }
    else if ct.contains("svg") { ImageFormat::Svg }
    else { return None; };
    Some(Arc::new(Image::from_bytes(fmt, bytes.to_vec())))
}

impl RepositoryView {
    pub(super) fn load_dir(&mut self, dir_path: String, force: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let origin = client.origin().ascii_serialization();
        let git_ref = self.active_ref.clone();
        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);

        if !force
            && let Some(hit) = cache.as_ref().and_then(|c| c.get_tree(&origin, project.id, &git_ref, &dir_path)) {
                self.dirs.insert(dir_path, hit.entries);
                cx.notify();
                return;
            }
        let path_req = dir_path.clone();
        let path_key = dir_path.clone();
        let ref_req = git_ref.clone();
        let task = cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.repo_tree(project.id, &ref_req, &path_req).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                v.dir_tasks.remove(&dir_path);
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.active_ref != git_ref { return; }
                if let Ok(Ok(entries)) = res {
                    if let Some(c) = cache.as_ref() {
                        c.insert_tree(cache_gen, &origin, project.id, &git_ref, &dir_path, CachedTree {
                            entries: entries.clone(), commits_hydrated: false, server_supports_commits: true,
                        });
                    }
                    v.dirs.insert(dir_path, entries);
                    cx.notify();
                }
            });
        });
        self.dir_tasks.insert(path_key, task);
    }

    pub(super) fn open_file_request(
        &mut self,
        path: String,
        git_ref: Option<String>,
        blob_sha: Option<String>,
        line: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(r) = git_ref.filter(|s| !s.is_empty())
            && self.active_ref != r {
                self.active_ref = r.clone();
                self.dirs.clear();
                self.dir_tasks.clear();
                self.expanded_dirs.clear();
                self.state.update(cx, |s, _| s.set_active_ref(r));
            }
        if !self.dirs.contains_key("") { self.load_dir(String::new(), false, cx); }
        self.expand_ancestors(&path, cx);
        self.active_path = Some(path.clone());
        self.active_blob = None;
        self.file_last_commit = None;
        self.commit_task = None;
        self.decoded_image = None;
        self.markdown_preview = is_markdown_path(&path);
        self.blob_error = None;
        self.blob_seq = self.blob_seq.wrapping_add(1);
        let seq = self.blob_seq;

        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { cx.notify(); return; };
        let (h_ep, p_ep, offline) = (st.host_epoch, st.project_epoch, st.offline_mode);
        let origin = client.origin().ascii_serialization();
        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);

        if !offline {
            let (c_meta, ref_meta, path_meta) = (client.clone(), self.active_ref.clone(), path.clone());
            self.commit_task = Some(cx.spawn(async move |this, cx| {
                let res = spawn_tokio(async move {
                    c_meta.repo_commits(project.id, &ref_meta, Some(&path_meta), 1, 1).await.ok()
                }).await;
                let _ = this.update(cx, |v, cx| {
                    let cur = v.state.read(cx);
                    if cur.host_epoch == h_ep && cur.project_epoch == p_ep && v.blob_seq == seq
                        && let Ok(Some(mut list)) = res
                    {
                        v.file_last_commit = list.pop();
                        cx.notify();
                    }
                });
            }));
        }

        if let Some(sha) = blob_sha.as_deref()
            && let Some(blob) = cache.as_ref().and_then(|c| c.get_blob(&origin, project.id, sha)) {
                self.apply_blob(&path, blob, line, window, cx);
                return;
            }
        if offline { cx.notify(); return; }

        self.blob_loading = true;
        cx.notify();
        let active_ref = self.active_ref.clone();
        self.blob_task = Some(cx.spawn_in(window, async move |this, cx| {
            let (p_req, sha_req) = (path.clone(), blob_sha.clone());
            let res = spawn_tokio(async move {
                if let Some(sha) = sha_req.as_deref().filter(|s| !s.is_empty()) {
                    client.repo_blob_raw(project.id, sha, &p_req).await.map_err(|e| e.to_string())
                } else {
                    client.repo_file_raw(project.id, &p_req, &active_ref).await.map_err(|e| e.to_string())
                }
            })
            .await;
            let _ = this.update_in(cx, |v, window, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.blob_seq != seq { return; }
                v.blob_loading = false;
                match res {
                    Ok(Ok(blob)) => {
                        if let Some(sha) = blob.sha.as_deref().or(blob_sha.as_deref())
                            && let Some(c) = cache.as_ref() {
                                c.insert_blob(cache_gen, &origin, project.id, sha, blob.clone());
                            }
                        v.apply_blob(&path, blob, line, window, cx);
                    }
                    Ok(Err(err)) | Err(err) => {
                        v.blob_error = Some(err.clone());
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                        cx.notify();
                    }
                }
            });
        }));
    }

    pub(super) fn apply_blob(
        &mut self, path: &str, blob: RepoBlob, line: Option<u32>,
        window: &mut Window, cx: &mut Context<Self>,
    ) {
        self.blob_loading = false;
        self.blob_error = None;
        self.decoded_image = match &blob.kind {
            RepoBlobKind::Image { bytes, content_type } => decode_image(content_type, bytes),
            _ => None,
        };
        if let RepoBlobKind::Text(ref text) = blob.kind {
            let lang = language_from_path(path);
            let value = text.clone();
            self.editor_state.update(cx, |ed, cx| {
                ed.set_highlighter(lang, cx);
                ed.set_value(value, window, cx);
                if let Some(ln) = line.filter(|&n| n > 0) {
                    let pos = Position::new(ln.saturating_sub(1), 0);
                    ed.unfold_at(pos, cx);
                    ed.set_cursor_position(pos, window, cx);
                }
            });
        }
        self.active_blob = Some(blob);
        cx.notify();
    }

    fn expand_ancestors(&mut self, path: &str, cx: &mut Context<Self>) {
        let mut acc = String::new();
        let parts: Vec<&str> = path.split('/').collect();
        for seg in parts.iter().take(parts.len().saturating_sub(1)) {
            if !acc.is_empty() { acc.push('/'); }
            acc.push_str(seg);
            self.expanded_dirs.insert(acc.clone());
            if !self.dirs.contains_key(&acc) { self.load_dir(acc.clone(), false, cx); }
        }
    }
}
