use url::Url;

pub fn is_allowed_download_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    match url.scheme() {
        "https" => true,
        "http" => matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
        ),
        _ => false,
    }
}

pub fn parse_sha256_manifest(manifest: &str, target_filename: &str) -> Option<String> {
    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, |c: char| c.is_ascii_whitespace());
        let hash = parts.next()?.trim();
        let rest = parts.next()?.trim();
        let trimmed = rest.strip_prefix('*').unwrap_or(rest);
        let file = trimmed.strip_prefix("./").unwrap_or(trimmed);
        if file == target_filename
            && hash.len() == 64
            && hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

pub fn clean_release_notes(raw: &str) -> String {
    let mut out = Vec::new();
    let mut in_checksums = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("### SHA-256 Checksums") {
            in_checksums = true;
            continue;
        }
        if in_checksums {
            if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                in_checksums = false;
            } else {
                continue;
            }
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}

pub fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const KIB: f64 = 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}
