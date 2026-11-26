use std::collections::HashMap;

use reqwest::Method;

use super::client::GitLab;
use super::error::Result;
use super::types_project::{LanguageSlice, ProjectDetail, ProjectOverviewCounts};
use super::types_repo::{RepoCommit, RepoTag};

impl GitLab {
    pub async fn project_detail(&self, project_id: u64) -> Result<ProjectDetail> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}?statistics=true&license=true"),
        )
        .await
    }

    pub async fn project_languages(&self, project_id: u64) -> Result<Vec<LanguageSlice>> {
        let raw: HashMap<String, f32> = self
            .send(Method::GET, &format!("/projects/{project_id}/languages"))
            .await?;
        let mut out: Vec<LanguageSlice> = raw
            .into_iter()
            .filter(|(_, pct)| pct.is_finite() && *pct > 0.0)
            .map(|(name, percent)| LanguageSlice { name, percent })
            .collect();
        out.sort_by(|a, b| {
            b.percent
                .partial_cmp(&a.percent)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    }

    pub async fn project_overview_counts(&self, project_id: u64) -> ProjectOverviewCounts {
        let b_path = format!("/projects/{project_id}/repository/branches");
        let t_path = format!("/projects/{project_id}/repository/tags");
        let r_path = format!("/projects/{project_id}/releases");
        let (branches, tags, releases) = tokio::join!(
            self.fetch_total_count(&b_path),
            self.fetch_total_count(&t_path),
            self.fetch_total_count(&r_path),
        );
        ProjectOverviewCounts {
            branches: branches.ok().flatten(),
            tags: tags.ok().flatten(),
            releases: releases.ok().flatten(),
        }
    }

    pub async fn star_project(&self, project_id: u64) -> Result<ProjectDetail> {
        self.send(Method::POST, &format!("/projects/{project_id}/star"))
            .await
    }

    pub async fn unstar_project(&self, project_id: u64) -> Result<ProjectDetail> {
        self.send(Method::POST, &format!("/projects/{project_id}/unstar"))
            .await
    }

    pub async fn repo_commits(
        &self,
        project_id: u64,
        git_ref: &str,
        path: Option<&str>,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<RepoCommit>> {
        let mut url = format!(
            "/projects/{project_id}/repository/commits?per_page={}&page={}",
            per_page.clamp(1, 100),
            page.max(1)
        );
        if !git_ref.is_empty() {
            url.push_str("&ref_name=");
            url.push_str(&urlencoding::encode(git_ref));
        }
        if let Some(p) = path.filter(|s| !s.is_empty()) {
            url.push_str("&path=");
            url.push_str(&urlencoding::encode(p));
        }
        self.send(Method::GET, &url).await
    }

    pub async fn repo_commit_detail(&self, project_id: u64, sha: &str) -> Result<RepoCommit> {
        let sha_enc = urlencoding::encode(sha);
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/repository/commits/{sha_enc}"),
        )
        .await
    }

    pub async fn repo_tags(&self, project_id: u64) -> Result<Vec<RepoTag>> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/repository/tags?per_page=100"),
        )
        .await
    }
}
