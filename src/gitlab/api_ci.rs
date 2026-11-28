use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Method;
use tokio::sync::Semaphore;

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::types::{Branch, BridgeJob, Job, Pipeline, PipelineTestSummary};
use super::types_ci::PipelineDetail;

const MAX_MR_PIPELINE_FALLBACKS: usize = 25;
const MR_PIPELINE_CONCURRENCY: usize = 6;

#[derive(Debug, Clone)]
pub struct MrPipelineTarget {
    pub iid: u64,
    pub sha: Option<String>,
    pub source_branch: String,
}

fn is_mr_pipeline_ref(git_ref: &str, iid: u64) -> bool {
    let prefix = format!("refs/merge-requests/{iid}/");
    git_ref.starts_with(&prefix)
}

pub(crate) fn match_pipelines_to_mrs(
    targets: &[MrPipelineTarget],
    recent: &[Pipeline],
) -> HashMap<u64, Pipeline> {
    let mut out = HashMap::new();
    for t in targets {
        let sha = t.sha.as_deref().filter(|s| !s.is_empty());
        let by_mr_ref = recent
            .iter()
            .filter(|p| is_mr_pipeline_ref(&p.git_ref, t.iid))
            .max_by_key(|p| p.id);
        let by_branch_sha = sha.and_then(|s| {
            recent
                .iter()
                .filter(|p| p.sha == s && p.git_ref == t.source_branch)
                .max_by_key(|p| p.id)
        });
        let picked = match (by_mr_ref, by_branch_sha) {
            (Some(a), Some(b)) => Some(if a.id >= b.id { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => sha.and_then(|s| recent.iter().filter(|p| p.sha == s).max_by_key(|p| p.id)),
        };
        if let Some(p) = picked {
            out.insert(t.iid, p.clone());
        }
    }
    out
}

impl GitLab {
    pub async fn pipeline_detail(&self, project_id: u64, pipeline_id: u64) -> Result<PipelineDetail> {
        self.send(Method::GET, &format!("/projects/{project_id}/pipelines/{pipeline_id}"))
            .await
    }

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

    pub async fn mr_latest_pipeline(&self, project_id: u64, iid: u64) -> Result<Option<Pipeline>> {
        let path = format!("/projects/{project_id}/merge_requests/{iid}/pipelines?per_page=1");
        let list: Vec<Pipeline> = self.send(Method::GET, &path).await?;
        Ok(list.into_iter().next())
    }

    pub async fn hydrate_mr_head_pipelines(
        &self,
        project_id: u64,
        targets: Vec<MrPipelineTarget>,
    ) -> HashMap<u64, Pipeline> {
        if targets.is_empty() {
            return HashMap::new();
        }
        let recent: Vec<Pipeline> = self
            .send(Method::GET, &format!("/projects/{project_id}/pipelines?per_page=100"))
            .await
            .unwrap_or_default();
        let mut resolved = match_pipelines_to_mrs(&targets, &recent);
        let missing: Vec<u64> = targets
            .iter()
            .filter(|t| !resolved.contains_key(&t.iid))
            .take(MAX_MR_PIPELINE_FALLBACKS)
            .map(|t| t.iid)
            .collect();
        if missing.is_empty() {
            return resolved;
        }
        resolved.extend(self.fetch_mr_latest_pipelines(project_id, missing).await);
        resolved
    }

    pub async fn fetch_mr_latest_pipelines(
        &self,
        project_id: u64,
        iids: Vec<u64>,
    ) -> HashMap<u64, Pipeline> {
        let sem = Arc::new(Semaphore::new(MR_PIPELINE_CONCURRENCY));
        let mut handles = Vec::with_capacity(iids.len());
        for iid in iids {
            let client = self.clone();
            let sem = sem.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                let pipe = client.mr_latest_pipeline(project_id, iid).await.ok().flatten()?;
                Some((iid, pipe))
            }));
        }
        let mut out = HashMap::new();
        for handle in handles {
            if let Ok(Some((iid, pipe))) = handle.await {
                out.insert(iid, pipe);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipe(id: u64, status: &str, git_ref: &str, sha: &str) -> Pipeline {
        Pipeline {
            id,
            iid: None,
            status: status.into(),
            git_ref: git_ref.into(),
            sha: sha.into(),
            web_url: format!("https://gitlab.example.com/g/p/-/pipelines/{id}"),
            created_at: None,
            updated_at: None,
            source: None,
        }
    }

    #[test]
    fn matches_mr_ref_and_branch_sha_pipelines_newest_first() {
        let targets = vec![
            MrPipelineTarget { iid: 10, sha: Some("aaa111".into()), source_branch: "feat/a".into() },
            MrPipelineTarget { iid: 11, sha: Some("bbb222".into()), source_branch: "feat/b".into() },
            MrPipelineTarget { iid: 12, sha: Some("ccc333".into()), source_branch: "feat/c".into() },
        ];
        let recent = vec![
            pipe(101, "failed", "refs/merge-requests/10/head", "oldsha"),
            pipe(105, "running", "refs/merge-requests/10/merge", "mergedsha"),
            pipe(104, "success", "feat/b", "bbb222"),
            pipe(102, "failed", "feat/b", "bbb222"),
        ];
        let matched = match_pipelines_to_mrs(&targets, &recent);
        assert_eq!(matched.get(&10).map(|p| (p.id, p.status.as_str())), Some((105, "running")));
        assert_eq!(matched.get(&11).map(|p| (p.id, p.status.as_str())), Some((104, "success")));
        assert!(!matched.contains_key(&12));
    }
}
