#![allow(dead_code)]

use serde::Deserialize;

use super::types_repo::RepoTreeEntry;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectStatistics {
    #[serde(default)]
    pub commit_count: u64,
    #[serde(default)]
    pub repository_size: u64,
    #[serde(default)]
    pub storage_size: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectLicense {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectDetail {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub path_with_namespace: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub web_url: String,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub star_count: u64,
    #[serde(default)]
    pub forks_count: u64,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub http_url_to_repo: Option<String>,
    #[serde(default)]
    pub ssh_url_to_repo: Option<String>,
    #[serde(default)]
    pub ci_config_path: Option<String>,
    #[serde(default)]
    pub auto_devops_enabled: Option<bool>,
    #[serde(default)]
    pub wiki_enabled: bool,
    #[serde(default)]
    pub statistics: Option<ProjectStatistics>,
    #[serde(default)]
    pub license: Option<ProjectLicense>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectOverviewCounts {
    pub branches: Option<u32>,
    pub tags: Option<u32>,
    pub releases: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LanguageSlice {
    pub name: String,
    pub percent: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SpecialRepoFiles {
    pub readme: Option<RepoTreeEntry>,
    pub ci_config: Option<RepoTreeEntry>,
    pub license: Option<RepoTreeEntry>,
    pub changelog: Option<RepoTreeEntry>,
    pub contributing: Option<RepoTreeEntry>,
}
