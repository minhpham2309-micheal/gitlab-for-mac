#![allow(dead_code)]

use serde::Deserialize;

pub use super::mr_readiness::{
    mr_merge_readiness, mr_merge_readiness_with_threads, parse_system_note, MergeCheckFailure,
    MergeCheckKind, MergeReadiness, MergeReadinessTone, ParsedSystemNote, SystemCommitEntry,
};
pub use super::types_mr::{
    AwardEmoji, MergeRequest, Milestone, MrActivityFilter, MrApprovalUser, MrApprovals,
    MrListCounts, MrListQuery, MrPage, MrSortField, SortDir, TimeStats,
};
pub use super::types_review::{
    Branch, BridgeJob, DiffPosition, DiffRefs, Discussion, DiscussionNote, IssueFilter,
    PipelineTestSummary, PipelineTestTotals, RawTraceChunk, ReleaseLinks, TestSuiteSummary,
    TodoProject, TodoTarget,
};

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub path_with_namespace: String,
    pub default_branch: Option<String>,
    pub web_url: String,
    pub last_activity_at: Option<String>,
    #[serde(default)]
    pub star_count: u64,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

impl std::fmt::Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path_with_namespace)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pipeline {
    pub id: u64,
    pub iid: Option<u64>,
    pub status: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub sha: String,
    pub web_url: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub stage: String,
    pub web_url: String,
    pub duration: Option<f64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub allow_failure: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub iid: u64,
    pub title: String,
    pub state: String,
    pub web_url: String,
    pub author: Option<User>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub assignees: Vec<User>,
    #[serde(default)]
    pub user_notes_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Todo {
    pub id: u64,
    pub action_name: String,
    pub target_url: Option<String>,
    pub body: String,
    pub state: String,
    #[serde(default)]
    pub project: Option<TodoProject>,
    #[serde(default)]
    pub target_type: Option<String>,
    #[serde(default)]
    pub target: Option<TodoTarget>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub released_at: Option<String>,
    pub description: Option<String>,
    #[serde(default, rename = "_links")]
    pub links: Option<ReleaseLinks>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Schedule {
    pub id: u64,
    pub description: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub cron: String,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Runner {
    pub id: u64,
    pub description: Option<String>,
    pub status: Option<String>,
    pub is_shared: Option<bool>,
    pub paused: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchBlob {
    pub basename: Option<String>,
    pub data: Option<String>,
    pub filename: Option<String>,
    pub path: Option<String>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    pub startline: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Note {
    pub id: u64,
    pub body: String,
    pub created_at: Option<String>,
    pub author: Option<User>,
}

#[derive(Debug, Clone, Default)]
pub struct TraceChunk {
    pub job_id: u64,
    pub text: String,
    pub next_offset: u64,
    pub complete: bool,
}
