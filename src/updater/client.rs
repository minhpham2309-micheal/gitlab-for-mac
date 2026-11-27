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
        return handle_not_found(&client, cfg, &api_origin, token.as_deref()).await;
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
            note: None,
        }),
    }
}

async fn handle_not_found(
    client: &reqwest::Client,
    cfg: &UpdateConfig,
    api_origin: &url::Origin,
    token: Option<&str>,
) -> Result<UpdateCheckResult> {
    let list_url = format!(
        "{}/repos/{}/releases?per_page=5",
        cfg.api_base.trim_end_matches('/'),
        cfg.repo
    );
    if let Ok(list_resp) = get_with_origin_bound_auth(
        client,
        &list_url,
        api_origin,
        token,
        "application/vnd.github+json",
        None,
    )
    .await
        && list_resp.status().is_success()
    {
        let releases = list_resp.json::<Vec<GitHubRelease>>().await.unwrap_or_default();
        let note = match cfg.channel {
            UpdateChannel::Stable
                if releases.iter().any(|r| !r.draft && (r.prerelease || r.tag_name == "canary")) =>
            {
                "No Stable release published yet · Switch to Canary for rolling builds."
            }
            UpdateChannel::Stable => "No Stable release published yet.",
            UpdateChannel::Canary => "No Canary release published yet.",
        };
        return Ok(UpdateCheckResult::UpToDate {
            current_version: cfg.current_version.clone(),
            etag: None,
            note: Some(note.into()),
        });
    }
    Err(UpdateError::Message(
        "Unable to access GitHub Releases (private repo — run `gh auth login` or publish releases publicly)."
            .into(),
    ))
}
