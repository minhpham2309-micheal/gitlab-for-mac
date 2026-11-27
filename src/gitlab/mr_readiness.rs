use super::types::MergeRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeReadinessTone {
    Ready,
    Warning,
    Blocked,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeCheckKind {
    OpenThreads,
    Conflicts,
    Draft,
    PipelineFailed,
    PipelineRunning,
    NotApproved,
    NeedRebase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCheckFailure {
    pub kind: MergeCheckKind,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct MergeReadiness {
    pub tone: MergeReadinessTone,
    pub headline: String,
    pub can_merge_now: bool,
    pub can_auto_merge: bool,
    pub failed_checks: Vec<MergeCheckFailure>,
}

pub fn mr_merge_readiness(mr: &MergeRequest) -> MergeReadiness {
    mr_merge_readiness_with_threads(mr, 0)
}

pub fn mr_merge_readiness_with_threads(mr: &MergeRequest, unresolved_threads: usize) -> MergeReadiness {
    if mr.state == "merged" {
        let by = mr.merged_by.as_ref().map(|u| format!(" by {}", u.username)).unwrap_or_default();
        return MergeReadiness {
            tone: MergeReadinessTone::Merged, headline: format!("Merged{by} into {}.", mr.target_branch),
            can_merge_now: false, can_auto_merge: false, failed_checks: Vec::new(),
        };
    }
    if mr.state == "closed" {
        let by = mr.closed_by.as_ref().map(|u| format!(" by {}", u.username)).unwrap_or_default();
        return MergeReadiness {
            tone: MergeReadinessTone::Closed, headline: format!("Closed{by}."),
            can_merge_now: false, can_auto_merge: false, failed_checks: Vec::new(),
        };
    }
    let status = mr.detailed_merge_status.as_deref();
    let mut checks = Vec::new();
    if mr.draft {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::Draft, message: "Merge request must be marked as ready.".into() });
    }
    if mr.has_conflicts == Some(true) || status == Some("conflict") {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::Conflicts, message: "Merge conflicts must be resolved.".into() });
    }
    if status == Some("discussions_not_resolved") || mr.blocking_discussions_resolved == Some(false)
        || (unresolved_threads > 0 && status == Some("discussions_not_resolved"))
    {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::OpenThreads, message: "Open threads must be resolved.".into() });
    }
    if status == Some("ci_must_pass") {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::PipelineFailed, message: "Pipeline must succeed.".into() });
    } else if status == Some("ci_still_running") {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::PipelineRunning, message: "Pipeline must succeed.".into() });
    }
    if status == Some("not_approved") {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::NotApproved, message: "All required approvals must be given.".into() });
    }
    if status == Some("need_rebase") {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::NeedRebase, message: "Source branch must be rebased.".into() });
    }

    if !checks.is_empty() {
        let only_draft = checks.len() == 1 && checks[0].kind == MergeCheckKind::Draft;
        let only_running = checks.len() == 1 && checks[0].kind == MergeCheckKind::PipelineRunning;
        let hard_block = checks.iter().any(|c| matches!(c.kind, MergeCheckKind::Draft | MergeCheckKind::Conflicts | MergeCheckKind::PipelineFailed));
        let n = checks.len();
        let headline = if only_running {
            "Pipeline is running. Ready to merge once checks pass.".into()
        } else if n == 1 {
            "Merge blocked: 1 check failed".into()
        } else {
            format!("Merge blocked: {n} checks failed")
        };
        return MergeReadiness {
            tone: if only_draft || only_running { MergeReadinessTone::Warning } else { MergeReadinessTone::Blocked },
            headline,
            can_merge_now: false,
            can_auto_merge: !hard_block,
            failed_checks: checks,
        };
    }

    if matches!(status, Some("checking" | "unchecked" | "preparing")) {
        return MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Checking if merge request can be merged…".into(),
            can_merge_now: true, can_auto_merge: false, failed_checks: Vec::new(),
        };
    }
    MergeReadiness {
        tone: MergeReadinessTone::Ready,
        headline: "Ready to merge by members who can write to the target branch.".into(),
        can_merge_now: true, can_auto_merge: false, failed_checks: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitlab::types_mr::MrApprovals;

    #[test]
    fn parses_gitlab_html_and_markdown_commit_system_notes() {
        let html = "added 1 commit\n\n<ul><li>36066823 - fix: reclaim legacy logs</li></ul>\n\n[Compare with previous version](/team/app/-/merge_requests/5/diffs?diff_id=2)";
        let parsed = parse_system_note(html);
        assert_eq!(parsed.summary, "added 1 commit");
        assert_eq!(parsed.commits.len(), 1);
        assert_eq!(parsed.commits[0].sha, "36066823");
        assert_eq!(parsed.compare_url.as_deref(), Some("/team/app/-/merge_requests/5/diffs?diff_id=2"));

        let md = "added 2 commits\n\n- 88e3b2da - fix: prune archived logs\n- 9fabcb25 - chore: tidy";
        assert_eq!(parse_system_note(md).commits.len(), 2);
    }

    #[test]
    fn evaluates_merge_readiness_and_null_approvals() {
        let appr: MrApprovals = serde_json::from_str(
            r#"{"approved":false,"approvals_required":null,"approvals_left":null,"user_can_approve":true,"user_has_approved":false,"approved_by":[]}"#,
        ).unwrap();
        assert!(appr.user_can_approve);
        assert_eq!(appr.approvals_required, 0);

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
            merged_at: None, closed_by: None, closed_at: None, sha: None, user: None,
        };
        assert_eq!(mr_merge_readiness(&base).tone, MergeReadinessTone::Ready);

        let mut draft = base.clone();
        draft.draft = true;
        assert_eq!(mr_merge_readiness(&draft).tone, MergeReadinessTone::Warning);
        assert!(!mr_merge_readiness(&draft).can_merge_now);

        let mut threads = base.clone();
        threads.detailed_merge_status = Some("discussions_not_resolved".into());
        let tr = mr_merge_readiness_with_threads(&threads, 1);
        assert_eq!(tr.tone, MergeReadinessTone::Blocked);
        assert_eq!(tr.headline, "Merge blocked: 1 check failed");
        assert!(tr.can_auto_merge);
        assert_eq!(tr.failed_checks[0].kind, MergeCheckKind::OpenThreads);

        let mut conflict = base.clone();
        conflict.has_conflicts = Some(true);
        assert_eq!(mr_merge_readiness(&conflict).tone, MergeReadinessTone::Blocked);
        assert!(!mr_merge_readiness(&conflict).can_auto_merge);

        let mut merged = base.clone();
        merged.state = "merged".into();
        assert_eq!(mr_merge_readiness(&merged).tone, MergeReadinessTone::Merged);

        let mut closed = base;
        closed.state = "closed".into();
        assert_eq!(mr_merge_readiness(&closed).tone, MergeReadinessTone::Closed);
    }
}
