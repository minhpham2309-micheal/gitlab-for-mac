use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use reqwest::{header, Client, Method, Response, StatusCode};
use tokio::io::AsyncWriteExt;
use url::{Origin, Url};

use super::error::{GitLabError, Result};

const MAX_REDIRECTS: usize = 5;
pub const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
static PART_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn should_attach_token(target: &Url, auth_origin: &Origin) -> bool {
    matches!(target.scheme(), "http" | "https") && target.origin() == *auth_origin
}

pub fn next_allow_token(allow_token: bool, next_url: &Url, auth_origin: &Origin) -> bool {
    allow_token && should_attach_token(next_url, auth_origin)
}

pub fn resolve_redirect(current: &Url, location: &str) -> Result<Url> {
    let next = current
        .join(location)
        .map_err(|err| GitLabError::Message(format!("invalid redirect location: {err}")))?;
    if !matches!(next.scheme(), "http" | "https") {
        return Err(GitLabError::Message(format!(
            "blocked redirect to unsupported scheme '{}'",
            next.scheme()
        )));
    }
    Ok(next)
}

pub async fn send_with_redirects(
    client: &Client,
    method: Method,
    initial_url: Url,
    auth_origin: &Origin,
    token: &str,
    json_body: Option<Vec<u8>>,
    extra_headers: &[(header::HeaderName, String)],
) -> Result<Response> {
    send_with_redirects_checked(client, method, initial_url, auth_origin, token, json_body, extra_headers, |_| true).await
}

#[allow(clippy::too_many_arguments)]
pub async fn send_with_redirects_checked<F: Fn(&Url) -> bool>(
    client: &Client,
    mut method: Method,
    initial_url: Url,
    auth_origin: &Origin,
    token: &str,
    mut json_body: Option<Vec<u8>>,
    extra_headers: &[(header::HeaderName, String)],
    allow_url: F,
) -> Result<Response> {
    let mut url = initial_url;
    let mut allow_token = should_attach_token(&url, auth_origin);
    for _ in 0..=MAX_REDIRECTS {
        if !allow_url(&url) {
            return Err(GitLabError::Message(format!("blocked unsafe redirect target '{url}'")));
        }
        let mut req = client.request(method.clone(), url.clone());
        if allow_token && should_attach_token(&url, auth_origin) {
            req = req.header("PRIVATE-TOKEN", token);
        }
        for (name, value) in extra_headers {
            req = req.header(name, value);
        }
        if let Some(body) = &json_body {
            req = req.header(header::CONTENT_TYPE, "application/json").body(body.clone());
        }
        let response = req.send().await?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        let Some(location) = response.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()) else {
            return Ok(response);
        };
        let status = response.status();
        let next_url = resolve_redirect(&url, location)?;
        allow_token = next_allow_token(allow_token, &next_url, auth_origin);
        if status == StatusCode::SEE_OTHER
            || ((status == StatusCode::MOVED_PERMANENTLY || status == StatusCode::FOUND) && method == Method::POST)
        {
            method = Method::GET;
            json_body = None;
        } else if !allow_token {
            json_body = None;
        }
        url = next_url;
    }
    Err(GitLabError::Message("too many HTTP redirects".into()))
}

struct PartFileGuard(Option<PathBuf>);
impl Drop for PartFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn ensure_not_symlink(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(GitLabError::Message(format!("refusing to overwrite symlink {}", path.display())));
    }
    Ok(())
}

pub async fn stream_to_file(mut response: Response, path: &Path, max_bytes: u64) -> Result<u64> {
    ensure_not_symlink(path)?;
    let seq = PART_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("artifact");
    let part_path = path.with_file_name(format!(".{file_name}.{}.{seq}.part", std::process::id()));
    let mut guard = PartFileGuard(Some(part_path.clone()));

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part_path)
        .await
        .map_err(|err| GitLabError::Message(err.to_string()))?;

    let mut written: u64 = 0;
    while let Some(chunk) = response.chunk().await? {
        written = written.saturating_add(chunk.len() as u64);
        if written > max_bytes {
            return Err(GitLabError::Message(format!("artifact exceeds {} MB safety limit", max_bytes / (1024 * 1024))));
        }
        file.write_all(&chunk).await.map_err(|err| GitLabError::Message(err.to_string()))?;
    }
    file.flush().await.map_err(|err| GitLabError::Message(err.to_string()))?;
    drop(file);
    ensure_not_symlink(path)?;
    tokio::fs::rename(&part_path, path).await.map_err(|err| GitLabError::Message(err.to_string()))?;
    guard.0 = None;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_never_reattaches_after_cross_origin_hop() {
        let base = Url::parse("https://gitlab.example.com:8443/api/v4").unwrap();
        let origin = base.origin();
        let same = Url::parse("https://gitlab.example.com:8443/api/v4/projects").unwrap();
        let diff_port = Url::parse("https://gitlab.example.com:9443/api/v4/projects").unwrap();
        let external = Url::parse("https://attacker.example/bounce").unwrap();

        assert!(should_attach_token(&same, &origin));
        assert!(!should_attach_token(&diff_port, &origin));
        let hop1 = next_allow_token(true, &external, &origin);
        assert!(!hop1);
        assert!(!next_allow_token(hop1, &same, &origin));
    }

    #[test]
    fn resolve_redirect_blocks_non_http_schemes() {
        let base = Url::parse("https://gitlab.example.com/api/v4/projects").unwrap();
        let rel = resolve_redirect(&base, "/api/v4/other").unwrap();
        assert_eq!(rel.as_str(), "https://gitlab.example.com/api/v4/other");
        assert!(resolve_redirect(&base, "file:///etc/passwd").is_err());
        assert!(resolve_redirect(&base, "javascript:alert(1)").is_err());
    }
}
