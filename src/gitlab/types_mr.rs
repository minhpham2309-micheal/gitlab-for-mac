#![allow(dead_code)]

use serde::Deserialize;

use super::permissions::MrUserPermissions;
use super::types::{Pipeline, User};
use super::types_review::DiffRefs;

#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequest {
    pub id: u64,
    pub iid: u64,
    pub title: String,
    pub state: String,
    pub source_branch: String,
    pub target_branch: String,
    pub web_url: String,
    pub merge_status: Option<String>,
    pub has_conflicts: Option<bool>,
    pub author: Option<User>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub diff_refs: Option<DiffRefs>,
    #[serde(default)]
    pub head_pipeline: Option<Pipeline>,
    #[serde(default)]
    pub reviewers: Vec<User>,
    #[serde(default)]
    pub assignees: Vec<User>,
    #[serde(default)]
    pub user_notes_count: u32,
    #[serde(default)]
    pub upvotes: u32,
    #[serde(default)]
    pub downvotes: u32,
    #[serde(default)]
    pub milestone: Option<Milestone>,
    #[serde(default)]
    pub time_stats: Option<TimeStats>,
    #[serde(default)]
    pub detailed_merge_status: Option<String>,
    #[serde(default)]
    pub diverged_commits_count: Option<u32>,
    #[serde(default)]
    pub rebase_in_progress: Option<bool>,
    #[serde(default)]
    pub should_remove_source_branch: Option<bool>,
    #[serde(default)]
    pub force_remove_source_branch: Option<bool>,
    #[serde(default)]
    pub squash: Option<bool>,
    #[serde(default)]
    pub blocking_discussions_resolved: Option<bool>,
    #[serde(default)]
    pub merged_by: Option<User>,
    #[serde(default)]
    pub merged_at: Option<String>,
    #[serde(default)]
    pub closed_by: Option<User>,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub user: Option<MrUserPermissions>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Milestone {
    pub id: u64,
    #[serde(default)]
    pub iid: Option<u64>,
    pub title: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TimeStats {
    #[serde(default)]
    pub time_estimate: u64,
    #[serde(default)]
    pub total_time_spent: u64,
    #[serde(default)]
    pub human_time_estimate: Option<String>,
    #[serde(default)]
    pub human_total_time_spent: Option<String>,
}

impl TimeStats {
    pub fn summary(&self) -> Option<String> {
        let spent = self.human_total_time_spent.as_deref().filter(|s| !s.is_empty());
        let est = self.human_time_estimate.as_deref().filter(|s| !s.is_empty());
        match (spent, est) {
            (Some(s), Some(e)) => Some(format!("Spent {s} / Est {e}")),
            (Some(s), None) => Some(format!("Spent {s}")),
            (None, Some(e)) => Some(format!("Estimate {e}")),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MrApprovalUser {
    pub user: User,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MrApprovals {
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub approvals_required: u32,
    #[serde(default)]
    pub approvals_left: u32,
    #[serde(default)]
    pub user_has_approved: bool,
    #[serde(default)]
    pub user_can_approve: bool,
    #[serde(default)]
    pub approved_by: Vec<MrApprovalUser>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AwardEmoji {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub user: Option<User>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MrListCounts {
    pub open: Option<u32>,
    pub merged: Option<u32>,
    pub closed: Option<u32>,
    pub all: Option<u32>,
}

impl MrListCounts {
    pub fn for_filter(&self, filter: super::MrFilter) -> Option<u32> {
        match filter {
            super::MrFilter::Open => self.open,
            super::MrFilter::Merged => self.merged,
            super::MrFilter::Closed => self.closed,
            super::MrFilter::All => self.all,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MrSortField {
    #[default]
    CreatedAt,
    UpdatedAt,
    Title,
}

impl MrSortField {
    pub const ALL: [Self; 3] = [Self::CreatedAt, Self::UpdatedAt, Self::Title];
    pub fn label(self) -> &'static str {
        match self { Self::CreatedAt => "Created date", Self::UpdatedAt => "Updated date", Self::Title => "Title" }
    }
    pub fn api_value(self) -> &'static str {
        match self { Self::CreatedAt => "created_at", Self::UpdatedAt => "updated_at", Self::Title => "title" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDir { #[default] Desc, Asc }

impl SortDir {
    pub fn toggle(self) -> Self { match self { Self::Desc => Self::Asc, Self::Asc => Self::Desc } }
    pub fn api_value(self) -> &'static str { match self { Self::Desc => "desc", Self::Asc => "asc" } }
}

#[derive(Debug, Clone)]
pub struct MrListQuery {
    pub state: String,
    pub search: String,
    pub order_by: MrSortField,
    pub sort: SortDir,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Default)]
pub struct MrPage {
    pub items: Vec<MergeRequest>,
    pub page: usize,
    pub per_page: usize,
    pub total: Option<u32>,
    pub next_page: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MrActivityFilter { #[default] All, CommentsOnly, HistoryOnly }

impl MrActivityFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::CommentsOnly, Self::HistoryOnly];
    pub fn label(self) -> &'static str {
        match self { Self::All => "All activity", Self::CommentsOnly => "Comments only", Self::HistoryOnly => "History only" }
    }
}
