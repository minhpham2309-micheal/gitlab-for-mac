use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use gitlab_for_mac::updater::client::{check_for_update, download_and_verify};
use gitlab_for_mac::updater::install::{
    detect_install_target_from_exe, parse_codesign_team_id, resolve_app_bundle_from_exe,
};
use gitlab_for_mac::updater::manifest::{
    clean_release_notes, format_bytes, is_allowed_download_url, parse_sha256_manifest,
};
use gitlab_for_mac::updater::version::{
    evaluate_release, extract_canary_id, is_newer_stable, select_release_assets, GitHubAsset,
    GitHubRelease,
};
use gitlab_for_mac::updater::{
    InstallBlocker, InstallTarget, UpdateChannel, UpdateCheckResult, UpdateConfig,
};

fn sample_release(tag: &str, prerelease: bool, draft: bool) -> GitHubRelease {
    GitHubRelease {
        tag_name: tag.into(),
        name: Some(format!("GitLab for Mac {tag}")),
        body: Some(
            "### SHA-256 Checksums\n```text\nabcd  file.zip\n```\n## Highlights\n- Faster diffs\n"
                .into(),
        ),
        html_url: format!("https://github.com/example/gitlab-for-mac/releases/tag/{tag}"),
        draft,
        prerelease,
        published_at: Some("2026-11-27T05:00:00Z".into()),
        assets: vec![
            GitHubAsset {
                name: format!("GitLab-for-Mac-{tag}-arm64.app.zip"),
                size: 12_345_678,
                url: "https://api.github.com/repos/example/gitlab-for-mac/releases/assets/1".into(),
                browser_download_url: format!(
                    "https://github.com/example/gitlab-for-mac/releases/download/{tag}/GitLab-for-Mac-{tag}-arm64.app.zip"
                ),
            },
            GitHubAsset {
                name: format!("GitLab-for-Mac-{tag}-arm64.dmg"),
                size: 13_000_000,
                url: "https://api.github.com/repos/example/gitlab-for-mac/releases/assets/2".into(),
                browser_download_url: format!(
                    "https://github.com/example/gitlab-for-mac/releases/download/{tag}/GitLab-for-Mac-{tag}-arm64.dmg"
                ),
            },
            GitHubAsset {
                name: "SHA256SUMS.txt".into(),
                size: 512,
                url: "https://api.github.com/repos/example/gitlab-for-mac/releases/assets/3".into(),
                browser_download_url: format!(
                    "https://github.com/example/gitlab-for-mac/releases/download/{tag}/SHA256SUMS.txt"
                ),
            },
        ],
    }
}

#[test]
fn semver_and_stable_release_evaluation_rules() {
    assert!(is_newer_stable("0.1.0", "v0.2.0"));
    assert!(is_newer_stable("0.1.0", "0.1.1"));
    assert!(is_newer_stable("0.2.0-rc.1", "v0.2.0"));
    assert!(!is_newer_stable("0.2.0", "v0.2.0"));
    assert!(!is_newer_stable("0.2.0", "v0.1.9"));
    assert!(!is_newer_stable("0.1.0", "canary"));

    let stable = sample_release("v0.2.0", false, false);
    let info = evaluate_release(&stable, UpdateChannel::Stable, "0.1.0", "arm64", None).unwrap();
    assert_eq!(info.version, "0.2.0");
    assert_eq!(info.asset_name, "GitLab-for-Mac-v0.2.0-arm64.app.zip");
    assert!(info.checksums_download_url.is_some());
    assert!(info.dmg_download_url.is_some());
    assert!(!info.notes.contains("SHA-256 Checksums"));
    assert!(info.notes.contains("Faster diffs"));

    // Prerelease and draft are ignored on Stable channel.
    assert!(evaluate_release(&sample_release("v0.2.0-rc.1", true, false), UpdateChannel::Stable, "0.1.0", "arm64", None).is_none());
    assert!(evaluate_release(&sample_release("v0.2.0", false, true), UpdateChannel::Stable, "0.1.0", "arm64", None).is_none());
    // Missing architecture asset returns None.
    assert!(evaluate_release(&stable, UpdateChannel::Stable, "0.1.0", "x86_64", None).is_none());
}

#[test]
fn canary_evaluation_detects_new_commit_and_skips_installed() {
    let mut canary = sample_release("canary", true, false);
    canary.name = Some("GitLab for Mac Canary (1212d63)".into());
    canary.body = Some("## Rolling Canary Build (`0.1.0-canary.1212d63`)\nChanges".into());

    assert_eq!(extract_canary_id(&canary).as_deref(), Some("canary-1212d63"));
    let info = evaluate_release(&canary, UpdateChannel::Canary, "0.1.0", "arm64", None).unwrap();
    assert_eq!(info.canary_id.as_deref(), Some("canary-1212d63"));
    assert_eq!(info.version, "0.1.0-canary.1212d63");

    // Once installed, the same canary ID is treated as up to date.
    assert!(evaluate_release(&canary, UpdateChannel::Canary, "0.1.0", "arm64", Some("canary-1212d63")).is_none());
}

#[test]
fn sha256_manifest_and_url_policy_validation() {
    let digest = "a".repeat(64);
    let manifest = format!(
        "# comment\n{digest}  GitLab-for-Mac-v0.2.0-arm64.app.zip\n{} *./GitLab-for-Mac-v0.2.0-x86_64.app.zip\nbadhash  short.zip\n",
        "B".repeat(64)
    );
    assert_eq!(
        parse_sha256_manifest(&manifest, "GitLab-for-Mac-v0.2.0-arm64.app.zip").as_deref(),
        Some(digest.as_str())
    );
    assert_eq!(
        parse_sha256_manifest(&manifest, "GitLab-for-Mac-v0.2.0-x86_64.app.zip").as_deref(),
        Some("b".repeat(64).as_str())
    );
    assert!(parse_sha256_manifest(&manifest, "short.zip").is_none());
    assert!(parse_sha256_manifest(&manifest, "missing.zip").is_none());

    assert!(is_allowed_download_url("https://github.com/example/release.zip"));
    assert!(is_allowed_download_url("http://127.0.0.1:8080/release.zip"));
    assert!(!is_allowed_download_url("http://evil.example.com/release.zip"));
    assert!(!is_allowed_download_url("file:///etc/passwd"));

    let unsafe_assets = vec![GitHubAsset {
        name: "GitLab-for-Mac-v0.2.0-arm64.app.zip".into(),
        size: 100,
        url: String::new(),
        browser_download_url: "http://insecure.example.com/app.zip".into(),
    }];
    assert!(select_release_assets(&unsafe_assets, "arm64").is_none());
    assert_eq!(clean_release_notes("### SHA-256 Checksums\n```\nabc\n```\n## Notes\nHi"), "## Notes\nHi");
    assert_eq!(format_bytes(1_572_864), "1.5 MB");
}

#[test]
fn install_target_and_codesign_team_detection() {
    assert_eq!(
        resolve_app_bundle_from_exe(Path::new("/Applications/GitLab for Mac.app/Contents/MacOS/gitlab-for-mac")),
        Some(PathBuf::from("/Applications/GitLab for Mac.app"))
    );
    assert!(resolve_app_bundle_from_exe(Path::new("/opt/build/target/debug/gitlab-for-mac")).is_none());
    assert_eq!(
        detect_install_target_from_exe(Path::new("/opt/build/target/debug/gitlab-for-mac")),
        InstallTarget::Blocked(InstallBlocker::NotAnAppBundle)
    );
    assert_eq!(
        detect_install_target_from_exe(Path::new(
            "/private/var/folders/aa/bb/T/AppTranslocation/UUID/d/GitLab for Mac.app/Contents/MacOS/gitlab-for-mac"
        )),
        InstallTarget::Blocked(InstallBlocker::AppTranslocated)
    );
    assert_eq!(
        detect_install_target_from_exe(Path::new(
            "/Volumes/GitLab for Mac/GitLab for Mac.app/Contents/MacOS/gitlab-for-mac"
        )),
        InstallTarget::Blocked(InstallBlocker::ReadOnlyVolume)
    );

    let tmp = std::env::temp_dir().join(format!("glab-upd-target-{}", std::process::id()));
    let bundle = tmp.join("GitLab for Mac.app");
    let exe = bundle.join("Contents/MacOS/gitlab-for-mac");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    assert_eq!(detect_install_target_from_exe(&exe), InstallTarget::WritableBundle(bundle));
    let _ = std::fs::remove_dir_all(&tmp);

    assert_eq!(parse_codesign_team_id("Authority=Dev\nTeamIdentifier=ABCDE12345\n"), Some("ABCDE12345".into()));
    assert_eq!(parse_codesign_team_id("Signature=adhoc\nTeamIdentifier=not set\n"), None);
}

async fn spawn_mock_http<F>(handler: F) -> String
where
    F: Fn(String) -> (u16, Vec<(String, String)>, Vec<u8>) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let h = Arc::clone(&handler);
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let (status, headers, body) = h(req);
                let mut resp = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                    body.len()
                );
                for (k, v) in headers {
                    resp.push_str(&format!("{k}: {v}\r\n"));
                }
                resp.push_str("\r\n");
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn check_download_verify_and_strip_token_on_cross_origin_redirect() {
    let payload = b"synthetic-signed-macos-app-zip-bytes-for-sha256".to_vec();
    let expected_sha: String = Sha256::digest(&payload).iter().map(|b| format!("{b:02x}")).collect();
    let saw_auth_on_cdn = Arc::new(AtomicBool::new(false));
    let saw_auth_clone = Arc::clone(&saw_auth_on_cdn);
    let payload_cdn = payload.clone();

    // Second local origin simulates GitHub CDN/S3 redirect target.
    let cdn_base = spawn_mock_http(move |req| {
        if req.to_ascii_lowercase().contains("authorization:") {
            saw_auth_clone.store(true, Ordering::SeqCst);
        }
        (200, vec![("Content-Type".into(), "application/octet-stream".into())], payload_cdn.clone())
    })
    .await;

    let cdn_for_api = cdn_base.clone();
    let sha_for_api = expected_sha.clone();
    let api_origin_slot: Arc<std::sync::OnceLock<String>> = Arc::new(std::sync::OnceLock::new());
    let api_origin_for_handler = Arc::clone(&api_origin_slot);
    let api_base = spawn_mock_http(move |req| {
        let first_line = req.lines().next().unwrap_or("");
        let self_base = api_origin_for_handler.get().map(String::as_str).unwrap_or("");
        if first_line.starts_with("GET /repos/example/app/releases/latest") {
            if req.contains("If-None-Match: \"etag-v2\"") || req.contains("if-none-match: \"etag-v2\"") {
                return (304, vec![], Vec::new());
            }
            let body = serde_json::json!({
                "tag_name": "v0.2.0",
                "name": "GitLab for Mac v0.2.0",
                "body": "New release!",
                "html_url": "https://github.com/example/app/releases/tag/v0.2.0",
                "draft": false,
                "prerelease": false,
                "published_at": "2026-11-27T06:00:00Z",
                "assets": [
                    {
                        "name": "GitLab-for-Mac-v0.2.0-arm64.app.zip",
                        "size": 47,
                        "url": format!("{cdn_for_api}/asset"),
                        "browser_download_url": format!("{self_base}/download/app.zip")
                    },
                    {
                        "name": "SHA256SUMS.txt",
                        "size": 100,
                        "url": "",
                        "browser_download_url": format!("{self_base}/download/SHA256SUMS.txt")
                    }
                ]
            })
            .to_string();
            return (200, vec![("ETag".into(), "\"etag-v2\"".into()), ("Content-Type".into(), "application/json".into())], body.into_bytes());
        }
        if first_line.starts_with("GET /download/app.zip") {
            return (302, vec![("Location".into(), format!("{cdn_for_api}/signed-asset"))], Vec::new());
        }
        if first_line.starts_with("GET /download/SHA256SUMS.txt") {
            let text = format!("{sha_for_api}  GitLab-for-Mac-v0.2.0-arm64.app.zip\n");
            return (200, vec![], text.into_bytes());
        }
        (404, vec![], Vec::new())
    })
    .await;
    api_origin_slot.set(api_base.clone()).unwrap();

    let cfg = UpdateConfig {
        api_base: api_base.clone(),
        repo: "example/app".into(),
        channel: UpdateChannel::Stable,
        current_version: "0.1.0".into(),
        current_arch: "arm64",
        etag: None,
        installed_canary_id: None,
        github_token: Some("test-bearer-fixture".into()),
        allow_gh_cli_fallback: false,
    };

    let res = check_for_update(&cfg).await.unwrap();
    let (info, etag) = match res {
        UpdateCheckResult::Available { info, etag } => (*info, etag),
        other => panic!("expected Available, got {other:?}"),
    };
    assert_eq!(etag.as_deref(), Some("\"etag-v2\""));

    // ETag 304 NotModified check.
    let mut cfg_etag = cfg.clone();
    cfg_etag.etag = etag;
    assert_eq!(check_for_update(&cfg_etag).await.unwrap(), UpdateCheckResult::NotModified);

    // Download + SHA-256 verification + cross-origin token stripping check.
    let dest = std::env::temp_dir().join(format!("glab-upd-dl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dest);
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_cb = Arc::clone(&ticks);
    let verified = download_and_verify(&cfg, &info, &dest, move |_| {
        ticks_cb.fetch_add(1, Ordering::Relaxed);
    })
    .await
    .unwrap();
    assert_eq!(verified.sha256, expected_sha);
    assert_eq!(verified.bytes, payload.len() as u64);
    assert!(verified.zip_path.is_file());
    assert!(ticks.load(Ordering::Relaxed) >= 2);
    assert!(!saw_auth_on_cdn.load(Ordering::SeqCst), "Authorization header leaked across origins!");

    // Corrupted checksum must fail closed and leave no partial file.
    let bad_base = spawn_mock_http(move |req| {
        if req.starts_with("GET /bad-sums") {
            let wrong = format!("{}  GitLab-for-Mac-v0.2.0-arm64.app.zip\n", "0".repeat(64));
            return (200, vec![], wrong.into_bytes());
        }
        (200, vec![], b"tampered-bytes".to_vec())
    })
    .await;
    let mut bad_info = info.clone();
    bad_info.asset_download_url = format!("{bad_base}/bad-zip");
    bad_info.checksums_download_url = Some(format!("{bad_base}/bad-sums"));
    let bad_dest = std::env::temp_dir().join(format!("glab-upd-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bad_dest);
    let err = download_and_verify(&cfg, &bad_info, &bad_dest, |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("SHA-256 verification failed"));
    assert_eq!(std::fs::read_dir(&bad_dest).unwrap().count(), 0);

    let _ = std::fs::remove_dir_all(&dest);
    let _ = std::fs::remove_dir_all(&bad_dest);
}

#[tokio::test]
async fn stable_404_with_canary_prerelease_returns_clear_channel_hint() {
    let api_base = spawn_mock_http(|req| {
        let first = req.lines().next().unwrap_or("");
        if first.starts_with("GET /repos/example/app/releases/latest") {
            return (404, vec![], br#"{"message":"Not Found"}"#.to_vec());
        }
        if first.starts_with("GET /repos/example/app/releases?per_page=5") {
            let body = serde_json::json!([
                { "tag_name": "canary", "name": "Canary", "draft": false, "prerelease": true, "assets": [] }
            ])
            .to_string();
            return (200, vec![("Content-Type".into(), "application/json".into())], body.into_bytes());
        }
        (404, vec![], Vec::new())
    })
    .await;

    let cfg = UpdateConfig {
        api_base,
        repo: "example/app".into(),
        channel: UpdateChannel::Stable,
        current_version: "0.1.0".into(),
        current_arch: "arm64",
        etag: None,
        installed_canary_id: None,
        github_token: None,
        allow_gh_cli_fallback: false,
    };
    let res = check_for_update(&cfg).await.unwrap();
    match res {
        UpdateCheckResult::UpToDate { note: Some(note), .. } => {
            assert!(note.contains("Canary"), "unexpected note: {note}");
        }
        other => panic!("expected UpToDate with Canary hint, got {other:?}"),
    }
}

#[cfg(target_os = "macos")]
#[test]
fn extract_and_verify_real_adhoc_signed_macos_bundle() {
    use std::process::Command;

    let root = std::env::temp_dir().join(format!("glab-upd-bundle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src_app = root.join("src/GitLab for Mac.app");
    let macos_dir = src_app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos_dir).unwrap();
    let bin = macos_dir.join("gitlab-for-mac");
    std::fs::copy("/bin/echo", &bin).unwrap();
    std::fs::write(
        src_app.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>gitlab-for-mac</string>
<key>CFBundleIdentifier</key><string>dev.gitlabformac.test</string>
</dict></plist>"#,
    )
    .unwrap();
    assert!(Command::new("/usr/bin/codesign").args(["--force", "--sign", "-"]).arg(&bin).status().unwrap().success());
    assert!(Command::new("/usr/bin/codesign").args(["--force", "--sign", "-"]).arg(&src_app).status().unwrap().success());

    let zip = root.join("GitLab-for-Mac-v0.2.0-arm64.app.zip");
    assert!(Command::new("/usr/bin/ditto").args(["-c", "-k", "--keepParent"]).arg(&src_app).arg(&zip).status().unwrap().success());

    let staging = root.join("extracted");
    let extracted = gitlab_for_mac::updater::install::extract_and_verify_bundle(&zip, &staging, &src_app).unwrap();
    assert!(extracted.join("Contents/MacOS/gitlab-for-mac").is_file());
    let _ = std::fs::remove_dir_all(&root);
}
