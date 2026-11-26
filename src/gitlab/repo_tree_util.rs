use std::collections::HashMap;

use super::client_raw::RawBlobResponse;
use super::types_project::SpecialRepoFiles;
use super::types_repo::{RepoBlob, RepoBlobKind, RepoTreeEntry};

const BINARY_SNIFF_BYTES: usize = 8192;

pub fn sort_tree_entries(entries: &mut [RepoTreeEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

pub fn is_image_path(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")
}

fn looks_like_image(path: &str, content_type: Option<&str>, bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { return Some("image/png".into()); }
    if bytes.starts_with(&[0xFF, 0xD8]) { return Some("image/jpeg".into()); }
    if bytes.starts_with(b"GIF8") { return Some("image/gif".into()); }
    if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp".into());
    }
    if path.to_ascii_lowercase().ends_with(".svg") {
        let head = std::str::from_utf8(&bytes[..bytes.len().min(512)]).unwrap_or("");
        if head.contains("<svg") { return Some("image/svg+xml".into()); }
        return None;
    }
    let ct = content_type.unwrap_or("");
    if ct.starts_with("image/") && !ct.contains("icon") && !bytes.starts_with(b"version https://git-lfs.github.com/") {
        return Some(ct.to_string());
    }
    None
}

pub fn is_probably_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(BINARY_SNIFF_BYTES)];
    if sample.contains(&0) { return true; }
    let ctrl = sample.iter().filter(|&&b| b < 0x09 || (b > 0x0d && b < 0x20)).count();
    !sample.is_empty() && ctrl * 10 > sample.len()
}

pub fn classify_blob(path: &str, resp: RawBlobResponse, max_bytes: usize) -> RepoBlob {
    let size_bytes = resp.reported_size.unwrap_or(resp.bytes.len());
    if resp.truncated {
        return RepoBlob { sha: resp.blob_sha, size_bytes, kind: RepoBlobKind::TooLarge { limit_bytes: max_bytes } };
    }
    if let Some(ct) = looks_like_image(path, resp.content_type.as_deref(), &resp.bytes) {
        return RepoBlob {
            sha: resp.blob_sha,
            size_bytes: resp.bytes.len(),
            kind: RepoBlobKind::Image { bytes: resp.bytes, content_type: ct },
        };
    }
    if is_probably_binary(&resp.bytes) {
        return RepoBlob { sha: resp.blob_sha, size_bytes: resp.bytes.len(), kind: RepoBlobKind::Binary };
    }
    let text = match String::from_utf8(resp.bytes) {
        Ok(s) => s,
        Err(err) => {
            let raw = err.into_bytes();
            let lossy = String::from_utf8_lossy(&raw);
            let bad = lossy.chars().take(4096).filter(|&c| c == '\u{FFFD}').count();
            if bad > 16 && bad * 20 > raw.len().min(4096) {
                return RepoBlob { sha: resp.blob_sha, size_bytes: raw.len(), kind: RepoBlobKind::Binary };
            }
            lossy.into_owned()
        }
    };
    RepoBlob { sha: resp.blob_sha, size_bytes: text.len(), kind: RepoBlobKind::Text(text) }
}

pub fn detect_special_files(
    entries: &[RepoTreeEntry],
    ci_config_path: Option<&str>,
) -> SpecialRepoFiles {
    let ci_target = ci_config_path.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(".gitlab-ci.yml");
    let mut out = SpecialRepoFiles::default();
    for entry in entries.iter().filter(|e| e.is_blob()) {
        let lower = entry.name.to_ascii_lowercase();
        if out.readme.is_none() && (lower == "readme" || lower.starts_with("readme.")) {
            out.readme = Some(entry.clone());
        }
        if out.ci_config.is_none() && (entry.path == ci_target || entry.name == ci_target) {
            out.ci_config = Some(entry.clone());
        }
        if out.license.is_none() && (lower == "license" || lower.starts_with("license.")) {
            out.license = Some(entry.clone());
        }
        if out.changelog.is_none() && (lower == "changelog" || lower.starts_with("changelog.")) {
            out.changelog = Some(entry.clone());
        }
        if out.contributing.is_none() && (lower == "contributing" || lower.starts_with("contributing.")) {
            out.contributing = Some(entry.clone());
        }
    }
    out
}

pub fn merge_last_commits(base: &mut [RepoTreeEntry], hydrated: &[RepoTreeEntry]) -> bool {
    let map: HashMap<&str, _> = hydrated
        .iter()
        .filter_map(|e| e.last_commit.as_ref().map(|c| (e.path.as_str(), c)))
        .collect();
    if map.is_empty() { return false; }
    let mut attached = false;
    for entry in base {
        if let Some(commit) = map.get(entry.path.as_str()) {
            entry.last_commit = Some((*commit).clone());
            attached = true;
        }
    }
    attached
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(name: &str, ty: &str) -> RepoTreeEntry {
        RepoTreeEntry {
            id: format!("sha-{name}"), name: name.into(), entry_type: ty.into(),
            path: name.into(), mode: "100644".into(), last_commit: None,
        }
    }

    #[test]
    fn sorts_dirs_before_files_case_insensitively() {
        let mut items = vec![mk("README.md", "blob"), mk("src", "tree"), mk(".env", "blob"), mk("App", "tree")];
        sort_tree_entries(&mut items);
        let names: Vec<&str> = items.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["App", "src", ".env", "README.md"]);
    }

    #[test]
    fn classifies_text_binary_image_and_oversize() {
        let png = RawBlobResponse {
            bytes: vec![0x89, b'P', b'N', b'G', 0, 1], truncated: false,
            content_type: None, blob_sha: Some("abc".into()), reported_size: Some(6),
        };
        assert!(matches!(classify_blob("logo.png", png, 1024).kind, RepoBlobKind::Image { .. }));

        let bin = RawBlobResponse {
            bytes: vec![b'E', b'L', b'F', 0x00, 0x01], truncated: false,
            content_type: None, blob_sha: None, reported_size: None,
        };
        assert!(matches!(classify_blob("bin.dat", bin, 1024).kind, RepoBlobKind::Binary));

        let big = RawBlobResponse {
            bytes: Vec::new(), truncated: true, content_type: None, blob_sha: None, reported_size: Some(5_000_000),
        };
        assert!(matches!(classify_blob("huge.json", big, 1024).kind, RepoBlobKind::TooLarge { .. }));
    }

    #[test]
    fn detects_special_files_from_root_entries() {
        let items = vec![mk("README.md", "blob"), mk(".gitlab-ci.yml", "blob"), mk("LICENSE", "blob")];
        let found = detect_special_files(&items, None);
        assert_eq!(found.readme.unwrap().name, "README.md");
        assert_eq!(found.ci_config.unwrap().name, ".gitlab-ci.yml");
        assert_eq!(found.license.unwrap().name, "LICENSE");
        assert!(found.changelog.is_none());
    }
}
