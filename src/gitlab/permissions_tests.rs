use super::permissions::*;
use super::types::{MergeRequest, MrApprovals, User};
use super::types_project::ProjectDetail;

fn sample_mr(author_id: u64, state: &str, can_merge: Option<bool>) -> MergeRequest {
    MergeRequest {
        id: 1,
        iid: 12,
        title: "Fix".into(),
        state: state.into(),
        source_branch: "fix".into(),
        target_branch: "main".into(),
        web_url: "https://gitlab.example.com/acme/app/-/merge_requests/12".into(),
        merge_status: Some("can_be_merged".into()),
        has_conflicts: Some(false),
        author: Some(User {
            id: author_id,
            username: "author".into(),
            name: "Author".into(),
            avatar_url: None,
        }),
        description: None,
        labels: vec![],
        draft: false,
        created_at: None,
        updated_at: None,
        diff_refs: None,
        head_pipeline: None,
        reviewers: vec![],
        assignees: vec![],
        user_notes_count: 0,
        upvotes: 0,
        downvotes: 0,
        milestone: None,
        time_stats: None,
        detailed_merge_status: Some("mergeable".into()),
        diverged_commits_count: None,
        rebase_in_progress: None,
        should_remove_source_branch: None,
        force_remove_source_branch: None,
        squash: None,
        blocking_discussions_resolved: None,
        merged_by: None,
        merged_at: None,
        closed_by: None,
        closed_at: None,
        sha: None,
        user: can_merge.map(|v| MrUserPermissions { can_merge: Some(v) }),
    }
}

#[test]
fn from_detail_picks_max_of_project_and_group_access() {
    let raw: ProjectDetail = serde_json::from_str(
        r#"{
            "id": 1,
            "archived": false,
            "permissions": {
                "project_access": { "access_level": 20 },
                "group_access": { "access_level": 40 }
            }
        }"#,
    )
    .unwrap();
    let perms = ProjectPermissions::from_detail(&raw);
    assert!(perms.loaded);
    assert_eq!(perms.access_level, ACCESS_MAINTAINER);
    assert!(!perms.archived);
    assert!(perms.can_run_pipelines());
}

#[test]
fn unknown_and_archived_projects_fail_closed() {
    let mr = sample_mr(7, "opened", Some(true));
    let appr = MrApprovals { user_can_approve: true, ..MrApprovals::default() };

    let unknown = ProjectPermissions::unknown();
    assert!(!unknown.can_comment());
    assert!(!unknown.can_create_mr());
    assert!(!unknown.can_run_pipelines());
    assert_eq!(unknown.mr_permissions(&mr, Some(&appr), Some(7)), MrPermissions::default());

    let archived = ProjectPermissions::with_level(ACCESS_OWNER, true);
    assert!(!archived.can_comment());
    assert!(!archived.can_create_issue());
    assert!(!archived.can_run_pipelines());
    assert_eq!(archived.mr_permissions(&mr, Some(&appr), Some(7)), MrPermissions::default());
}

#[test]
fn developer_on_protected_branch_cannot_merge_but_can_edit_and_approve() {
    let dev = ProjectPermissions::with_level(ACCESS_DEVELOPER, false);
    let mr = sample_mr(99, "opened", Some(false));
    let appr = MrApprovals {
        user_can_approve: true,
        user_has_approved: false,
        ..MrApprovals::default()
    };
    let mp = dev.mr_permissions(&mr, Some(&appr), Some(7));
    assert!(!mp.can_merge);
    assert!(mp.can_approve);
    assert!(!mp.can_unapprove);
    assert!(mp.can_edit);
    assert!(mp.can_close);
    assert!(mp.can_rebase);
    assert!(mp.can_resolve_discussions);
    assert!(dev.can_run_pipelines());
}

#[test]
fn reporter_non_author_is_read_only_for_mr_mutations_and_pipelines() {
    let rep = ProjectPermissions::with_level(ACCESS_REPORTER, false);
    let mr = sample_mr(99, "opened", Some(false));
    let mp = rep.mr_permissions(&mr, None, Some(7));
    assert!(!rep.can_create_mr());
    assert!(!rep.can_run_pipelines());
    assert!(rep.can_manage_issue(false));
    assert!(!mp.can_merge);
    assert!(!mp.can_approve);
    assert!(!mp.can_edit);
    assert!(!mp.can_edit_metadata);
    assert!(!mp.can_resolve_discussions);
    assert!(mp.can_comment);
    assert!(mp.can_todo);

    let mp_author = rep.mr_permissions(&mr, None, Some(99));
    assert!(mp_author.can_edit);
    assert!(mp_author.can_close);
    assert!(!mp_author.can_merge);
}

#[test]
fn approval_revoke_and_closed_reopen_respect_state() {
    let maint = ProjectPermissions::with_level(ACCESS_MAINTAINER, false);
    let open_mr = sample_mr(7, "opened", Some(true));
    let approved = MrApprovals {
        user_can_approve: true,
        user_has_approved: true,
        ..MrApprovals::default()
    };
    let mp = maint.mr_permissions(&open_mr, Some(&approved), Some(7));
    assert!(mp.can_merge);
    assert!(!mp.can_approve);
    assert!(mp.can_unapprove);

    let closed_mr = sample_mr(7, "closed", Some(true));
    let mp_closed = maint.mr_permissions(&closed_mr, Some(&approved), Some(7));
    assert!(!mp_closed.can_merge);
    assert!(!mp_closed.can_approve);
    assert!(!mp_closed.can_unapprove);
    assert!(!mp_closed.can_close);
    assert!(mp_closed.can_reopen);
}
