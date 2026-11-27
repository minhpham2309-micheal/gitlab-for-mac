#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemCommitEntry {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSystemNote {
    pub summary: String,
    pub commits: Vec<SystemCommitEntry>,
    pub compare_url: Option<String>,
}

fn split_commit_line(raw: &str) -> Option<SystemCommitEntry> {
    let trimmed = raw.trim().trim_start_matches(['-', '*', '•']).trim();
    let (sha, msg) = trimmed.split_once(" - ")?;
    let sha_clean = sha.trim().trim_matches('`');
    if (7..=40).contains(&sha_clean.len()) && sha_clean.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(SystemCommitEntry { sha: sha_clean.to_string(), message: msg.trim().to_string() })
    } else {
        None
    }
}

pub fn parse_system_note(body: &str) -> ParsedSystemNote {
    let mut lines = body.lines().map(str::trim).filter(|l| !l.is_empty());
    let summary = lines.next().unwrap_or("").to_string();
    let mut commits = Vec::new();
    let mut compare_url = None;
    let mut rest = body;
    while let Some(start) = rest.find("<li>") {
        let after = &rest[start + 4..];
        let Some(end) = after.find("</li>") else { break; };
        let item = after[..end].replace("<code>", "").replace("</code>", "");
        if let Some(entry) = split_commit_line(&item) { commits.push(entry); }
        rest = &after[end + 5..];
    }
    let had_html_commits = !commits.is_empty();
    for line in body.lines().map(str::trim) {
        if !had_html_commits && (line.starts_with("- ") || line.starts_with("* "))
            && let Some(entry) = split_commit_line(line) { commits.push(entry); }
        if let Some(idx) = line.find("[Compare with previous version](") {
            let after = &line[idx + "[Compare with previous version](".len()..];
            if let Some(end) = after.find(')') { compare_url = Some(after[..end].to_string()); }
        }
    }
    ParsedSystemNote { summary, commits, compare_url }
}
