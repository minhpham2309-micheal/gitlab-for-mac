use std::time::Duration;

use reqwest::{header, StatusCode};
use url::Url;

pub use super::download::download_and_verify;
use super::http::{build_client, get_with_origin_bound_auth, resolve_github_token};
use super::version::{evaluate_release, GitHubRelease};
use super::{Result, UpdateChannel, UpdateCheckResult, UpdateConfig, UpdateError};

fn release_endpoint(cfg: &UpdateConfig) -> String {
    let base = cfg.api_base.trim_end_matches('/');
    match cfg.channel {
        UpdateChannel::Stable => format!("{base}/repos/{}/releases/latest", cfg.repo),
        UpdateChannel::Canary => format!("{base}/repos/{}/releases/tags/canary", cfg.repo),
    }
}

pub async fn check_for_update(cfg: &UpdateConfig) -> Result<UpdateCheckResult> {
    let client = build_client(Duration::from_secs(25))?;
    let endpoint = release_endpoint(cfg);
    let api_origin = Url::parse(&endpoint)
        .map_err(|e| UpdateError::Message(format!("invalid update API base: {e}")))?
        .origin();

    let mut token = cfg.github_token.clone();
    let mut resp = get_with_origin_bound_auth(
        &client,
        &endpoint,
        &api_origin,
        token.as_deref(),
        "application/vnd.github+json",
        cfg.etag.as_deref(),
    )
    .await?;

    if resp.status() == StatusCode::NOT_FOUND
        && token.is_none()
        && let Some(fallback_tok) = resolve_github_token(cfg).await
    {
        token = Some(fallback_tok);
        resp = get_with_origin_bound_auth(
            &client,
            &endpoint,
            &api_origin,
            token.as_deref(),
            "application/vnd.github+json",
            cfg.etag.as_deref(),
        )
        .await?;
    }

    if resp.status() == StatusCode::NOT_MODIFIED {
        return Ok(UpdateCheckResult::NotModified);
    }
    if resp.status() == StatusCode::NOT_FOUND {
        return Err(UpdateError::Message(
            "No published release found (or repository is private — run `gh auth login` or publish releases publicly)."
                .into(),
        ));
    }
    if !resp.status().is_success() {
        return Err(UpdateError::Message(format!(
            "GitHub Releases returned HTTP {}",
            resp.status().as_u16()
        )));
    }

    let etag = resp
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let release: GitHubRelease = resp.json().await?;
    match evaluate_release(
        &release,
        cfg.channel,
        &cfg.current_version,
        cfg.current_arch,
        cfg.installed_canary_id.as_deref(),
    ) {
        Some(info) => Ok(UpdateCheckResult::Available {
            info: Box::new(info),
            etag,
        }),
        None => Ok(UpdateCheckResult::UpToDate {
            current_version: cfg.current_version.clone(),
            etag,
        }),
    }
}
