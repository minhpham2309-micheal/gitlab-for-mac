use reqwest::Method;

use super::client::GitLab;
use super::client_raw::MAX_VIEWER_BLOB_BYTES;
use super::diff::RawDiff;
use super::error::{GitLabError, Result};
use super::http_util::send_with_redirects;
use super::repo_tree_util::{classify_blob, sort_tree_entries};
use super::types_repo::{RepoBlob, RepoTreeEntry};

const MAX_DIR_PAGES: usize = 5;

fn tree_query(project_id: u64, git_ref: &str, path: &str, with_last_commit: bool) -> String {
    let mut url = format!("/projects/{project_id}/repository/tree?per_page=100");
    if !git_ref.is_empty() { url.push_str(&format!("&ref={}", urlencoding::encode(git_ref))); }
    if !path.is_empty() { url.push_str(&format!("&path={}", urlencoding::encode(path))); }
    if with_last_commit { url.push_str("&with_last_commit=true"); }
    url
}

impl GitLab {
    pub async fn repo_tree(&self, project_id: u64, git_ref: &str, path: &str) -> Result<Vec<RepoTreeEntry>> {
        let base = tree_query(project_id, git_ref, path, false);
        let mut entries = self.send_paginated::<RepoTreeEntry>(&base, MAX_DIR_PAGES).await?;
        sort_tree_entries(&mut entries);
        Ok(entries)
    }

    pub async fn repo_tree_with_last_commit(
        &self, project_id: u64, git_ref: &str, path: &str,
    ) -> Result<Vec<RepoTreeEntry>> {
        let base = tree_query(project_id, git_ref, path, true);
        let mut entries = self.send_paginated::<RepoTreeEntry>(&base, MAX_DIR_PAGES).await?;
        sort_tree_entries(&mut entries);
        Ok(entries)
    }

    pub async fn repo_tree_recursive_capped(
        &self, project_id: u64, git_ref: &str, max_entries: usize,
    ) -> Result<(Vec<RepoTreeEntry>, bool)> {
        let cap = max_entries.max(100);
        let max_pages = cap.div_ceil(100).min(25);
        let mut out = Vec::new();
        let mut truncated = false;
        let mut page_token: Option<String> = None;

        for page in 1..=max_pages {
            let mut url = format!("/projects/{project_id}/repository/tree?recursive=true&per_page=100&pagination=keyset");
            if !git_ref.is_empty() { url.push_str(&format!("&ref={}", urlencoding::encode(git_ref))); }
            if let Some(tok) = page_token.as_deref() { url.push_str(&format!("&page_token={}", urlencoding::encode(tok))); }
            let response = send_with_redirects(
                &self.http, Method::GET, self.url(&url)?, &self.origin, &self.auth.token, None, &[],
            )
            .await?;
            let status = response.status();
            let next_link = response.headers().get("link").and_then(|v| v.to_str().ok()).and_then(extract_page_token);
            let has_offset_next = response
                .headers()
                .get("x-next-page")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|s| !s.trim().is_empty());
            let bytes = response.bytes().await?;
            if !status.is_success() {
                if page == 1 && status.as_u16() == 400 {
                    return self.repo_tree_recursive_offset(project_id, git_ref, cap).await;
                }
                return Err(GitLabError::from_status(status, String::from_utf8_lossy(&bytes).into_owned()));
            }
            if page == 1 && next_link.is_none() && has_offset_next {
                return self.repo_tree_recursive_offset(project_id, git_ref, cap).await;
            }
            let items: Vec<RepoTreeEntry> = serde_json::from_slice(&bytes)
                .map_err(|err| GitLabError::Message(err.to_string()))?;
            let empty = items.is_empty();
            out.extend(items);
            if out.len() > cap || (out.len() == cap && next_link.is_some()) {
                out.truncate(cap);
                truncated = true;
                break;
            }
            match next_link {
                Some(tok) if !empty => page_token = Some(tok),
                _ => break,
            }
        }
        Ok((out, truncated))
    }

    async fn repo_tree_recursive_offset(&self, project_id: u64, git_ref: &str, cap: usize) -> Result<(Vec<RepoTreeEntry>, bool)> {
        let mut base = format!("/projects/{project_id}/repository/tree?recursive=true&per_page=100");
        if !git_ref.is_empty() { base.push_str(&format!("&ref={}", urlencoding::encode(git_ref))); }
        let mut items = self.send_paginated::<RepoTreeEntry>(&base, cap.div_ceil(100).min(20)).await?;
        let truncated = items.len() >= cap;
        items.truncate(cap);
        Ok((items, truncated))
    }

    pub async fn repo_blob_raw(&self, project_id: u64, blob_sha: &str, path_hint: &str) -> Result<RepoBlob> {
        let url = format!("/projects/{project_id}/repository/blobs/{}/raw", urlencoding::encode(blob_sha));
        let mut resp = self.get_raw_capped(&url, MAX_VIEWER_BLOB_BYTES).await?;
        if resp.blob_sha.is_none() { resp.blob_sha = Some(blob_sha.to_string()); }
        Ok(classify_blob(path_hint, resp, MAX_VIEWER_BLOB_BYTES))
    }

    pub async fn repo_file_raw(&self, project_id: u64, path: &str, git_ref: &str) -> Result<RepoBlob> {
        let url = format!(
            "/projects/{project_id}/repository/files/{}/raw?ref={}",
            urlencoding::encode(path),
            urlencoding::encode(git_ref)
        );
        let resp = self.get_raw_capped(&url, MAX_VIEWER_BLOB_BYTES).await?;
        Ok(classify_blob(path, resp, MAX_VIEWER_BLOB_BYTES))
    }

    pub async fn repo_commit_diff(&self, project_id: u64, sha: &str) -> Result<Vec<RawDiff>> {
        let sha_enc = urlencoding::encode(sha);
        self.send_paginated(&format!("/projects/{project_id}/repository/commits/{sha_enc}/diff?per_page=100"), 5).await
    }
}

pub(crate) fn extract_page_token(link_header: &str) -> Option<String> {
    for part in link_header.split(',') {
        if !part.contains("rel=\"next\"") { continue; }
        let start = part.find('<')? + 1;
        let end = part[start..].find('>')? + start;
        let url = url::Url::parse(&part[start..end]).ok()?;
        for (k, v) in url.query_pairs() {
            if k == "page_token" && !v.is_empty() { return Some(v.into_owned()); }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_page_token;

    #[test]
    fn parses_keyset_next_page_token_from_link_header() {
        let link = "<https://gitlab.example/api/v4/projects/1/repository/tree?pagination=keyset&page_token=abc%2F123>; rel=\"next\"";
        assert_eq!(extract_page_token(link).as_deref(), Some("abc/123"));
        assert_eq!(extract_page_token("<https://gitlab.example/x>; rel=\"prev\""), None);
    }
}
