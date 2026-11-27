use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{InstallBlocker, InstallTarget, Result, UpdateError, APP_BUNDLE_NAME, APP_EXECUTABLE_NAME};

pub fn resolve_app_bundle_from_exe(exe_path: &Path) -> Option<PathBuf> {
    let macos_dir = exe_path.parent()?;
    if macos_dir.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name().and_then(|s| s.to_str()) != Some("Contents") {
        return None;
    }
    let bundle = contents_dir.parent()?;
    (bundle.extension().and_then(|s| s.to_str()) == Some("app")).then(|| bundle.to_path_buf())
}

pub fn detect_install_target_from_exe(exe_path: &Path) -> InstallTarget {
    let Some(bundle) = resolve_app_bundle_from_exe(exe_path) else {
        return InstallTarget::Blocked(InstallBlocker::NotAnAppBundle);
    };
    let path_str = bundle.to_string_lossy();
    if path_str.contains("/AppTranslocation/") {
        return InstallTarget::Blocked(InstallBlocker::AppTranslocated);
    }
    if path_str.starts_with("/Volumes/") {
        return InstallTarget::Blocked(InstallBlocker::ReadOnlyVolume);
    }
    let Some(parent) = bundle.parent() else {
        return InstallTarget::Blocked(InstallBlocker::NotWritable(bundle));
    };
    if !is_dir_writable(parent) {
        return InstallTarget::Blocked(InstallBlocker::NotWritable(parent.to_path_buf()));
    }
    InstallTarget::WritableBundle(bundle)
}

pub fn detect_current_install_target() -> InstallTarget {
    std::env::current_exe()
        .map(|exe| detect_install_target_from_exe(&exe))
        .unwrap_or(InstallTarget::Blocked(InstallBlocker::NotAnAppBundle))
}

fn is_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".gitlab-for-mac-write-probe-{}", std::process::id()));
    match fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn parse_codesign_team_id(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        let val = line.strip_prefix("TeamIdentifier=")?.trim();
        (!val.is_empty() && val != "not set").then(|| val.to_string())
    })
}

fn read_bundle_team_id(bundle: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/codesign").args(["-dvv"]).arg(bundle).output().ok()?;
    parse_codesign_team_id(&String::from_utf8_lossy(&out.stderr))
}

pub fn extract_and_verify_bundle(
    zip_path: &Path,
    staging_dir: &Path,
    current_bundle: &Path,
) -> Result<PathBuf> {
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir)?;
    }
    fs::create_dir_all(staging_dir)?;

    let ditto = Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(zip_path)
        .arg(staging_dir)
        .output()?;
    if !ditto.status.success() {
        return Err(UpdateError::Message(format!(
            "Failed to unpack update archive: {}",
            String::from_utf8_lossy(&ditto.stderr).trim()
        )));
    }

    let extracted = locate_extracted_app(staging_dir)?;
    let verify = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&extracted)
        .output()?;
    if !verify.status.success() {
        return Err(UpdateError::Message(format!(
            "Update bundle failed macOS code-signature verification: {}",
            String::from_utf8_lossy(&verify.stderr).trim()
        )));
    }

    if let Some(expected_team) = read_bundle_team_id(current_bundle) {
        let actual_team = read_bundle_team_id(&extracted);
        if actual_team.as_deref() != Some(expected_team.as_str()) {
            return Err(UpdateError::Message(format!(
                "Update signing team mismatch (expected {expected_team}, got {})",
                actual_team.as_deref().unwrap_or("unsigned/ad-hoc")
            )));
        }
    }
    Ok(extracted)
}

fn locate_extracted_app(staging_dir: &Path) -> Result<PathBuf> {
    let preferred = staging_dir.join(APP_BUNDLE_NAME);
    let candidate = if preferred.exists() {
        preferred
    } else {
        fs::read_dir(staging_dir)?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().and_then(|s| s.to_str()) == Some("app"))
            .ok_or_else(|| UpdateError::Message("No .app bundle found inside update archive".into()))?
    };

    let meta = fs::symlink_metadata(&candidate)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(UpdateError::Message("Extracted .app entry is not a valid bundle directory".into()));
    }
    let bin = candidate.join("Contents").join("MacOS").join(APP_EXECUTABLE_NAME);
    let plist = candidate.join("Contents").join("Info.plist");
    if !bin.is_file() || !plist.is_file() {
        return Err(UpdateError::Message(
            "Extracted .app bundle is missing Contents/MacOS executable or Info.plist".into(),
        ));
    }
    Ok(candidate)
}

const SWAP_SCRIPT: &str = r#"
pid="$1"; target="$2"; incoming="$3"; backup="$4"; cleanup="$5"
i=0
while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 150 ]; do sleep 0.1; i=$((i + 1)); done
if ! mv "$target" "$backup"; then rm -rf "$incoming"; exit 1; fi
if mv "$incoming" "$target"; then
  rm -rf "$backup" "$cleanup"
  /usr/bin/open -n "$target"
else
  mv "$backup" "$target" 2>/dev/null || true
  rm -rf "$incoming"
  exit 1
fi
"#;

pub fn stage_sibling_and_spawn_relaunch(
    current_bundle: &Path,
    extracted_bundle: &Path,
    cleanup_root: &Path,
    current_pid: u32,
) -> Result<()> {
    let parent = current_bundle
        .parent()
        .ok_or_else(|| UpdateError::Message("Current app bundle has no parent directory".into()))?;
    let name = current_bundle
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(APP_BUNDLE_NAME);
    let incoming = parent.join(format!(".{name}.incoming.{current_pid}"));
    let backup = parent.join(format!(".{name}.backup.{current_pid}"));
    if incoming.exists() {
        let _ = fs::remove_dir_all(&incoming);
    }

    let copy = Command::new("/usr/bin/ditto").arg(extracted_bundle).arg(&incoming).output()?;
    if !copy.status.success() {
        let _ = fs::remove_dir_all(&incoming);
        return Err(UpdateError::Message(format!(
            "Failed to stage updated bundle next to {}: {}",
            current_bundle.display(),
            String::from_utf8_lossy(&copy.stderr).trim()
        )));
    }

    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", SWAP_SCRIPT, "--", &current_pid.to_string()])
        .arg(current_bundle)
        .arg(&incoming)
        .arg(&backup)
        .arg(cleanup_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    cmd.process_group(0);
    cmd.spawn()?;
    Ok(())
}
