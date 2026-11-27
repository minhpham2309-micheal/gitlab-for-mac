use reqwest::Method;
use serde_json::json;

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::types::{MergeRequest, Pipeline, User};
use super::types_mr::{AwardEmoji, MrApprovals, MrListCounts, MrListQuery, MrPage};
use super::types_repo::RepoCommit;

impl GitLab {
    pub async fn merge_requests_page(&self, project_id: u64, q: &MrListQuery) -> Result<MrPage> {
        let state = urlencoding::encode(&q.state);
        let page = q.page.max(1);
        let per_page = q.per_page.clamp(1, 100);
        let mut path = format!(
            "/projects/{project_id}/merge_requests?state={state}&order_by={}&sort={}&per_page={per_page}&page={page}",
            q.order_by.api_value(), q.sort.api_value(),
        );
        if !q.search.trim().is_empty() {
            path.push_str("&search=");
            path.push_str(&urlencoding::encode(q.search.trim()));
        }
        let (items, total, next_page) = self.send_page::<MergeRequest>(&path).await?;
        Ok(MrPage { items, page, per_page, total, next_page })
    }

    pub async fn merge_request_counts(&self, project_id: u64, search: &str) -> MrListCounts {
        let q = search.trim();
        let sfx = if q.is_empty() { String::new() } else { format!("&search={}", urlencoding::encode(q)) };
        let p_open = format!("/projects/{project_id}/merge_requests?state=opened{sfx}");
        let p_merged = format!("/projects/{project_id}/merge_requests?state=merged{sfx}");
        let p_closed = format!("/projects/{project_id}/merge_requests?state=closed{sfx}");
        let p_all = format!("/projects/{project_id}/merge_requests?state=all{sfx}");
        let (open, merged, closed, all) = tokio::join!(
            self.fetch_total_count(&p_open),
            self.fetch_total_count(&p_merged),
            self.fetch_total_count(&p_closed),
            self.fetch_total_count(&p_all),
        );
        MrListCounts {
            open: open.ok().flatten(),
            merged: merged.ok().flatten(),
            closed: closed.ok().flatten(),
            all: all.ok().flatten(),
        }
    }

    pub async fn merge_request_detail(&self, project_id: u64, iid: u64) -> Result<MergeRequest> {
        let path = format!(
            "/projects/{project_id}/merge_requests/{iid}?include_diverged_commits_count=true&include_rebase_in_progress=true&with_merge_status_recheck=true"
        );
        self.send(Method::GET, &path).await
    }

    pub async fn mr_approvals(&self, project_id: u64, iid: u64) -> Result<Option<MrApprovals>> {
        let path = format!("/projects/{project_id}/merge_requests/{iid}/approvals");
        match self.send::<MrApprovals>(Method::GET, &path).await {
            Ok(a) => Ok(Some(a)),
            Err(GitLabError::Http { status: 403 | 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn unapprove_mr(&self, project_id: u64, iid: u64) -> Result<()> {
        self.send_empty(Method::POST, &format!("/projects/{project_id}/merge_requests/{iid}/unapprove")).await
    }

    pub async fn merge_mr_with_options(
        &self,
        project_id: u64,
        iid: u64,
        remove_source_branch: bool,
        squash: bool,
        auto_merge: bool,
        commit_message: Option<&str>,
    ) -> Result<()> {
        let mut body = json!({ "should_remove_source_branch": remove_source_branch, "squash": squash });
        if auto_merge {
            body["auto_merge"] = json!(true);
            body["merge_when_pipeline_succeeds"] = json!(true);
        }
        if let Some(msg) = commit_message.map(str::trim).filter(|s| !s.is_empty()) {
            let key = if squash { "squash_commit_message" } else { "merge_commit_message" };
            body[key] = json!(msg);
        }
        let _: serde_json::Value = self
            .send_body(Method::PUT, &format!("/projects/{project_id}/merge_requests/{iid}/merge"), Some(body))
            .await?;
        Ok(())
    }

    pub async fn reopen_mr(&self, project_id: u64, iid: u64) -> Result<MergeRequest> {
        self.send_body(
            Method::PUT,
            &format!("/projects/{project_id}/merge_requests/{iid}"),
            Some(json!({ "state_event": "reopen" })),
        ).await
    }

    pub async fn rebase_mr(&self, project_id: u64, iid: u64) -> Result<()> {
        let _: serde_json::Value = self
            .send_body(Method::PUT, &format!("/projects/{project_id}/merge_requests/{iid}/rebase"), Some(json!({})))
            .await?;
        Ok(())
    }

    pub async fn mr_commits(&self, project_id: u64, iid: u64) -> Result<Vec<RepoCommit>> {
        self.send_paginated(&format!("/projects/{project_id}/merge_requests/{iid}/commits?per_page=100"), 5).await
    }

    pub async fn mr_pipelines(&self, project_id: u64, iid: u64) -> Result<Vec<Pipeline>> {
        self.send_paginated(&format!("/projects/{project_id}/merge_requests/{iid}/pipelines?per_page=50"), 3).await
    }

    pub async fn mr_participants(&self, project_id: u64, iid: u64) -> Result<Vec<User>> {
        self.send(Method::GET, &format!("/projects/{project_id}/merge_requests/{iid}/participants?per_page=50")).await
    }

    pub async fn mr_award_emojis(&self, project_id: u64, iid: u64) -> Result<Vec<AwardEmoji>> {
        self.send_paginated(&format!("/projects/{project_id}/merge_requests/{iid}/award_emoji?per_page=100"), 3).await
    }

    pub async fn add_mr_award_emoji(&self, project_id: u64, iid: u64, name: &str) -> Result<AwardEmoji> {
        self.send_body(
            Method::POST,
            &format!("/projects/{project_id}/merge_requests/{iid}/award_emoji"),
            Some(json!({ "name": name })),
        ).await
    }

    pub async fn delete_mr_award_emoji(&self, project_id: u64, iid: u64, award_id: u64) -> Result<()> {
        self.send_empty(
            Method::DELETE,
            &format!("/projects/{project_id}/merge_requests/{iid}/award_emoji/{award_id}"),
        ).await
    }

    pub async fn create_mr_todo(&self, project_id: u64, iid: u64) -> Result<()> {
        let _: serde_json::Value = self
            .send_body(Method::POST, &format!("/projects/{project_id}/merge_requests/{iid}/todo"), Some(json!({})))
            .await?;
        Ok(())
    }

    pub async fn update_mr_fields(
        &self, project_id: u64, iid: u64, body: serde_json::Value,
    ) -> Result<MergeRequest> {
        self.send_body(Method::PUT, &format!("/projects/{project_id}/merge_requests/{iid}"), Some(body)).await
    }

    pub async fn project_members(&self, project_id: u64) -> Result<Vec<User>> {
        let primary = format!("/projects/{project_id}/members/all?per_page=100");
        let raw = match self.send_paginated::<User>(&primary, 3).await {
            Ok(users) => users,
            Err(GitLabError::Http { status: 403 | 404, .. }) => {
                let fallback = format!("/projects/{project_id}/users?per_page=100");
                self.send_paginated::<User>(&fallback, 3).await?
            }
            Err(err) => return Err(err),
        };
        let mut deduped: Vec<User> = Vec::with_capacity(raw.len());
        for user in raw {
            if deduped.iter().all(|u| u.id != user.id) {
                deduped.push(user);
            }
        }
        deduped.sort_by_key(|a| a.name.to_lowercase());
        Ok(deduped)
    }
}
