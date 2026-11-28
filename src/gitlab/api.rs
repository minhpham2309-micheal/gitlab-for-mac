use reqwest::Method;
use serde_json::json;

use super::client::GitLab;
use super::error::Result;
use super::types::*;

impl GitLab {
    pub async fn current_user(&self) -> Result<User> {
        self.send(Method::GET, "/user").await
    }

    pub async fn projects(&self) -> Result<Vec<Project>> {
        let starred = self
            .send::<Vec<Project>>(
                Method::GET,
                "/projects?starred=true&simple=true&archived=false&order_by=last_activity_at&sort=desc&per_page=50",
            )
            .await
            .unwrap_or_default();
        let member = self
            .send::<Vec<Project>>(
                Method::GET,
                "/projects?membership=true&simple=true&archived=false&order_by=last_activity_at&sort=desc&per_page=100",
            )
            .await?;
        Ok(super::merge_projects(starred, member))
    }

    pub async fn search_projects(&self, query: &str) -> Result<Vec<Project>> {
        let q = urlencoding::encode(query);
        self.send(
            Method::GET,
            &format!(
                "/projects?search={q}&simple=true&archived=false&order_by=last_activity_at&sort=desc&per_page=50"
            ),
        )
        .await
    }

    pub async fn job(&self, project_id: u64, job_id: u64) -> Result<Job> {
        self.send(Method::GET, &format!("/projects/{project_id}/jobs/{job_id}"))
            .await
    }

    pub async fn pipelines(&self, project_id: u64) -> Result<Vec<Pipeline>> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/pipelines?per_page=40"),
        )
        .await
    }

    pub async fn jobs(&self, project_id: u64, pipeline_id: u64) -> Result<Vec<Job>> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/pipelines/{pipeline_id}/jobs?per_page=80"),
        )
        .await
    }

    pub async fn retry_job(&self, project_id: u64, job_id: u64) -> Result<Job> {
        self.send(
            Method::POST,
            &format!("/projects/{project_id}/jobs/{job_id}/retry"),
        )
        .await
    }

    pub async fn cancel_job(&self, project_id: u64, job_id: u64) -> Result<Job> {
        self.send(
            Method::POST,
            &format!("/projects/{project_id}/jobs/{job_id}/cancel"),
        )
        .await
    }

    pub async fn play_job(&self, project_id: u64, job_id: u64) -> Result<Job> {
        self.send(
            Method::POST,
            &format!("/projects/{project_id}/jobs/{job_id}/play"),
        )
        .await
    }

    pub async fn cancel_pipeline(&self, project_id: u64, pipeline_id: u64) -> Result<Pipeline> {
        self.send(
            Method::POST,
            &format!("/projects/{project_id}/pipelines/{pipeline_id}/cancel"),
        )
        .await
    }

    pub async fn retry_pipeline(&self, project_id: u64, pipeline_id: u64) -> Result<Pipeline> {
        self.send(
            Method::POST,
            &format!("/projects/{project_id}/pipelines/{pipeline_id}/retry"),
        )
        .await
    }

    pub async fn run_pipeline(&self, project_id: u64, git_ref: &str) -> Result<Pipeline> {
        self.send_body(
            Method::POST,
            &format!("/projects/{project_id}/pipeline"),
            Some(json!({ "ref": git_ref })),
        )
        .await
    }

    pub async fn merge_requests(&self, project_id: u64, state: &str) -> Result<Vec<MergeRequest>> {
        self.send(
            Method::GET,
            &format!(
                "/projects/{project_id}/merge_requests?state={state}&order_by=updated_at&sort=desc&per_page=50"
            ),
        )
        .await
    }

    pub async fn merge_request_diff(&self, project_id: u64, iid: u64) -> Result<super::MrDiff> {
        let payload = self
            .send::<super::diff::ChangesPayload>(
                Method::GET,
                &format!("/projects/{project_id}/merge_requests/{iid}/changes"),
            )
            .await?;
        Ok(super::MrDiff::from_payload(payload))
    }

    pub async fn approve_mr(&self, project_id: u64, iid: u64) -> Result<()> {
        self.send_empty(
            Method::POST,
            &format!("/projects/{project_id}/merge_requests/{iid}/approve"),
        )
        .await
    }

    pub async fn merge_mr(&self, project_id: u64, iid: u64) -> Result<()> {
        self.send_empty(
            Method::PUT,
            &format!("/projects/{project_id}/merge_requests/{iid}/merge"),
        )
        .await
    }

    pub async fn close_mr(&self, project_id: u64, iid: u64) -> Result<MergeRequest> {
        self.send_body(
            Method::PUT,
            &format!("/projects/{project_id}/merge_requests/{iid}"),
            Some(json!({ "state_event": "close" })),
        )
        .await
    }

    pub async fn close_issue(&self, project_id: u64, iid: u64) -> Result<Issue> {
        self.send_body(
            Method::PUT,
            &format!("/projects/{project_id}/issues/{iid}"),
            Some(json!({ "state_event": "close" })),
        )
        .await
    }

    pub async fn todos(&self) -> Result<Vec<Todo>> {
        self.todos_by_state("pending").await
    }

    pub async fn todos_by_state(&self, state: &str) -> Result<Vec<Todo>> {
        self.send(Method::GET, &format!("/todos?state={}&per_page=40", urlencoding::encode(state)))
            .await
    }

    pub async fn mark_todo_done(&self, id: u64) -> Result<()> {
        self.send_empty(Method::POST, &format!("/todos/{id}/mark_as_done"))
            .await
    }
}
