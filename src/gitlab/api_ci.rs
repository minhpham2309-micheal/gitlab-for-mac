use reqwest::Method;

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::types::{Branch, BridgeJob, Job, PipelineTestSummary};

impl GitLab {
    pub async fn pipeline_jobs_all(&self, project_id: u64, pipeline_id: u64) -> Result<Vec<Job>> {
        let path = format!(
            "/projects/{project_id}/pipelines/{pipeline_id}/jobs?include_retried=true&per_page=100"
        );
        self.send_paginated(&path, 10).await
    }

    pub async fn pipeline_bridges(
        &self,
        project_id: u64,
        pipeline_id: u64,
    ) -> Result<Vec<BridgeJob>> {
        let path = format!("/projects/{project_id}/pipelines/{pipeline_id}/bridges?per_page=100");
        match self.send_paginated(&path, 5).await {
            Ok(items) => Ok(items),
            Err(_) => Ok(Vec::new()),
        }
    }

    pub async fn pipeline_test_report_summary(
        &self,
        project_id: u64,
        pipeline_id: u64,
    ) -> Result<Option<PipelineTestSummary>> {
        let path = format!("/projects/{project_id}/pipelines/{pipeline_id}/test_report_summary");
        match self.send::<PipelineTestSummary>(Method::GET, &path).await {
            Ok(summary) if summary.total.count > 0 => Ok(Some(summary)),
            Ok(_) | Err(GitLabError::Http { status: 403 | 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn branches(&self, project_id: u64) -> Result<Vec<Branch>> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/repository/branches?per_page=100"),
        )
        .await
    }
}
