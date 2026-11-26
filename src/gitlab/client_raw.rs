use reqwest::{header, Method};

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::http_util::send_with_redirects;

pub const MAX_VIEWER_BLOB_BYTES: usize = 1_536 * 1024;

#[derive(Debug, Clone)]
pub struct RawBlobResponse {
    pub bytes: Vec<u8>,
    pub truncated: bool,
    pub content_type: Option<String>,
    pub blob_sha: Option<String>,
    pub reported_size: Option<usize>,
}

fn header_str(headers: &header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn header_usize(headers: &header::HeaderMap, name: &str) -> Option<usize> {
    header_str(headers, name).and_then(|s| s.parse::<usize>().ok())
}

impl GitLab {
    pub(super) async fn get_raw_capped(
        &self,
        path: &str,
        max_bytes: usize,
    ) -> Result<RawBlobResponse> {
        let mut response = send_with_redirects(
            &self.http,
            Method::GET,
            self.url(path)?,
            &self.origin,
            &self.auth.token,
            None,
            &[],
        )
        .await?;
        let status = response.status();
        let headers = response.headers().clone();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GitLabError::from_status(status, body));
        }

        let content_type = header_str(&headers, header::CONTENT_TYPE.as_str())
            .map(|ct| ct.to_ascii_lowercase());
        let blob_sha = header_str(&headers, "x-gitlab-blob-id");
        let reported_size = header_usize(&headers, "x-gitlab-size")
            .or_else(|| header_usize(&headers, header::CONTENT_LENGTH.as_str()));

        if reported_size.is_some_and(|sz| sz > max_bytes) {
            return Ok(RawBlobResponse {
                bytes: Vec::new(),
                truncated: true,
                content_type,
                blob_sha,
                reported_size,
            });
        }

        let mut bytes = Vec::with_capacity(reported_size.unwrap_or(0).min(max_bytes));
        let mut truncated = false;
        let mut seen_bytes = 0usize;
        while let Some(chunk) = response.chunk().await? {
            seen_bytes = seen_bytes.saturating_add(chunk.len());
            if seen_bytes > max_bytes {
                truncated = true;
                bytes.clear();
                break;
            }
            bytes.extend_from_slice(&chunk);
        }

        Ok(RawBlobResponse {
            bytes,
            truncated,
            content_type,
            blob_sha,
            reported_size: reported_size.or(Some(seen_bytes)),
        })
    }
}
