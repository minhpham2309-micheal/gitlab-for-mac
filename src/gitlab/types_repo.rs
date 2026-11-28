#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct TreeLastCommit {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub short_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub committed_date: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl TreeLastCommit {
    pub fn timestamp(&self) -> Option<&str> {
        self.committed_date.as_deref().or(self.created_at.as_deref())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RepoTreeEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub path: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub last_commit: Option<TreeLastCommit>,
}

impl RepoTreeEntry {
    pub fn is_dir(&self) -> bool {
        self.entry_type == "tree"
    }

    pub fn is_blob(&self) -> bool {
        self.entry_type == "blob"
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoCommitPipeline {
    pub id: u64,
    pub status: String,
    #[serde(default)]
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoCommit {
    pub id: String,
    #[serde(default)]
    pub short_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: Option<String>,
    #[serde(default)]
    pub authored_date: Option<String>,
    #[serde(default)]
    pub committed_date: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub last_pipeline: Option<RepoCommitPipeline>,
}

impl RepoCommit {
    pub fn display_short_id(&self) -> &str {
        if !self.short_id.is_empty() {
            &self.short_id
        } else {
            self.id.get(..8).unwrap_or(&self.id)
        }
    }

    pub fn timestamp(&self) -> Option<&str> {
        self.authored_date
            .as_deref()
            .or(self.committed_date.as_deref())
            .or(self.created_at.as_deref())
    }

    pub fn extra_body(&self) -> Option<&str> {
        let rest = self.message.strip_prefix(&self.title).unwrap_or(&self.message).trim();
        (!rest.is_empty() && rest != self.title.trim()).then_some(rest)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RepoTagCommit {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub short_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub authored_date: Option<String>,
    #[serde(default)]
    pub committed_date: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl RepoTagCommit {
    pub fn display_short_id(&self) -> &str {
        if !self.short_id.is_empty() {
            &self.short_id
        } else {
            self.id.get(..8).unwrap_or(&self.id)
        }
    }

    pub fn timestamp(&self) -> Option<&str> {
        self.committed_date
            .as_deref()
            .or(self.authored_date.as_deref())
            .or(self.created_at.as_deref())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RepoTagRelease {
    #[serde(default)]
    pub tag_name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RepoTag {
    pub name: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub protected: bool,
    #[serde(default)]
    pub commit: Option<RepoTagCommit>,
    #[serde(default)]
    pub release: Option<RepoTagRelease>,
}

impl RepoTag {
    pub fn timestamp(&self) -> Option<&str> {
        self.commit.as_ref().and_then(|c| c.timestamp())
    }
}

#[derive(Debug, Clone)]
pub enum RepoBlobKind {
    Text(String),
    Image { bytes: Vec<u8>, content_type: String },
    Binary,
    TooLarge { limit_bytes: usize },
}

#[derive(Debug, Clone)]
pub struct RepoBlob {
    pub sha: Option<String>,
    pub size_bytes: usize,
    pub kind: RepoBlobKind,
}

impl RepoBlob {
    pub fn approx_bytes(&self) -> usize {
        match &self.kind {
            RepoBlobKind::Text(s) => s.len(),
            RepoBlobKind::Image { bytes, .. } => bytes.len(),
            RepoBlobKind::Binary | RepoBlobKind::TooLarge { .. } => 64,
        }
    }
}
