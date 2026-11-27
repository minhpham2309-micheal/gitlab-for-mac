use semver::Version;
use serde::Deserialize;

use super::manifest::{clean_release_notes, is_allowed_download_url};
use super::{UpdateChannel, UpdateInfo};

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub browser_download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubAssetSummary {
    pub name: String,
    pub size: u64,
    pub api_url: String,
    pub download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedAssets {
    pub app_zip: GitHubAssetSummary,
    pub checksums: Option<GitHubAssetSummary>,
    pub dmg_url: Option<String>,
}

pub fn parse_semver_tag(raw: &str) -> Option<Version> {
    let trimmed = raw.trim();
    Version::parse(trimmed.strip_prefix('v').unwrap_or(trimmed)).ok()
}

pub fn is_newer_stable(current_version: &str, candidate_tag: &str) -> bool {
    matches!(
        (parse_semver_tag(current_version), parse_semver_tag(candidate_tag)),
        (Some(cur), Some(cand)) if cand > cur
    )
}

pub fn extract_canary_id(release: &GitHubRelease) -> Option<String> {
    if let Some(body) = release.body.as_deref()
        && let Some(start) = body.find("-canary.")
    {
        let sha: String = body[start + 8..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if sha.len() >= 7 {
            return Some(format!("canary-{sha}"));
        }
    }
    if let Some(name) = release.name.as_deref()
        && let Some(open) = name.rfind('(')
        && let Some(close) = name[open + 1..].find(')')
    {
        let token = name[open + 1..open + 1 + close].trim();
        if token.len() >= 7 && token.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Some(format!("canary-{token}"));
        }
    }
    release
        .published_at
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|ts| format!("canary-{ts}"))
}

fn summarize(a: &GitHubAsset) -> GitHubAssetSummary {
    GitHubAssetSummary {
        name: a.name.clone(),
        size: a.size,
        api_url: a.url.clone(),
        download_url: a.browser_download_url.clone(),
    }
}

pub fn select_release_assets(assets: &[GitHubAsset], arch: &str) -> Option<SelectedAssets> {
    let zip_suffix = format!("-{arch}.app.zip");
    let dmg_suffix = format!("-{arch}.dmg");
    let app_zip = assets
        .iter()
        .find(|a| a.name.ends_with(&zip_suffix) && is_allowed_download_url(&a.browser_download_url))
        .map(summarize)?;
    let checksums = assets
        .iter()
        .find(|a| a.name == "SHA256SUMS.txt" && is_allowed_download_url(&a.browser_download_url))
        .map(summarize);
    let dmg_url = assets
        .iter()
        .find(|a| a.name.ends_with(&dmg_suffix) && is_allowed_download_url(&a.browser_download_url))
        .map(|a| a.browser_download_url.clone());
    Some(SelectedAssets { app_zip, checksums, dmg_url })
}

pub fn evaluate_release(
    release: &GitHubRelease,
    channel: UpdateChannel,
    current_version: &str,
    arch: &str,
    installed_canary_id: Option<&str>,
) -> Option<UpdateInfo> {
    if release.draft {
        return None;
    }
    let (version, canary_id) = match channel {
        UpdateChannel::Stable => {
            if release.prerelease || !is_newer_stable(current_version, &release.tag_name) {
                return None;
            }
            (release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name).to_string(), None)
        }
        UpdateChannel::Canary if release.tag_name == "canary" => {
            let id = extract_canary_id(release)?;
            let built_sha = option_env!("GITLAB_FOR_MAC_BUILD_SHA");
            if installed_canary_id == Some(id.as_str())
                || built_sha.is_some_and(|sha| id == format!("canary-{sha}"))
            {
                return None;
            }
            let label = id
                .strip_prefix("canary-")
                .map(|s| format!("{current_version}-canary.{s}"))
                .unwrap_or_else(|| id.clone());
            (label, Some(id))
        }
        UpdateChannel::Canary => {
            if !is_newer_stable(current_version, &release.tag_name) {
                return None;
            }
            (release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name).to_string(), None)
        }
    };

    let selected = select_release_assets(&release.assets, arch)?;
    let name = release
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&release.tag_name)
        .to_string();

    Some(UpdateInfo {
        tag: release.tag_name.clone(),
        version,
        name,
        notes: clean_release_notes(release.body.as_deref().unwrap_or_default()),
        published_at: release.published_at.clone(),
        html_url: release.html_url.clone(),
        asset_name: selected.app_zip.name,
        asset_download_url: selected.app_zip.download_url,
        asset_api_url: selected.app_zip.api_url,
        asset_size: selected.app_zip.size,
        checksums_download_url: selected.checksums.as_ref().map(|c| c.download_url.clone()),
        checksums_api_url: selected.checksums.as_ref().map(|c| c.api_url.clone()),
        dmg_download_url: selected.dmg_url,
        is_prerelease: release.prerelease,
        canary_id,
    })
}
