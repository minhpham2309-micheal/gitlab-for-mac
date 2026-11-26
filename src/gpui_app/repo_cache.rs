#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use gpui_kit::{App, Global};

use crate::gitlab::{
    LanguageSlice, ProjectDetail, ProjectOverviewCounts, RepoBlob, RepoTreeEntry,
};

const MAX_BLOBS: usize = 256;
const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;
const MAX_TREES: usize = 128;
const MAX_INDEXES: usize = 16;
const MAX_DETAILS: usize = 32;

type BlobKey = (String, u64, String);
type TreeKey = (String, u64, String, String);
type IndexKey = (String, u64, String);
type DetailKey = (String, u64);

#[derive(Clone)]
pub struct CachedTree {
    pub entries: Vec<RepoTreeEntry>,
    pub commits_hydrated: bool,
    pub server_supports_commits: bool,
}

#[derive(Clone)]
pub struct CachedProjectMeta {
    pub detail: ProjectDetail,
    pub languages: Vec<LanguageSlice>,
    pub counts: ProjectOverviewCounts,
}

#[derive(Default)]
struct RepoCacheInner {
    generation: u64,
    blobs: HashMap<BlobKey, (RepoBlob, usize)>,
    blob_order: VecDeque<BlobKey>,
    blob_bytes: usize,
    trees: HashMap<TreeKey, CachedTree>,
    tree_order: VecDeque<TreeKey>,
    indexes: HashMap<IndexKey, (Vec<RepoTreeEntry>, bool)>,
    index_order: VecDeque<IndexKey>,
    details: HashMap<DetailKey, CachedProjectMeta>,
    detail_order: VecDeque<DetailKey>,
}

#[derive(Clone)]
pub struct RepoCache {
    inner: Arc<RwLock<RepoCacheInner>>,
}

impl Global for RepoCache {}

impl Default for RepoCache {
    fn default() -> Self { Self::new() }
}

fn touch<K: PartialEq>(order: &mut VecDeque<K>, key: K) {
    if let Some(pos) = order.iter().position(|k| k == &key) { order.remove(pos); }
    order.push_back(key);
}

fn insert_bounded<K: Eq + std::hash::Hash + Clone, V>(
    map: &mut HashMap<K, V>, order: &mut VecDeque<K>, key: K, val: V, max: usize,
) {
    map.insert(key.clone(), val);
    touch(order, key);
    while map.len() > max {
        if let Some(old) = order.pop_front() { map.remove(&old); } else { break; }
    }
}

impl RepoCache {
    pub fn new() -> Self {
        let inner = RepoCacheInner { generation: 1, ..Default::default() };
        Self { inner: Arc::new(RwLock::new(inner)) }
    }

    pub fn init(cx: &mut App) {
        if cx.try_global::<Self>().is_none() { cx.set_global(Self::new()); }
    }

    pub fn generation(&self) -> u64 {
        self.inner.read().map(|g| g.generation).unwrap_or(0)
    }

    pub fn clear(&self) {
        if let Ok(mut g) = self.inner.write() {
            let next_gen = g.generation.wrapping_add(1);
            *g = RepoCacheInner { generation: next_gen, ..RepoCacheInner::default() };
        }
    }

    pub fn get_blob(&self, origin: &str, project_id: u64, sha: &str) -> Option<RepoBlob> {
        let key = (origin.to_string(), project_id, sha.to_string());
        let mut g = self.inner.write().ok()?;
        let blob = g.blobs.get(&key).map(|(b, _)| b.clone())?;
        touch(&mut g.blob_order, key);
        Some(blob)
    }

    pub fn insert_blob(&self, expected_gen: u64, origin: &str, project_id: u64, sha: &str, blob: RepoBlob) {
        let Ok(mut g) = self.inner.write() else { return; };
        if g.generation != expected_gen { return; }
        let key = (origin.to_string(), project_id, sha.to_string());
        let sz = blob.approx_bytes();
        if let Some((_, old)) = g.blobs.insert(key.clone(), (blob, sz)) {
            g.blob_bytes = g.blob_bytes.saturating_sub(old);
        }
        touch(&mut g.blob_order, key);
        g.blob_bytes = g.blob_bytes.saturating_add(sz);
        while g.blobs.len() > MAX_BLOBS || g.blob_bytes > MAX_BLOB_BYTES {
            let Some(oldest) = g.blob_order.pop_front() else { break; };
            if let Some((_, evicted)) = g.blobs.remove(&oldest) {
                g.blob_bytes = g.blob_bytes.saturating_sub(evicted);
            }
        }
    }

    pub fn get_tree(&self, origin: &str, project_id: u64, git_ref: &str, path: &str) -> Option<CachedTree> {
        let key = (origin.to_string(), project_id, git_ref.to_string(), path.to_string());
        let mut g = self.inner.write().ok()?;
        let val = g.trees.get(&key).cloned()?;
        touch(&mut g.tree_order, key);
        Some(val)
    }

    pub fn insert_tree(&self, expected_gen: u64, origin: &str, project_id: u64, git_ref: &str, path: &str, tree: CachedTree) {
        let Ok(mut g) = self.inner.write() else { return; };
        if g.generation != expected_gen { return; }
        let g = &mut *g;
        let key = (origin.to_string(), project_id, git_ref.to_string(), path.to_string());
        insert_bounded(&mut g.trees, &mut g.tree_order, key, tree, MAX_TREES);
    }

    pub fn get_index(&self, origin: &str, project_id: u64, git_ref: &str) -> Option<(Vec<RepoTreeEntry>, bool)> {
        let key = (origin.to_string(), project_id, git_ref.to_string());
        let mut g = self.inner.write().ok()?;
        let val = g.indexes.get(&key).cloned()?;
        touch(&mut g.index_order, key);
        Some(val)
    }

    pub fn insert_index(&self, expected_gen: u64, origin: &str, project_id: u64, git_ref: &str, items: Vec<RepoTreeEntry>, truncated: bool) {
        let Ok(mut g) = self.inner.write() else { return; };
        if g.generation != expected_gen { return; }
        let g = &mut *g;
        let key = (origin.to_string(), project_id, git_ref.to_string());
        insert_bounded(&mut g.indexes, &mut g.index_order, key, (items, truncated), MAX_INDEXES);
    }

    pub fn get_meta(&self, origin: &str, project_id: u64) -> Option<CachedProjectMeta> {
        let key = (origin.to_string(), project_id);
        let mut g = self.inner.write().ok()?;
        let val = g.details.get(&key).cloned()?;
        touch(&mut g.detail_order, key);
        Some(val)
    }

    pub fn insert_meta(&self, expected_gen: u64, origin: &str, project_id: u64, meta: CachedProjectMeta) {
        let Ok(mut g) = self.inner.write() else { return; };
        if g.generation != expected_gen { return; }
        let g = &mut *g;
        let key = (origin.to_string(), project_id);
        insert_bounded(&mut g.details, &mut g.detail_order, key, meta, MAX_DETAILS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitlab::RepoBlobKind;

    #[test]
    fn blob_cache_hits_and_clears_generation_on_host_switch() {
        let cache = RepoCache::new();
        let gen1 = cache.generation();
        let blob = RepoBlob { sha: Some("deadbeef".into()), size_bytes: 5, kind: RepoBlobKind::Text("hello".into()) };
        cache.insert_blob(gen1, "https://gl.test", 7, "deadbeef", blob.clone());
        assert!(cache.get_blob("https://gl.test", 7, "deadbeef").is_some());

        cache.clear();
        assert!(cache.get_blob("https://gl.test", 7, "deadbeef").is_none());
        cache.insert_blob(gen1, "https://gl.test", 7, "deadbeef", blob);
        assert!(cache.get_blob("https://gl.test", 7, "deadbeef").is_none());
    }
}
