use std::time::Duration;

use reqwest::{header, Client, Response};
use url::{Origin, Url};

use super::manifest::is_allowed_download_url;
use super::{Result, UpdateConfig, UpdateError, CURRENT_VERSION};

const MAX_REDIRECTS: usize = 6;

pub fn build_client(timeout: Duration) -> Result<Client> {
    Ok(Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .user_agent(format!("gitlab-for-mac/{CURRENT_VERSION}"))
        .build()?)
}

pub async fn get_with_origin_bound_auth(
    client: &Client,
    initial_url: &str,
    auth_origin: &Origin,
    token: Option<&str>,
    accept: &str,
    etag: Option<&str>,
) -> Result<Response> {
    let mut url = Url::parse(initial_url)
        .map_err(|e| UpdateError::Message(format!("invalid update URL: {e}")))?;
    if !is_allowed_download_url(url.as_str()) {
        return Err(UpdateError::Message(format!(
            "blocked non-HTTPS update URL '{}'",
            url.scheme()
        )));
    }
    let mut allow_token = token.is_some() && url.origin() == *auth_origin;

    for _ in 0..=MAX_REDIRECTS {
        let mut req = client.get(url.clone()).header(header::ACCEPT, accept);
        if allow_token
            && url.origin() == *auth_origin
            && let Some(tok) = token
        {
            req = req
                .header(header::AUTHORIZATION, format!("Bearer {tok}"))
                .header("X-GitHub-Api-Version", "2022-11-28");
        }
        if let Some(tag) = etag
            && url.origin() == *auth_origin
        {
            req = req.header(header::IF_NONE_MATCH, tag);
        }
        let resp = req.send().await?;
        if !resp.status().is_redirection() {
            return Ok(resp);
        }
        let Some(location) = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
        else {
            return Ok(resp);
        };
        let next = url
            .join(location)
            .map_err(|e| UpdateError::Message(format!("invalid redirect URL: {e}")))?;
        if !is_allowed_download_url(next.as_str()) {
            return Err(UpdateError::Message(format!(
                "blocked unsafe update redirect '{}'",
                next.scheme()
            )));
        }
        allow_token = allow_token && next.origin() == *auth_origin;
        url = next;
    }
    Err(UpdateError::Message("too many HTTP redirects".into()))
}

fn resolve_gh_binary() -> &'static str {
    for candidate in ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/opt/local/bin/gh"] {
        if std::path::Path::new(candidate).is_file() {
            return candidate;
        }
    }
    "gh"
}

pub async fn resolve_github_token(cfg: &UpdateConfig) -> Option<String> {
    if let Some(tok) = cfg
        .github_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(tok.to_string());
    }
    for var in ["GITLAB_FOR_MAC_GITHUB_TOKEN", "GITHUB_TOKEN"] {
        if let Ok(tok) = std::env::var(var)
            && !tok.trim().is_empty()
        {
            return Some(tok.trim().to_string());
        }
    }
    if !cfg.allow_gh_cli_fallback {
        return None;
    }
    let out = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::process::Command::new(resolve_gh_binary())
            .args(["auth", "token"])
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!token.is_empty()).then_some(token)
}
