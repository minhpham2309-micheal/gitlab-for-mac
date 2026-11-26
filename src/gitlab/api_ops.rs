use reqwest::Method;

use super::client::GitLab;
use super::error::Result;
use super::types::{Release, Runner, Schedule, SearchBlob};

impl GitLab {
    pub async fn releases(&self, project_id: u64) -> Result<Vec<Release>> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/releases?per_page=30"),
        )
        .await
    }

    pub async fn schedules(&self, project_id: u64) -> Result<Vec<Schedule>> {
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/pipeline_schedules"),
        )
        .await
    }

    pub async fn play_schedule(&self, project_id: u64, id: u64) -> Result<()> {
        self.send_empty(
            Method::POST,
            &format!("/projects/{project_id}/pipeline_schedules/{id}/play"),
        )
        .await
    }

    pub async fn runners(&self, project_id: u64) -> Result<Vec<Runner>> {
        self.send(Method::GET, &format!("/projects/{project_id}/runners"))
            .await
    }

    pub async fn search(&self, project_id: u64, query: &str) -> Result<Vec<SearchBlob>> {
        let q = urlencoding::encode(query);
        self.send(
            Method::GET,
            &format!("/projects/{project_id}/search?scope=blobs&search={q}&per_page=30"),
        )
        .await
    }
}
