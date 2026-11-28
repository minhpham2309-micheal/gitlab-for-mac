#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::Deserialize;

use super::time_util::parse_iso_utc_secs;
use super::types::{Release, SortDir, Todo};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ReleaseSource {
    #[serde(default)] pub format: String,
    #[serde(default)] pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ReleaseAssetLink {
    #[serde(default)] pub id: u64,
    #[serde(default)] pub name: String,
    #[serde(default)] pub url: String,
    #[serde(default)] pub direct_asset_url: Option<String>,
    #[serde(default)] pub link_type: Option<String>,
}

impl ReleaseAssetLink {
    pub fn open_url(&self) -> &str {
        self.direct_asset_url.as_deref().filter(|u| !u.is_empty()).unwrap_or(&self.url)
    }

    pub fn group_label(&self) -> &'static str {
        match self.link_type.as_deref().unwrap_or("") {
            "package" => "Packages",
            "image" => "Images",
            "runbook" => "Runbooks",
            _ => "Other",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ReleaseAssets {
    #[serde(default)] pub count: usize,
    #[serde(default)] pub sources: Vec<ReleaseSource>,
    #[serde(default)] pub links: Vec<ReleaseAssetLink>,
}

impl ReleaseAssets {
    pub fn total_count(&self) -> usize {
        if self.count > 0 { self.count } else { self.sources.len() + self.links.len() }
    }

    pub fn grouped_links(&self) -> Vec<(&'static str, Vec<&ReleaseAssetLink>)> {
        let mut out = Vec::new();
        for group in ["Packages", "Images", "Runbooks", "Other"] {
            let items: Vec<&ReleaseAssetLink> = self.links.iter().filter(|l| l.group_label() == group).collect();
            if !items.is_empty() { out.push((group, items)); }
        }
        out
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ReleaseEvidence {
    #[serde(default)] pub sha: Option<String>,
    #[serde(default)] pub filepath: Option<String>,
    #[serde(default)] pub collected_at: Option<String>,
}

impl ReleaseEvidence {
    pub fn display_name(&self) -> String {
        self.filepath.as_deref().and_then(|p| p.rsplit('/').next()).filter(|s| !s.is_empty()).unwrap_or("evidence.json").to_string()
    }

    pub fn short_sha(&self) -> Option<&str> {
        self.sha.as_deref().filter(|s| !s.is_empty()).map(|s| s.get(..8).unwrap_or(s))
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ReleaseCommit {
    #[serde(default)] pub id: String,
    #[serde(default)] pub short_id: String,
    #[serde(default)] pub title: String,
    #[serde(default)] pub web_url: Option<String>,
}

impl ReleaseCommit {
    pub fn display_short_id(&self) -> &str {
        if !self.short_id.is_empty() { &self.short_id } else { self.id.get(..8).unwrap_or(&self.id) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReleaseSortField { #[default] ReleasedAt, CreatedAt }

impl ReleaseSortField {
    pub const ALL: [Self; 2] = [Self::ReleasedAt, Self::CreatedAt];
    pub fn label(self) -> &'static str {
        match self { Self::ReleasedAt => "Released date", Self::CreatedAt => "Created date" }
    }
}

pub fn sort_releases(releases: &[Release], field: ReleaseSortField, dir: SortDir) -> Vec<Release> {
    let mut out = releases.to_vec();
    let pick = |r: &Release| -> i64 {
        let iso = match field {
            ReleaseSortField::ReleasedAt => r.released_at.as_deref().or(r.created_at.as_deref()),
            ReleaseSortField::CreatedAt => r.created_at.as_deref().or(r.released_at.as_deref()),
        };
        iso.and_then(parse_iso_utc_secs).unwrap_or(i64::MIN)
    };
    out.sort_by(|a, b| {
        let ord = pick(a).cmp(&pick(b)).then_with(|| a.tag_name.cmp(&b.tag_name));
        match dir { SortDir::Asc => ord, SortDir::Desc => ord.reverse() }
    });
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TodoTab { #[default] Pending, Snoozed, Done }

impl TodoTab {
    pub const ALL: [Self; 3] = [Self::Pending, Self::Snoozed, Self::Done];
    pub fn label(self) -> &'static str {
        match self { Self::Pending => "To Do", Self::Snoozed => "Snoozed", Self::Done => "Done" }
    }
}

pub fn todo_action_phrase(todo: &Todo) -> String {
    let who = todo.author.as_ref().map(|u| u.name.as_str()).filter(|s| !s.is_empty()).unwrap_or("Someone");
    match todo.action_name.as_str() {
        "assigned" => format!("{who} assigned you."),
        "mentioned" | "directly_addressed" => format!("{who} mentioned you."),
        "review_requested" => format!("{who} requested a review."),
        "approval_required" => format!("{who} requested your approval."),
        "build_failed" => "The pipeline failed.".into(),
        "unmergeable" => "Could not merge.".into(),
        "marked" => "You added a to-do item.".into(),
        "member_access_requested" => format!("{who} requested access."),
        _ if !todo.body.trim().is_empty() => todo.body.trim().to_string(),
        other => other.replace('_', " "),
    }
}

pub fn filter_todos(todos: &[Todo], query: &str) -> Vec<Todo> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() { return todos.to_vec(); }
    todos.iter().filter(|t| {
        t.body.to_ascii_lowercase().contains(&q)
            || t.action_name.to_ascii_lowercase().contains(&q)
            || t.project.as_ref().is_some_and(|p| p.path_with_namespace.to_ascii_lowercase().contains(&q))
            || t.target.as_ref().and_then(|x| x.title.as_deref()).is_some_and(|s| s.to_ascii_lowercase().contains(&q))
            || t.author.as_ref().is_some_and(|u| u.name.to_ascii_lowercase().contains(&q))
    }).cloned().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffTreeRow {
    Dir { depth: usize, name: String, path: String },
    File { depth: usize, name: String, full_path: String, file_ix: usize, additions: usize, deletions: usize, badge: &'static str },
}

#[derive(Default)]
struct DirNode {
    dirs: BTreeMap<String, DirNode>,
    files: Vec<(String, String, usize, usize, usize, &'static str)>,
}

fn flatten_dir(node: &DirNode, prefix: &str, depth: usize, out: &mut Vec<DiffTreeRow>) {
    for (seg, child) in &node.dirs {
        let (mut name, mut full, mut cur) = (seg.clone(), if prefix.is_empty() { seg.clone() } else { format!("{prefix}/{seg}") }, child);
        while cur.files.is_empty() && cur.dirs.len() == 1 {
            let (next_seg, next_child) = cur.dirs.iter().next().unwrap();
            name.push('/');
            name.push_str(next_seg);
            full.push('/');
            full.push_str(next_seg);
            cur = next_child;
        }
        out.push(DiffTreeRow::Dir { depth, name, path: full.clone() });
        flatten_dir(cur, &full, depth + 1, out);
    }
    for (name, full_path, file_ix, additions, deletions, badge) in &node.files {
        out.push(DiffTreeRow::File {
            depth, name: name.clone(), full_path: full_path.clone(),
            file_ix: *file_ix, additions: *additions, deletions: *deletions, badge,
        });
    }
}

pub fn build_diff_tree_rows(files: &[(String, usize, usize, &'static str)], filter: &str) -> Vec<DiffTreeRow> {
    let q = filter.trim().to_ascii_lowercase();
    let mut root = DirNode::default();
    for (ix, (path, add, del, badge)) in files.iter().enumerate() {
        if !q.is_empty() && !path.to_ascii_lowercase().contains(&q) { continue; }
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let Some((file_name, dirs)) = parts.split_last() else { continue; };
        let mut node = &mut root;
        for d in dirs { node = node.dirs.entry((*d).to_string()).or_default(); }
        node.files.push(((*file_name).to_string(), path.clone(), ix, *add, *del, *badge));
    }
    let mut out = Vec::new();
    flatten_dir(&root, "", 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compacts_single_child_dirs_in_diff_tree_and_filters() {
        let files = vec![
            ("app/src/main/resources/a.yml".into(), 2, 1, "M"),
            ("app/src/main/resources/b.yml".into(), 5, 0, "A"),
            ("app/src/test/Test.java".into(), 10, 0, "A"),
        ];
        let rows = build_diff_tree_rows(&files, "");
        assert!(matches!(&rows[0], DiffTreeRow::Dir { depth: 0, name, .. } if name == "app/src"));
        assert!(matches!(&rows[1], DiffTreeRow::Dir { depth: 1, name, .. } if name == "main/resources"));
        assert!(matches!(&rows[2], DiffTreeRow::File { depth: 2, name, file_ix: 0, .. } if name == "a.yml"));
        assert_eq!(build_diff_tree_rows(&files, "Test.java").len(), 2);
    }

    #[test]
    fn groups_release_assets_and_formats_todo_phrases() {
        let assets = ReleaseAssets {
            count: 3,
            sources: vec![ReleaseSource { format: "zip".into(), url: "https://gitlab.example.com/a.zip".into() }],
            links: vec![
                ReleaseAssetLink { name: "pkg.tar.gz".into(), url: "https://gitlab.example.com/p".into(), link_type: Some("package".into()), ..Default::default() },
                ReleaseAssetLink { name: "guide.md".into(), url: "https://gitlab.example.com/g".into(), link_type: Some("other".into()), ..Default::default() },
            ],
        };
        assert_eq!(assets.total_count(), 3);
        let groups = assets.grouped_links();
        assert_eq!((groups.len(), groups[0].0, groups[1].0), (2, "Packages", "Other"));
        assert_eq!(todo_action_phrase(&Todo { action_name: "unmergeable".into(), ..Default::default() }), "Could not merge.");
    }
}
