use reqwest::{header, Method, StatusCode};
use serde::de::DeserializeOwned;

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::http_util::{send_with_redirects, stream_to_file, MAX_ARTIFACT_BYTES};
use super::trace_util::fresh_trace;
use super::types::RawTraceChunk;

impl GitLab {
    pub async fn download_artifacts(&self, project_id: u64, job_id: u64) -> Result<String> {
        let dir = dirs::download_dir().ok_or_else(|| {
            GitLabError::Message("macOS Downloads directory is unavailable".into())
        })?;
        let response = send_with_redirects(
            &self.stream_http,
            Method::GET,
            self.url(&format!("/projects/{project_id}/jobs/{job_id}/artifacts"))?,
            &self.origin,
            &self.auth.token,
            None,
            &[],
        )
        .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GitLabError::from_status(status, body));
        }
        let path = dir.join(format!("glab-job-{job_id}.zip"));
        stream_to_file(response, &path, MAX_ARTIFACT_BYTES).await?;
        Ok(path.display().to_string())
    }

    pub async fn fetch_total_count(&self, path: &str) -> Result<Option<u32>> {
        let sep = if path.contains('?') { '&' } else { '?' };
        let url = self.url(&format!("{path}{sep}per_page=1"))?;
        let response = send_with_redirects(
            &self.http,
            Method::GET,
            url,
            &self.origin,
            &self.auth.token,
            None,
            &[],
        )
        .await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        Ok(response
            .headers()
            .get("x-total")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u32>().ok()))
    }

    pub(super) async fn send_paginated<T: DeserializeOwned>(
        &self,
        path: &str,
        max_pages: usize,
    ) -> Result<Vec<T>> {
        let cap = max_pages.max(1);
        let mut out = Vec::new();
        let mut page: usize = 1;
        for _ in 0..cap {
            let paged = append_page_param(path, page);
            let response = send_with_redirects(
                &self.http,
                Method::GET,
                self.url(&paged)?,
                &self.origin,
                &self.auth.token,
                None,
                &[],
            )
            .await?;
            let status = response.status();
            let next_page = response
                .headers()
                .get("x-next-page")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_next_page);
            let bytes = response.bytes().await?;
            if !status.is_success() {
                return Err(GitLabError::from_status(
                    status,
                    String::from_utf8_lossy(&bytes).into_owned(),
                ));
            }
            let items: Vec<T> = serde_json::from_slice(&bytes)
                .map_err(|err| GitLabError::Message(err.to_string()))?;
            let empty = items.is_empty();
            out.extend(items);
            match next_page {
                Some(next) if next > page && !empty => page = next,
                _ => break,
            }
        }
        Ok(out)
    }

    pub async fn trace_raw(
        &self,
        project_id: u64,
        job_id: u64,
        offset: u64,
    ) -> Result<RawTraceChunk> {
        let headers = [(header::RANGE, format!("bytes={offset}-"))];
        let response = send_with_redirects(
            &self.http,
            Method::GET,
            self.url(&format!("/projects/{project_id}/jobs/{job_id}/trace"))?,
            &self.origin,
            &self.auth.token,
            None,
            &headers,
        )
        .await?;
        let status = response.status();
        if status == StatusCode::RANGE_NOT_SATISFIABLE {
            return Ok(RawTraceChunk {
                job_id,
                bytes: Vec::new(),
                next_offset: offset,
                complete: true,
            });
        }
        let range = response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(GitLabError::from_status(
                status,
                String::from_utf8_lossy(&bytes).into_owned(),
            ));
        }
        let (next_offset, fresh) = fresh_trace(offset, status, range.as_deref(), &bytes);
        Ok(RawTraceChunk {
            job_id,
            bytes: fresh.to_vec(),
            next_offset,
            complete: fresh.is_empty() && status != StatusCode::PARTIAL_CONTENT,
        })
    }
}

pub(crate) fn append_page_param(path: &str, page: usize) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}page={page}")
}

pub(crate) fn parse_next_page(raw: &str) -> Option<usize> {
    raw.trim().parse::<usize>().ok().filter(|&n| n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_next_page_header_and_builds_page_url() {
        assert_eq!(parse_next_page("2"), Some(2));
        assert_eq!(parse_next_page(" 3 "), Some(3));
        assert_eq!(parse_next_page(""), None);
        assert_eq!(parse_next_page("0"), None);
        assert_eq!(parse_next_page("abc"), None);
        assert_eq!(append_page_param("/projects/1/diffs?per_page=50", 2), "/projects/1/diffs?per_page=50&page=2");
        assert_eq!(append_page_param("/projects/1/diffs", 1), "/projects/1/diffs?page=1");
    }
}
