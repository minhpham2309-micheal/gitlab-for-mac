use std::path::Path;
use std::process::Command;

fn clean_short_sha(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let short: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .take(7)
        .collect();
    (short.len() == 7).then(|| short.to_ascii_lowercase())
}

fn resolve_short_sha() -> Option<String> {
    for var in ["GITLAB_FOR_MAC_BUILD_SHA", "GITHUB_SHA"] {
        if let Ok(val) = std::env::var(var)
            && let Some(sha) = clean_short_sha(&val)
        {
            return Some(sha);
        }
    }
    let out = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    clean_short_sha(&String::from_utf8_lossy(&out.stdout))
}

fn main() {
    println!("cargo:rerun-if-env-changed=GITLAB_FOR_MAC_BUILD_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    if Path::new(".git/HEAD").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
    }
    if let Some(sha) = resolve_short_sha() {
        println!("cargo:rustc-env=GITLAB_FOR_MAC_BUILD_SHA={sha}");
    }
}
