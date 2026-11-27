use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use url::{Origin, Url};

use super::http::{build_client, get_with_origin_bound_auth, resolve_github_token};
use super::manifest::parse_sha256_manifest;
use super::{
    DownloadProgress, Result, UpdateConfig, UpdateError, UpdateInfo, VerifiedArchive,
    MAX_UPDATE_ARCHIVE_BYTES,
};

const MAX_CHECKSUM_BYTES: u64 = 256 * 1024;
static PART_SEQ: AtomicU64 = AtomicU64::new(1);

struct PartGuard(Option<PathBuf>);
impl Drop for PartGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

async fn fetch_expected_sha256(
    client: &Client,
    api_origin: &Origin,
    token: Option<&str>,
    info: &UpdateInfo,
) -> Result<Option<String>> {
    let Some(download_url) = info.checksums_download_url.as_deref() else {
        return Ok(None);
    };
    let (target_url, accept) = match (token, info.checksums_api_url.as_deref()) {
        (Some(_), Some(api_url)) if !api_url.is_empty() => (api_url, "application/octet-stream"),
        _ => (download_url, "*/*"),
    };
    let resp =
        get_with_origin_bound_auth(client, target_url, api_origin, token, accept, None).await?;
    if !resp.status().is_success() {
        return Err(UpdateError::Message(format!(
            "Failed to download SHA256SUMS.txt (HTTP {})",
            resp.status().as_u16()
        )));
    }
    let bytes = resp.bytes().await?;
    if bytes.len() as u64 > MAX_CHECKSUM_BYTES {
        return Err(UpdateError::Message("SHA256SUMS.txt exceeds size limit".into()));
    }
    let manifest = String::from_utf8_lossy(&bytes);
    let expected = parse_sha256_manifest(&manifest, &info.asset_name).ok_or_else(|| {
        UpdateError::Message(format!(
            "SHA256SUMS.txt does not contain a valid checksum for {}",
            info.asset_name
        ))
    })?;
    Ok(Some(expected))
}

pub async fn download_and_verify<F>(
    cfg: &UpdateConfig,
    info: &UpdateInfo,
    dest_dir: &Path,
    on_progress: F,
) -> Result<VerifiedArchive>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    tokio::fs::create_dir_all(dest_dir).await?;
    let client = build_client(Duration::from_secs(600))?;
    let api_origin = Url::parse(&cfg.api_base)
        .map_err(|e| UpdateError::Message(format!("invalid update API base: {e}")))?
        .origin();
    let mut token = cfg.github_token.clone();
    let mut resp = get_with_origin_bound_auth(
        &client,
        &info.asset_download_url,
        &api_origin,
        token.as_deref(),
        "*/*",
        None,
    )
    .await?;

    if resp.status() == StatusCode::NOT_FOUND && !info.asset_api_url.is_empty() {
        if token.is_none() {
            token = resolve_github_token(cfg).await;
        }
        if token.is_some() {
            resp = get_with_origin_bound_auth(
                &client,
                &info.asset_api_url,
                &api_origin,
                token.as_deref(),
                "application/octet-stream",
                None,
            )
            .await?;
        }
    }
    if !resp.status().is_success() {
        return Err(UpdateError::Message(format!(
            "Failed to download {} (HTTP {})",
            info.asset_name,
            resp.status().as_u16()
        )));
    }

    let expected_sha = fetch_expected_sha256(&client, &api_origin, token.as_deref(), info).await?;
    let total = resp
        .content_length()
        .or((info.asset_size > 0).then_some(info.asset_size));
    let safe_name = Path::new(&info.asset_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("update.app.zip");
    let final_path = dest_dir.join(safe_name);
    let seq = PART_SEQ.fetch_add(1, Ordering::Relaxed);
    let part_path = dest_dir.join(format!(".{safe_name}.{}.{seq}.part", std::process::id()));
    let mut guard = PartGuard(Some(part_path.clone()));

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part_path)
        .await?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();
    on_progress(DownloadProgress { downloaded_bytes: 0, total_bytes: total });

    while let Some(chunk) = resp.chunk().await? {
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > MAX_UPDATE_ARCHIVE_BYTES {
            return Err(UpdateError::Message("Update archive exceeds 256 MB safety limit".into()));
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        if last_emit.elapsed() >= Duration::from_millis(80) {
            on_progress(DownloadProgress { downloaded_bytes: downloaded, total_bytes: total });
            last_emit = Instant::now();
        }
    }
    file.flush().await?;
    drop(file);
    on_progress(DownloadProgress { downloaded_bytes: downloaded, total_bytes: total });

    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if let Some(expected) = expected_sha
        && digest != expected
    {
        return Err(UpdateError::Message(format!(
            "SHA-256 verification failed for {} (expected {}, got {})",
            info.asset_name,
            &expected[..12],
            &digest[..12]
        )));
    }

    tokio::fs::rename(&part_path, &final_path).await?;
    guard.0 = None;
    Ok(VerifiedArchive { zip_path: final_path, sha256: digest, bytes: downloaded })
}
