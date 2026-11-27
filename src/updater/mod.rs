//! Self-update checking, verified downloading, and macOS `.app` bundle replacement.
//!
//! Kept free of UI dependencies so release parsing, SHA-256 verification, and
//! bundle safety checks can be tested deterministically without a GUI window.

pub mod client;
pub mod download;
pub mod http;
pub mod install;
pub mod manifest;
pub mod version;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_GITHUB_API_BASE: &str = "https://api.github.com";
pub const DEFAULT_GITHUB_REPO: &str = "minhpham2309-micheal/gitlab-for-mac";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_SHORT_SHA: Option<&str> = option_env!("GITLAB_FOR_MAC_BUILD_SHA");
pub const APP_BUNDLE_NAME: &str = "GitLab for Mac.app";
pub const APP_EXECUTABLE_NAME: &str = "gitlab-for-mac";
pub const MAX_UPDATE_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
pub const AUTO_CHECK_INTERVAL_SECS: u64 = 4 * 60 * 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Canary,
}

impl UpdateChannel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::Canary => "Canary",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateConfig {
    pub api_base: String,
    pub repo: String,
    pub channel: UpdateChannel,
    pub current_version: String,
    pub current_arch: &'static str,
    pub etag: Option<String>,
    pub installed_canary_id: Option<String>,
    pub github_token: Option<String>,
    pub allow_gh_cli_fallback: bool,
}

impl UpdateConfig {
    pub fn for_current_app(
        channel: UpdateChannel,
        etag: Option<String>,
        installed_canary_id: Option<String>,
    ) -> Self {
        let env_or = |k: &str, default: &str| {
            std::env::var(k)
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| default.into())
        };
        Self {
            api_base: env_or("GITLAB_FOR_MAC_UPDATE_API_BASE", DEFAULT_GITHUB_API_BASE),
            repo: env_or("GITLAB_FOR_MAC_UPDATE_REPO", DEFAULT_GITHUB_REPO),
            channel,
            current_version: CURRENT_VERSION.into(),
            current_arch: current_macos_arch(),
            etag,
            installed_canary_id,
            github_token: None,
            allow_gh_cli_fallback: true,
        }
    }
}

pub const fn current_macos_arch() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "arm64"
    }
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "unknown"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub tag: String,
    pub version: String,
    pub name: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub html_url: String,
    pub asset_name: String,
    pub asset_download_url: String,
    pub asset_api_url: String,
    pub asset_size: u64,
    pub checksums_download_url: Option<String>,
    pub checksums_api_url: Option<String>,
    pub dmg_download_url: Option<String>,
    pub is_prerelease: bool,
    pub canary_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    UpToDate { current_version: String, etag: Option<String>, note: Option<String> },
    NotModified,
    Available { info: Box<UpdateInfo>, etag: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArchive {
    pub zip_path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallBlocker {
    NotAnAppBundle,
    AppTranslocated,
    ReadOnlyVolume,
    NotWritable(PathBuf),
}

impl InstallBlocker {
    pub fn explanation(&self) -> String {
        match self {
            Self::NotAnAppBundle => {
                "Running from a standalone/dev binary. Open GitHub Releases to download the packaged app.".into()
            }
            Self::AppTranslocated => {
                "App is running under macOS App Translocation. Move GitLab for Mac.app into /Applications to enable 1-click updates.".into()
            }
            Self::ReadOnlyVolume => {
                "App is running from a read-only disk image. Drag GitLab for Mac.app into /Applications first.".into()
            }
            Self::NotWritable(path) => format!(
                "Cannot write to {}. Install manually from GitHub Releases or adjust folder permissions.",
                path.display()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallTarget {
    WritableBundle(PathBuf),
    InstallToApplications {
        target_bundle: PathBuf,
        current_bundle: Option<PathBuf>,
        reason: InstallBlocker,
    },
    Blocked(InstallBlocker),
}

impl InstallTarget {
    pub fn destination_bundle(&self) -> Option<&std::path::Path> {
        match self {
            Self::WritableBundle(path) => Some(path.as_path()),
            Self::InstallToApplications { target_bundle, .. } => Some(target_bundle.as_path()),
            Self::Blocked(_) => None,
        }
    }

    pub fn current_bundle(&self) -> Option<&std::path::Path> {
        match self {
            Self::WritableBundle(path) => Some(path.as_path()),
            Self::InstallToApplications { current_bundle, .. } => current_bundle.as_deref(),
            Self::Blocked(_) => None,
        }
    }

    pub fn can_install_in_app(&self) -> bool {
        self.destination_bundle().is_some()
    }

    pub fn action_label(&self) -> &'static str {
        match self {
            Self::WritableBundle(_) => "Update & Restart",
            Self::InstallToApplications { .. } => "Install to Applications & Restart",
            Self::Blocked(_) => "Download Release",
        }
    }

    pub fn info_note(&self) -> Option<String> {
        match self {
            Self::WritableBundle(_) => None,
            Self::InstallToApplications { target_bundle, .. } => Some(format!(
                "Will install directly to {} and relaunch.",
                target_bundle.display()
            )),
            Self::Blocked(blocker) => Some(blocker.explanation()),
        }
    }
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("network request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, UpdateError>;

pub fn updates_cache_dir() -> Result<PathBuf> {
    dirs::cache_dir()
        .map(|dir| dir.join("gitlab-for-mac").join("updates"))
        .ok_or_else(|| UpdateError::Message("unable to determine macOS cache directory".into()))
}

pub fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
