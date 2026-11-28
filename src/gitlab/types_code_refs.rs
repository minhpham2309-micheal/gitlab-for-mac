#![allow(dead_code)]

use std::collections::HashMap;

use super::time_util::{format_short_date, iso_date_key, parse_iso_utc_secs};
use super::types::{Branch, MergeRequest, Pipeline, SortDir};
use super::types_repo::{RepoCommit, RepoTag};

/// GitLab considers non-default branches with no commits in 3 months (90 days) stale.
pub const STALE_BRANCH_SECS: i64 = 90 * 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchScope {
    #[default]
    Overview,
    Active,
    Stale,
    All,
}

impl BranchScope {
    pub const ALL: [Self; 4] = [Self::Overview, Self::Active, Self::Stale, Self::All];

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Active => "Active",
            Self::Stale => "Stale",
            Self::All => "All",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagSortField {
    #[default]
    Updated,
    Name,
}

impl TagSortField {
    pub const ALL: [Self; 2] = [Self::Updated, Self::Name];

    pub fn label(self) -> &'static str {
        match self {
            Self::Updated => "Updated date",
            Self::Name => "Name",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommitDateGroup {
    pub date_key: String,
    pub label: String,
    pub items: Vec<(usize, RepoCommit)>,
}

pub fn group_commits_by_date(commits: &[RepoCommit]) -> Vec<CommitDateGroup> {
    let mut groups: Vec<CommitDateGroup> = Vec::new();
    for (ix, commit) in commits.iter().enumerate() {
        let ts = commit.timestamp().unwrap_or("");
        let (key, label) = match iso_date_key(ts) {
            Some(k) => (k.to_string(), format_short_date(ts)),
            None => ("unknown".to_string(), "Unknown date".to_string()),
        };
        if let Some(last) = groups.last_mut().filter(|g| g.date_key == key) {
            last.items.push((ix, commit.clone()));
        } else {
            groups.push(CommitDateGroup { date_key: key, label, items: vec![(ix, commit.clone())] });
        }
    }
    groups
}

pub fn tags_by_commit_sha(tags: &[RepoTag]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for tag in tags {
        let sha = tag
            .commit.as_ref().map(|c| c.id.as_str()).filter(|s| !s.is_empty())
            .or(tag.target.as_deref().filter(|s| !s.is_empty()));
        if let Some(sha) = sha {
            map.entry(sha.to_string()).or_default().push(tag.name.clone());
        }
    }
    map
}

pub fn is_branch_stale(branch: &Branch, now_secs: i64) -> bool {
    if branch.default { return false; }
    let Some(ts) = branch.commit.as_ref().and_then(|c| c.timestamp()).and_then(parse_iso_utc_secs) else {
        return false;
    };
    (now_secs - ts) >= STALE_BRANCH_SECS
}

fn branch_timestamp_secs(branch: &Branch) -> i64 {
    branch.commit.as_ref().and_then(|c| c.timestamp()).and_then(parse_iso_utc_secs).unwrap_or(i64::MIN)
}

fn sort_branches_recent(list: &mut [Branch]) {
    list.sort_by(|a, b| {
        b.default.cmp(&a.default)
            .then_with(|| branch_timestamp_secs(b).cmp(&branch_timestamp_secs(a)))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
}

pub fn split_active_stale_branches(branches: &[Branch], query: &str, now_secs: i64) -> (Vec<Branch>, Vec<Branch>) {
    let q = query.trim().to_ascii_lowercase();
    let (mut active, mut stale) = (Vec::new(), Vec::new());
    for br in branches {
        let matched = q.is_empty()
            || br.name.to_ascii_lowercase().contains(&q)
            || br.commit.as_ref().is_some_and(|c| {
                c.title.to_ascii_lowercase().contains(&q) || c.short_id.to_ascii_lowercase().contains(&q)
            });
        if !matched { continue; }
        if is_branch_stale(br, now_secs) { stale.push(br.clone()); } else { active.push(br.clone()); }
    }
    sort_branches_recent(&mut active);
    sort_branches_recent(&mut stale);
    (active, stale)
}

pub fn match_branch_mr<'a>(branch_name: &str, mrs: &'a [MergeRequest]) -> Option<&'a MergeRequest> {
    let mut best: Option<&'a MergeRequest> = None;
    for mr in mrs.iter().filter(|m| m.source_branch == branch_name) {
        let prio = |s: &str| match s { "opened" | "open" | "locked" => 2, "merged" => 1, _ => 0 };
        let replace = best.is_none_or(|cur| {
            let (p_new, p_cur) = (prio(&mr.state), prio(&cur.state));
            p_new > p_cur || (p_new == p_cur && mr.iid > cur.iid)
        });
        if replace { best = Some(mr); }
    }
    best
}

pub fn match_ref_pipeline<'a>(ref_name: &str, sha: Option<&str>, pipelines: &'a [Pipeline]) -> Option<&'a Pipeline> {
    if let Some(sha) = sha.filter(|s| !s.is_empty())
        && let Some(p) = pipelines.iter().find(|p| p.sha == sha)
    {
        return Some(p);
    }
    pipelines.iter().find(|p| p.git_ref == ref_name)
}

pub fn filter_and_sort_tags(tags: &[RepoTag], query: &str, sort: TagSortField, dir: SortDir) -> Vec<RepoTag> {
    let q = query.trim().to_ascii_lowercase();
    let mut out: Vec<RepoTag> = tags.iter().filter(|t| {
        q.is_empty()
            || t.name.to_ascii_lowercase().contains(&q)
            || t.message.as_deref().is_some_and(|m| m.to_ascii_lowercase().contains(&q))
            || t.commit.as_ref().is_some_and(|c| {
                c.title.to_ascii_lowercase().contains(&q) || c.short_id.to_ascii_lowercase().contains(&q)
            })
            || t.release.as_ref().and_then(|r| r.description.as_deref()).is_some_and(|d| d.to_ascii_lowercase().contains(&q))
    }).cloned().collect();
    out.sort_by(|a, b| {
        let ord = match sort {
            TagSortField::Updated => {
                let ta = a.timestamp().and_then(parse_iso_utc_secs).unwrap_or(i64::MIN);
                let tb = b.timestamp().and_then(parse_iso_utc_secs).unwrap_or(i64::MIN);
                ta.cmp(&tb).then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
            }
            TagSortField::Name => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
        };
        match dir { SortDir::Asc => ord, SortDir::Desc => ord.reverse() }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitlab::{BranchCommit, RepoTagCommit};

    #[test]
    fn groups_commits_by_iso_day_preserving_indices() {
        let c = |id: &str, date: &str| RepoCommit {
            id: id.into(), short_id: id.into(), title: id.into(), message: id.into(),
            author_name: "alice".into(), author_email: None,
            authored_date: Some(date.into()), committed_date: None, created_at: None,
            web_url: None, last_pipeline: None,
        };
        let groups = group_commits_by_date(&[
            c("aaa", "2026-11-26T10:00:00Z"),
            c("bbb", "2026-11-26T08:00:00Z"),
            c("ccc", "2026-11-25T20:00:00Z"),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].label, "Nov 26, 2026");
        assert_eq!(groups[0].items.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(groups[1].label, "Nov 25, 2026");
        assert_eq!(groups[1].items.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn splits_active_and_stale_branches_keeping_default_active() {
        let now = parse_iso_utc_secs("2026-11-28T00:00:00Z").unwrap();
        let mk = |name: &str, def: bool, date: &str| Branch {
            name: name.into(), default: def,
            commit: Some(BranchCommit { committed_date: Some(date.into()), ..Default::default() }),
            ..Default::default()
        };
        let (active, stale) = split_active_stale_branches(&[
            mk("main", true, "2025-01-01T00:00:00Z"),
            mk("feat/fresh", false, "2026-11-25T00:00:00Z"),
            mk("feat/old", false, "2026-04-01T00:00:00Z"),
        ], "", now);
        assert_eq!(active.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["main", "feat/fresh"]);
        assert_eq!(stale.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["feat/old"]);
    }

    #[test]
    fn sorts_and_filters_tags() {
        let mk = |name: &str, date: &str| RepoTag {
            name: name.into(),
            commit: Some(RepoTagCommit { id: name.into(), committed_date: Some(date.into()), ..Default::default() }),
            ..Default::default()
        };
        let tags = vec![mk("v1.0.0", "2026-11-01T00:00:00Z"), mk("v1.1.0", "2026-11-26T00:00:00Z")];
        assert_eq!(filter_and_sort_tags(&tags, "", TagSortField::Updated, SortDir::Desc)[0].name, "v1.1.0");
        assert_eq!(filter_and_sort_tags(&tags, "v1.0", TagSortField::Name, SortDir::Asc).len(), 1);
    }
}
