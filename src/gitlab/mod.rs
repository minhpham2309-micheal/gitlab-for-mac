mod ansi;
mod ansi_palette;
mod api;
mod api_ci;
mod api_discussions;
mod api_mr;
mod api_ops;
mod api_repo;
mod api_repo_meta;
mod client;
mod client_extra;
mod client_raw;
mod diff;
mod diff_patch;
#[cfg(test)]
mod diff_patch_tests;
mod error;
mod http_util;
pub mod log_model;
#[cfg(test)]
mod log_model_tests;
pub mod log_parse;
mod log_query;
pub mod log_sanitize;
#[cfg(test)]
mod log_util;
pub mod media;
mod mr_readiness;
pub mod repo_tree_util;
pub mod time_util;
mod trace_util;
mod types;
mod types_mr;
mod types_project;
mod types_repo;
mod types_review;

pub use ansi::{colorize, strip, Rgb};
pub use client::GitLab;
pub use client_raw::MAX_VIEWER_BLOB_BYTES;
pub use diff::{parse_unified_full, DiffKind, DiffLine, MrChange, MrDiff, RawDiff};
pub use error::GitLabError;
pub use types::*;
pub use types_project::*;
pub use types_repo::*;

pub fn merge_projects(starred: Vec<Project>, member: Vec<Project>) -> Vec<Project> {
    let mut out = starred;
    for project in member {
        if out.iter().all(|existing| existing.id != project.id) {
            out.push(project);
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MrFilter {
    #[default]
    Open,
    Merged,
    Closed,
    All,
}

impl MrFilter {
    pub const ALL: [Self; 4] = [Self::Open, Self::Merged, Self::Closed, Self::All];

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Merged => "Merged",
            Self::Closed => "Closed",
            Self::All => "All",
        }
    }

    pub fn api_state(self) -> &'static str {
        match self {
            Self::Open => "opened",
            Self::Merged => "merged",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }

    pub fn matches(self, state: &str) -> bool {
        match self {
            Self::All => true,
            Self::Open => mr_can_review(state),
            Self::Merged => state == "merged",
            Self::Closed => state == "closed",
        }
    }
}

pub fn mr_can_review(state: &str) -> bool {
    matches!(state, "opened" | "open" | "locked")
}

pub fn job_is_live(status: &str) -> bool {
    matches!(
        status,
        "created"
            | "pending"
            | "running"
            | "waiting_for_resource"
            | "preparing"
            | "waiting_for_callback"
            | "canceling"
    )
}

pub fn pipeline_is_live(status: &str) -> bool {
    matches!(
        status,
        "created"
            | "pending"
            | "running"
            | "waiting_for_resource"
            | "preparing"
            | "waiting_for_callback"
            | "canceling"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_dedups_by_id() {
        let starred = vec![Project {
            id: 1,
            name: "A".into(),
            path_with_namespace: "g/a".into(),
            default_branch: None,
            web_url: String::new(),
            last_activity_at: None,
            star_count: 1,
            avatar_url: None,
        }];
        let member = vec![
            starred[0].clone(),
            Project {
                id: 2,
                name: "B".into(),
                path_with_namespace: "g/b".into(),
                default_branch: None,
                web_url: String::new(),
                last_activity_at: None,
                star_count: 0,
                avatar_url: None,
            },
        ];
        assert_eq!(merge_projects(starred, member).len(), 2);
    }

    #[test]
    fn open_mrs_can_be_reviewed() {
        assert!(mr_can_review("opened"));
        assert!(mr_can_review("locked"));
        assert!(!mr_can_review("merged"));
        assert!(!mr_can_review("closed"));
    }

    #[test]
    fn mr_filter_maps_gitlab_states() {
        assert!(MrFilter::Open.matches("opened"));
        assert!(MrFilter::Merged.matches("merged"));
        assert!(MrFilter::Closed.matches("closed"));
        assert!(MrFilter::All.matches("merged"));
        assert!(!MrFilter::Open.matches("merged"));
        assert_eq!(MrFilter::Open.api_state(), "opened");
    }

    #[test]
    fn pipeline_live_covers_running_and_pending() {
        assert!(pipeline_is_live("running"));
        assert!(pipeline_is_live("pending"));
        assert!(pipeline_is_live("canceling"));
        assert!(job_is_live("canceling"));
        assert!(!pipeline_is_live("success"));
        assert!(!pipeline_is_live("failed"));
        assert!(!job_is_live("canceled"));
    }
}
