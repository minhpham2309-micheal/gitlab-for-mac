use super::types::MergeRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeReadinessTone {
    Ready,
    Warning,
    Blocked,
    Merged,
    Closed,
}

#[derive(Debug, Clone)]
pub struct MergeReadiness {
    pub tone: MergeReadinessTone,
    pub headline: String,
    pub can_merge_now: bool,
}

pub fn mr_merge_readiness(mr: &MergeRequest) -> MergeReadiness {
    if mr.state == "merged" {
        let by = mr.merged_by.as_ref().map(|u| format!(" by {}", u.username)).unwrap_or_default();
        return MergeReadiness {
            tone: MergeReadinessTone::Merged,
            headline: format!("Merged{by} into {}.", mr.target_branch),
            can_merge_now: false,
        };
    }
    if mr.state == "closed" {
        let by = mr.closed_by.as_ref().map(|u| format!(" by {}", u.username)).unwrap_or_default();
        return MergeReadiness {
            tone: MergeReadinessTone::Closed,
            headline: format!("Closed{by}."),
            can_merge_now: false,
        };
    }
    if mr.draft {
        return MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Merge blocked: merge request must be marked as ready.".into(),
            can_merge_now: false,
        };
    }
    if mr.has_conflicts == Some(true) || mr.detailed_merge_status.as_deref() == Some("conflict") {
        return MergeReadiness {
            tone: MergeReadinessTone::Blocked,
            headline: "Merge blocked: merge conflicts must be resolved.".into(),
            can_merge_now: false,
        };
    }
    match mr.detailed_merge_status.as_deref() {
        Some("ci_still_running") => MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Pipeline is running. Ready to merge once checks pass.".into(),
            can_merge_now: true,
        },
        Some("ci_must_pass") => MergeReadiness {
            tone: MergeReadinessTone::Blocked,
            headline: "Merge blocked: pipeline must succeed.".into(),
            can_merge_now: false,
        },
        Some("discussions_not_resolved") => MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Merge blocked: all threads must be resolved.".into(),
            can_merge_now: false,
        },
        Some("not_approved") => MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Merge blocked: all required approvals must be given.".into(),
            can_merge_now: false,
        },
        Some("need_rebase") => MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Merge blocked: source branch must be rebased.".into(),
            can_merge_now: false,
        },
        Some("checking" | "unchecked" | "preparing") => MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Checking if merge request can be merged…".into(),
            can_merge_now: true,
        },
        _ => MergeReadiness {
            tone: MergeReadinessTone::Ready,
            headline: "Ready to merge by members who can write to the target branch.".into(),
            can_merge_now: true,
        },
    }
}

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

    // Extract <li>...</li> entries if GitLab returned HTML commit lists.
    let mut rest = body;
    while let Some(start) = rest.find("<li>") {
        let after = &rest[start + 4..];
        let Some(end) = after.find("</li>") else { break; };
        let item = after[..end].replace("<code>", "").replace("</code>", "");
        if let Some(entry) = split_commit_line(&item) {
            commits.push(entry);
        }
        rest = &after[end + 5..];
    }

    let had_html_commits = !commits.is_empty();
    for line in body.lines().map(str::trim) {
        if !had_html_commits && (line.starts_with("- ") || line.starts_with("* "))
            && let Some(entry) = split_commit_line(line) {
                commits.push(entry);
            }
        if let Some(idx) = line.find("[Compare with previous version](") {
            let after = &line[idx + "[Compare with previous version](".len()..];
            if let Some(end) = after.find(')') {
                compare_url = Some(after[..end].to_string());
            }
        }
    }

    ParsedSystemNote { summary, commits, compare_url }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gitlab_html_and_markdown_commit_system_notes() {
        let html = "added 1 commit\n\n<ul><li>36066823 - fix: reclaim legacy logs</li></ul>\n\n[Compare with previous version](/team/app/-/merge_requests/5/diffs?diff_id=2)";
        let parsed = parse_system_note(html);
        assert_eq!(parsed.summary, "added 1 commit");
        assert_eq!(parsed.commits.len(), 1);
        assert_eq!(parsed.commits[0].sha, "36066823");
        assert_eq!(parsed.commits[0].message, "fix: reclaim legacy logs");
        assert_eq!(parsed.compare_url.as_deref(), Some("/team/app/-/merge_requests/5/diffs?diff_id=2"));

        let md = "added 2 commits\n\n- 88e3b2da - fix: prune archived logs\n- 9fabcb25 - chore: tidy";
        let p2 = parse_system_note(md);
        assert_eq!(p2.commits.len(), 2);
        assert_eq!(p2.commits[1].sha, "9fabcb25");
    }

    #[test]
    fn evaluates_merge_readiness_across_gitlab_states() {
        let base = MergeRequest {
            id: 1, iid: 9, title: "Fix worker".into(), state: "opened".into(),
            source_branch: "fix/worker".into(), target_branch: "main".into(),
            web_url: "https://gitlab.example.com/team/app/-/merge_requests/9".into(),
            merge_status: Some("can_be_merged".into()), has_conflicts: Some(false),
            author: None, description: None, labels: vec![], draft: false,
            created_at: None, updated_at: None, diff_refs: None, head_pipeline: None,
            reviewers: vec![], assignees: vec![], user_notes_count: 0, upvotes: 0,
            downvotes: 0, milestone: None, time_stats: None, detailed_merge_status: None,
            diverged_commits_count: None, rebase_in_progress: None,
            should_remove_source_branch: None, force_remove_source_branch: None,
            squash: None, blocking_discussions_resolved: None, merged_by: None,
            merged_at: None, closed_by: None, closed_at: None, sha: None,
        };
        assert_eq!(mr_merge_readiness(&base).tone, MergeReadinessTone::Ready);

        let mut draft = base.clone();
        draft.draft = true;
        assert_eq!(mr_merge_readiness(&draft).tone, MergeReadinessTone::Warning);
        assert!(!mr_merge_readiness(&draft).can_merge_now);

        let mut conflict = base.clone();
        conflict.has_conflicts = Some(true);
        assert_eq!(mr_merge_readiness(&conflict).tone, MergeReadinessTone::Blocked);

        let mut merged = base.clone();
        merged.state = "merged".into();
        assert_eq!(mr_merge_readiness(&merged).tone, MergeReadinessTone::Merged);

        let mut closed = base;
        closed.state = "closed".into();
        assert_eq!(mr_merge_readiness(&closed).tone, MergeReadinessTone::Closed);
    }
}
