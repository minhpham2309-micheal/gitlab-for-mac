use super::api_mr_checks::MergeabilityCheck;
use super::types::MergeRequest;
use super::{mr_can_review, pipeline_is_live};

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
    Conflicts,
    OpenThreads,
    Draft,
    PipelineFailed,
    PipelineRunning,
    NotApproved,
    NeedRebase,
    Other,
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
    pub is_checking: bool,
    pub failed_checks: Vec<MergeCheckFailure>,
}

pub fn mr_merge_status_pending(mr: &MergeRequest, gql_checks: &[MergeabilityCheck]) -> bool {
    if !mr_can_review(&mr.state) {
        return false;
    }
    matches!(mr.detailed_merge_status.as_deref(), Some("checking" | "unchecked" | "preparing"))
        || matches!(mr.merge_status.as_deref(), Some("checking" | "unchecked" | "cannot_be_merged_recheck"))
        || gql_checks.iter().any(|c| c.status.eq_ignore_ascii_case("CHECKING"))
}

pub fn mr_merge_readiness(mr: &MergeRequest) -> MergeReadiness {
    mr_merge_readiness_full(mr, &[], 0)
}

pub fn mr_merge_readiness_with_threads(mr: &MergeRequest, unresolved_threads: usize) -> MergeReadiness {
    mr_merge_readiness_full(mr, &[], unresolved_threads)
}

fn map_gql_failed_check(mr: &MergeRequest, identifier: &str) -> Option<MergeCheckFailure> {
    let id = identifier.trim().to_ascii_uppercase();
    let (kind, msg): (MergeCheckKind, String) = match id.as_str() {
        "CONFLICT" => (MergeCheckKind::Conflicts, "Merge conflicts must be resolved.".into()),
        "DISCUSSIONS_NOT_RESOLVED" => (MergeCheckKind::OpenThreads, "Open threads must be resolved.".into()),
        "DRAFT_STATUS" => (MergeCheckKind::Draft, "Merge request must be marked as ready.".into()),
        "CI_MUST_PASS" => {
            let live = mr.detailed_merge_status.as_deref() == Some("ci_still_running")
                || mr.head_pipeline.as_ref().is_some_and(|p| pipeline_is_live(&p.status));
            let kind = if live { MergeCheckKind::PipelineRunning } else { MergeCheckKind::PipelineFailed };
            (kind, "Pipeline must succeed.".into())
        }
        "NOT_APPROVED" => (MergeCheckKind::NotApproved, "All required approvals must be given.".into()),
        "NEED_REBASE" => (MergeCheckKind::NeedRebase, "Source branch must be rebased.".into()),
        "REQUESTED_CHANGES" => (MergeCheckKind::Other, "Reviewers requested changes.".into()),
        "MERGE_REQUEST_BLOCKED" => (MergeCheckKind::Other, "Blocked by another merge request.".into()),
        "COMMITS_STATUS" => (MergeCheckKind::Other, "Source branch has no commits to merge.".into()),
        "BROKEN_STATUS" => (MergeCheckKind::Other, "Merge request is in a broken state.".into()),
        "LOCKED_PATHS" | "LOCKED_LFS_FILES" => (MergeCheckKind::Other, "Locked files must be unlocked before merging.".into()),
        "EXTERNAL_STATUS_CHECKS" | "STATUS_CHECKS_MUST_PASS" => (MergeCheckKind::Other, "External status checks must pass.".into()),
        "JIRA_ASSOCIATION_MISSING" => (MergeCheckKind::Other, "Jira issue reference is required.".into()),
        "NOT_OPEN" => return None,
        _ => (MergeCheckKind::Other, format!("Merge check failed: {}.", id.to_ascii_lowercase().replace('_', " "))),
    };
    Some(MergeCheckFailure { kind, message: msg })
}

fn collect_rest_checks(mr: &MergeRequest, pending: bool) -> Vec<MergeCheckFailure> {
    let status = mr.detailed_merge_status.as_deref();
    let mut checks = Vec::new();
    if mr.has_conflicts == Some(true) || status == Some("conflict") {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::Conflicts, message: "Merge conflicts must be resolved.".into() });
    }
    if mr.draft {
        checks.push(MergeCheckFailure { kind: MergeCheckKind::Draft, message: "Merge request must be marked as ready.".into() });
    }
    if status == Some("discussions_not_resolved") || mr.blocking_discussions_resolved == Some(false) {
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
    match status {
        Some("requested_changes") => checks.push(MergeCheckFailure { kind: MergeCheckKind::Other, message: "Reviewers requested changes.".into() }),
        Some("merge_request_blocked") => checks.push(MergeCheckFailure { kind: MergeCheckKind::Other, message: "Blocked by another merge request.".into() }),
        Some("broken_status") => checks.push(MergeCheckFailure { kind: MergeCheckKind::Other, message: "Merge request is in a broken state.".into() }),
        Some("commits_status") => checks.push(MergeCheckFailure { kind: MergeCheckKind::Other, message: "Source branch has no commits to merge.".into() }),
        _ => {}
    }
    if checks.is_empty() && !pending && mr.merge_status.as_deref() == Some("cannot_be_merged") {
        let (kind, msg) = if mr.has_conflicts == Some(false) {
            (MergeCheckKind::Other, "Merge request cannot be merged.".into())
        } else {
            (MergeCheckKind::Conflicts, "Merge conflicts must be resolved.".into())
        };
        checks.push(MergeCheckFailure { kind, message: msg });
    }
    checks
}

pub fn mr_merge_readiness_full(
    mr: &MergeRequest,
    gql_checks: &[MergeabilityCheck],
    _unresolved_threads: usize,
) -> MergeReadiness {
    if mr.state == "merged" {
        let by = mr.merged_by.as_ref().map(|u| format!(" by {}", u.username)).unwrap_or_default();
        return MergeReadiness {
            tone: MergeReadinessTone::Merged, headline: format!("Merged{by} into {}.", mr.target_branch),
            can_merge_now: false, can_auto_merge: false, is_checking: false, failed_checks: Vec::new(),
        };
    }
    if mr.state == "closed" {
        let by = mr.closed_by.as_ref().map(|u| format!(" by {}", u.username)).unwrap_or_default();
        return MergeReadiness {
            tone: MergeReadinessTone::Closed, headline: format!("Closed{by}."),
            can_merge_now: false, can_auto_merge: false, is_checking: false, failed_checks: Vec::new(),
        };
    }
    let pending = mr_merge_status_pending(mr, gql_checks);
    let mut checks = if !gql_checks.is_empty() {
        let mut out: Vec<MergeCheckFailure> = gql_checks
            .iter()
            .filter(|c| c.status.eq_ignore_ascii_case("FAILED"))
            .filter_map(|c| map_gql_failed_check(mr, &c.identifier))
            .collect();
        if (mr.has_conflicts == Some(true) || mr.detailed_merge_status.as_deref() == Some("conflict"))
            && !out.iter().any(|c| c.kind == MergeCheckKind::Conflicts)
        {
            out.insert(0, MergeCheckFailure { kind: MergeCheckKind::Conflicts, message: "Merge conflicts must be resolved.".into() });
        }
        out
    } else {
        collect_rest_checks(mr, pending)
    };
    if let Some(pos) = checks.iter().position(|c| c.kind == MergeCheckKind::Conflicts)
        && pos > 0
    {
        let conflict = checks.remove(pos);
        checks.insert(0, conflict);
    }

    if !checks.is_empty() {
        let only_draft = checks.len() == 1 && checks[0].kind == MergeCheckKind::Draft;
        let only_running = checks.len() == 1 && checks[0].kind == MergeCheckKind::PipelineRunning;
        let hard_block = checks.iter().any(|c| {
            matches!(c.kind, MergeCheckKind::Draft | MergeCheckKind::Conflicts | MergeCheckKind::PipelineFailed | MergeCheckKind::Other)
        });
        let n = checks.len();
        let headline = if only_running && !pending {
            "Pipeline is running. Ready to merge once checks pass.".into()
        } else if n == 1 {
            "Merge blocked: 1 check failed".into()
        } else {
            format!("Merge blocked: {n} checks failed")
        };
        return MergeReadiness {
            tone: if (only_draft || only_running) && !pending { MergeReadinessTone::Warning } else { MergeReadinessTone::Blocked },
            headline,
            can_merge_now: false,
            can_auto_merge: !hard_block && !pending,
            is_checking: pending,
            failed_checks: checks,
        };
    }
    if pending {
        return MergeReadiness {
            tone: MergeReadinessTone::Warning,
            headline: "Checking if merge request can be merged…".into(),
            can_merge_now: false, can_auto_merge: false, is_checking: true, failed_checks: Vec::new(),
        };
    }
    MergeReadiness {
        tone: MergeReadinessTone::Ready,
        headline: "Ready to merge by members who can write to the target branch.".into(),
        can_merge_now: true, can_auto_merge: false, is_checking: false, failed_checks: Vec::new(),
    }
}

pub use super::system_notes::{parse_system_note, ParsedSystemNote, SystemCommitEntry};
