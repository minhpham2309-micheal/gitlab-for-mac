use super::api_mr_checks::MergeabilityCheck;
use super::mr_readiness::*;
use super::types::MergeRequest;
use super::types_mr::MrApprovals;

fn base_mr() -> MergeRequest {
    MergeRequest {
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
    }
}

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

    let base = base_mr();
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
    let cr = mr_merge_readiness(&conflict);
    assert_eq!(cr.tone, MergeReadinessTone::Blocked);
    assert!(!cr.can_auto_merge);
    assert_eq!(cr.failed_checks[0].kind, MergeCheckKind::Conflicts);

    let mut merged = base.clone();
    merged.state = "merged".into();
    assert_eq!(mr_merge_readiness(&merged).tone, MergeReadinessTone::Merged);

    let mut closed = base;
    closed.state = "closed".into();
    assert_eq!(mr_merge_readiness(&closed).tone, MergeReadinessTone::Closed);
}

#[test]
fn conflict_surfaces_first_when_short_circuited_by_open_threads_or_rechecking() {
    // GitLab REST detailed_merge_status short-circuits on discussions_not_resolved,
    // while GraphQL mergeabilityChecks reveals CONFLICT too.
    let mut mr = base_mr();
    mr.detailed_merge_status = Some("discussions_not_resolved".into());
    mr.blocking_discussions_resolved = Some(false);
    let gql = vec![
        MergeabilityCheck { identifier: "DISCUSSIONS_NOT_RESOLVED".into(), status: "FAILED".into() },
        MergeabilityCheck { identifier: "CONFLICT".into(), status: "FAILED".into() },
        MergeabilityCheck { identifier: "CI_MUST_PASS".into(), status: "SUCCESS".into() },
    ];
    let r = mr_merge_readiness_full(&mr, &gql, 1);
    assert_eq!(r.tone, MergeReadinessTone::Blocked);
    assert_eq!(r.headline, "Merge blocked: 2 checks failed");
    assert_eq!(r.failed_checks[0].kind, MergeCheckKind::Conflicts);
    assert_eq!(r.failed_checks[1].kind, MergeCheckKind::OpenThreads);
    assert!(!r.can_auto_merge);

    // When only CONFLICT fails in GraphQL (project does not require resolving threads),
    // do not misreport OpenThreads.
    let gql_conflict_only = vec![
        MergeabilityCheck { identifier: "DISCUSSIONS_NOT_RESOLVED".into(), status: "INACTIVE".into() },
        MergeabilityCheck { identifier: "CONFLICT".into(), status: "FAILED".into() },
    ];
    let r2 = mr_merge_readiness_full(&mr, &gql_conflict_only, 1);
    assert_eq!(r2.headline, "Merge blocked: 1 check failed");
    assert_eq!(r2.failed_checks.len(), 1);
    assert_eq!(r2.failed_checks[0].kind, MergeCheckKind::Conflicts);
    assert!(!r2.can_auto_merge);

    // Pending merge_status recheck blocks auto-merge and sets is_checking even if open threads exist.
    let mut recheck = base_mr();
    recheck.merge_status = Some("cannot_be_merged_recheck".into());
    recheck.blocking_discussions_resolved = Some(false);
    let r3 = mr_merge_readiness_full(&recheck, &[], 1);
    assert!(r3.is_checking);
    assert!(!r3.can_auto_merge);
    assert!(mr_merge_status_pending(&recheck, &[]));
}
