#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::types::User;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct DiffRefs {
    #[serde(default)]
    pub base_sha: Option<String>,
    #[serde(default)]
    pub head_sha: Option<String>,
    #[serde(default)]
    pub start_sha: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DiffPosition {
    pub position_type: String,
    pub base_sha: String,
    pub start_sha: String,
    pub head_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_line: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscussionNote {
    pub id: u64,
    pub body: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub author: Option<User>,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub resolvable: bool,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub position: Option<DiffPosition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Discussion {
    pub id: String,
    #[serde(default)]
    pub individual_note: bool,
    #[serde(default)]
    pub notes: Vec<DiscussionNote>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoProject {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path_with_namespace: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TodoTarget {
    #[serde(default)]
    pub iid: Option<u64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReleaseLinks {
    #[serde(rename = "self", default)]
    pub self_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Branch {
    pub name: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub protected: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BridgeJob {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub stage: String,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub allow_failure: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct RawTraceChunk {
    pub job_id: u64,
    pub bytes: Vec<u8>,
    pub next_offset: u64,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IssueFilter {
    #[default]
    Open,
    Closed,
    All,
}

impl IssueFilter {
    pub const ALL: [Self; 3] = [Self::Open, Self::Closed, Self::All];

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
            Self::All => "All",
        }
    }

    pub fn api_state(self) -> &'static str {
        match self {
            Self::Open => "opened",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}
