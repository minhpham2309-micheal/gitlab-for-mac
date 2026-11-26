use reqwest::Method;
use serde_json::json;

use super::client::GitLab;
use super::diff::{ChangesPayload, RawDiff};
use super::error::{GitLabError, Result};
use super::types::*;

pub fn validate_discussion_id(id: &str) -> Result<String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(GitLabError::Message(format!("invalid discussion id '{id}'")));
    }
    Ok(urlencoding::encode(id).into_owned())
}

impl GitLab {
    pub async fn project(&self, project_id: u64) -> Result<Project> {
        self.send(Method::GET, &format!("/projects/{project_id}"))
            .await
    }

    pub async fn merge_request(&self, project_id: u64, iid: u64) -> Result<MergeRequest> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/merge_requests/{iid}"),
        )
        .await
    }

    pub async fn merge_request_diffs(&self, project_id: u64, iid: u64) -> Result<Vec<RawDiff>> {
        let path = format!("/projects/{project_id}/merge_requests/{iid}/diffs?per_page=50");
        match self.send_paginated::<RawDiff>(&path, 10).await {
            Ok(diffs) => Ok(diffs),
            Err(GitLabError::Http { status: 404, .. }) => {
                let payload = self
                    .send::<ChangesPayload>(
                        Method::GET,
                        &format!("/projects/{project_id}/merge_requests/{iid}/changes"),
                    )
                    .await?;
                Ok(payload.changes)
            }
            Err(err) => Err(err),
        }
    }

    pub async fn mr_discussions(&self, project_id: u64, iid: u64) -> Result<Vec<Discussion>> {
        let path = format!("/projects/{project_id}/merge_requests/{iid}/discussions?per_page=50");
        self.send_paginated(&path, 10).await
    }

    pub async fn create_mr_discussion(
        &self,
        project_id: u64,
        iid: u64,
        body: &str,
        position: Option<DiffPosition>,
    ) -> Result<Discussion> {
        let payload = match position {
            Some(pos) => json!({ "body": body, "position": pos }),
            None => json!({ "body": body }),
        };
        self.send_body(
            Method::POST,
            &format!("/projects/{project_id}/merge_requests/{iid}/discussions"),
            Some(payload),
        )
        .await
    }

    pub async fn reply_mr_discussion(
        &self,
        project_id: u64,
        iid: u64,
        discussion_id: &str,
        body: &str,
    ) -> Result<DiscussionNote> {
        let id = validate_discussion_id(discussion_id)?;
        self.send_body(
            Method::POST,
            &format!("/projects/{project_id}/merge_requests/{iid}/discussions/{id}/notes"),
            Some(json!({ "body": body })),
        )
        .await
    }

    pub async fn resolve_mr_discussion(
        &self,
        project_id: u64,
        iid: u64,
        discussion_id: &str,
        resolved: bool,
    ) -> Result<Discussion> {
        let id = validate_discussion_id(discussion_id)?;
        self.send_body(
            Method::PUT,
            &format!("/projects/{project_id}/merge_requests/{iid}/discussions/{id}"),
            Some(json!({ "resolved": resolved })),
        )
        .await
    }

    pub async fn issues_by_state(&self, project_id: u64, state: &str) -> Result<Vec<Issue>> {
        let state = urlencoding::encode(state);
        self.send(
            Method::GET,
            &format!(
                "/projects/{project_id}/issues?state={state}&order_by=updated_at&sort=desc&per_page=50"
            ),
        )
        .await
    }

    pub async fn issue_notes(&self, project_id: u64, iid: u64) -> Result<Vec<Note>> {
        let path = format!("/projects/{project_id}/issues/{iid}/notes?sort=asc&per_page=50");
        self.send_paginated(&path, 5).await
    }

    pub async fn create_issue_note(&self, project_id: u64, iid: u64, body: &str) -> Result<Note> {
        self.send_body(
            Method::POST,
            &format!("/projects/{project_id}/issues/{iid}/notes"),
            Some(json!({ "body": body })),
        )
        .await
    }

    pub async fn mark_all_todos_done(&self) -> Result<()> {
        self.send_empty(Method::POST, "/todos/mark_as_done").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_discussion_id_strictly_hex() {
        assert_eq!(validate_discussion_id("a1b2c3d409ef").unwrap(), "a1b2c3d409ef");
        assert!(validate_discussion_id("").is_err());
        assert!(validate_discussion_id("../../merge").is_err());
        assert!(validate_discussion_id("abc?resolved=true").is_err());
        assert!(validate_discussion_id("xyz123").is_err());
    }
}
