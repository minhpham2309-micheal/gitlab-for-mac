use gpui_kit::{
    component::WindowExt as _,
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{
    Discussion, DiscussionNote, Job, MergeRequest, MrApprovals, MrFilter, MrListCounts,
    MrUserPermissions, Pipeline, PipelineTestSummary, PipelineTestTotals, Project,
    ProjectPermissions, RawDiff, RepoCommit, TestSuiteSummary, User, ACCESS_MAINTAINER,
};
use gitlab_for_mac::gpui_app::{
    actions::{NavBack, NavForward},
    init_app,
    nav_history::NavTarget,
    shell::AppShell,
    state::AppState,
};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_user() -> User {
    User { id: 7, username: "alice".into(), name: "Alice Dev".into(), avatar_url: None }
}

fn sample_project() -> Project {
    Project {
        id: 101,
        name: "demo-app".into(),
        path_with_namespace: "acme/backend/demo-app".into(),
        default_branch: Some("main".into()),
        web_url: "https://gitlab.example.com/acme/backend/demo-app".into(),
        last_activity_at: Some("2026-11-26T00:00:00Z".into()),
        star_count: 5,
        avatar_url: None,
    }
}

fn sample_mr() -> MergeRequest {
    MergeRequest {
        id: 500,
        iid: 53,
        title: "fix: run container as deploy user for log ownership".into(),
        state: "opened".into(),
        source_branch: "fix/container-user-log-ownership".into(),
        target_branch: "main".into(),
        web_url: "https://gitlab.example.com/acme/backend/demo-app/-/merge_requests/53".into(),
        merge_status: Some("can_be_merged".into()),
        has_conflicts: Some(false),
        author: Some(sample_user()),
        description: Some("Container-user fix:\n\n- Keep logs owned by deploy user.".into()),
        labels: vec![],
        draft: false,
        created_at: Some("2026-10-27T08:00:00Z".into()),
        updated_at: Some("2026-10-27T09:00:00Z".into()),
        diff_refs: None,
        head_pipeline: Some(Pipeline {
            id: 110118,
            iid: Some(8),
            status: "success".into(),
            git_ref: "refs/merge-requests/53/head".into(),
            sha: "9fabcb25112233445566".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/pipelines/110118".into(),
            created_at: Some("2026-10-27T08:30:00Z".into()),
            updated_at: Some("2026-10-27T08:35:00Z".into()),
            source: Some("merge_request_event".into()),
        }),
        reviewers: vec![],
        assignees: vec![],
        user_notes_count: 0,
        upvotes: 0,
        downvotes: 0,
        milestone: None,
        time_stats: None,
        detailed_merge_status: Some("mergeable".into()),
        diverged_commits_count: Some(2),
        rebase_in_progress: Some(false),
        should_remove_source_branch: Some(false),
        force_remove_source_branch: Some(false),
        squash: Some(false),
        blocking_discussions_resolved: Some(true),
        merged_by: None,
        merged_at: None,
        closed_by: None,
        closed_at: None,
        sha: Some("9fabcb25112233445566".into()),
        user: Some(MrUserPermissions { can_merge: Some(true) }),
    }
}

fn mount_mr_shell(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    cx.update(|cx| {
        let host = HostAuth {
            hostname: "gitlab.example.com".into(),
            api_host: "gitlab.example.com".into(),
            api_protocol: "https".into(),
            token: "glpat-fixture".into(),
            user: Some("alice".into()),
        };
        let state = cx.new(|_| {
            AppState::from_parts(vec![host.clone()], Some(host), vec![sample_project()], Prefs::default(), "Ready")
        });
        let bounds = Bounds::new(gpui_kit::point(px(0.), px(0.)), size(px(1400.), px(900.)));
        gpui_kit::open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            cx,
            move |window, cx| cx.new(|cx| AppShell::new(state.clone(), window, cx)),
        )
        .expect("open headless shell")
    })
}

#[gpui_kit::test]
fn mr_list_and_detail_match_gitlab_layout_and_navigation(cx: &mut TestAppContext) {
    let (window, shell) = mount_mr_shell(cx);
    let mr = sample_mr();
    let counts = MrListCounts { open: Some(1), merged: Some(51), closed: Some(2), all: Some(54) };
    let sys_disc = Discussion {
        id: "a1b2c3".into(),
        individual_note: true,
        notes: vec![DiscussionNote {
            id: 10,
            body: "added 1 commit\n\n<ul><li>36066823 - fix: reclaim legacy logs</li></ul>\n\n[Compare with previous version](/acme/backend/demo-app/-/merge_requests/53/diffs?diff_id=2)".into(),
            created_at: Some("2026-10-27T08:15:00Z".into()),
            author: Some(sample_user()),
            system: true,
            resolvable: false,
            resolved: false,
            position: None,
        }],
    };
    let commits = vec![RepoCommit {
        id: "3606682300000000".into(),
        short_id: "36066823".into(),
        title: "fix: reclaim legacy logs".into(),
        message: "fix: reclaim legacy logs".into(),
        author_name: "Alice Dev".into(),
        author_email: None,
        authored_date: Some("2026-10-27T08:15:00Z".into()),
        committed_date: None,
        created_at: None,
        web_url: None,
        last_pipeline: None,
    }];
    let jobs = vec![
        Job { id: 1, name: "lint".into(), status: "success".into(), stage: "test".into(), web_url: String::new(), duration: Some(5.0), started_at: None, finished_at: None, allow_failure: None },
        Job { id: 2, name: "build".into(), status: "success".into(), stage: "build".into(), web_url: String::new(), duration: Some(12.0), started_at: None, finished_at: None, allow_failure: None },
        Job { id: 3, name: "deploy".into(), status: "success".into(), stage: "deploy".into(), web_url: String::new(), duration: Some(8.0), started_at: None, finished_at: None, allow_failure: None },
    ];
    let diffs = vec![RawDiff {
        old_path: "deploy.sh".into(), new_path: "deploy.sh".into(),
        diff: "@@ -1,2 +1,3 @@\n #!/bin/sh\n+export APP_UID=1000\n echo ok\n".into(),
        new_file: false, renamed_file: false, deleted_file: false,
        too_large: false, collapsed: false, generated_file: false,
    }];

    // 1. Navigate to project Merge Requests and flush ProjectChanged before seeding fixtures.
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::MergeRequests, cx));
    })
    .unwrap();
    cx.run_until_parked();

    // 2. Verify full-width MR List View layout & filter tabs.
    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_MAINTAINER, false), cx);
        });
        let mrs_view = shell.read(cx).mrs_view().clone();
        mrs_view.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr.clone()], counts.clone(), None, vec![sys_disc.clone()], commits.clone(),
                mr.head_pipeline.clone().into_iter().collect(), jobs.clone(), diffs.clone(),
                Some(MrApprovals { user_can_approve: true, ..MrApprovals::default() }),
                vec![sample_user()], vec![], cx,
            );
        });
        win.render_frame(cx);

        let bc = win.find("mr-breadcrumb-bar").bounds();
        let tab_open = win.find("mr-tab-opened").bounds();
        let tab_merged = win.find("mr-tab-merged").bounds();
        let new_btn = win.find("mr-new-btn").bounds();
        let filter_bar = win.find("mr-filter-bar").bounds();
        let row = win.find("mr-list-row-53").bounds();
        let ci_badge = win.find("mr-list-ci-53").bounds();
        let updated = win.find("mr-list-updated-53").bounds();
        let footer = win.find("mr-list-footer").bounds();

        assert!(bc.bottom() <= tab_open.top());
        assert!(tab_open.right() <= tab_merged.left());
        assert!(tab_merged.right() <= new_btn.left());
        assert!(tab_open.bottom() <= filter_bar.top());
        assert!(filter_bar.bottom() <= row.top());
        assert_eq!(filter_bar.left(), row.left());
        assert_eq!(filter_bar.right(), row.right());
        assert!(ci_badge.right() < updated.left(), "CI status pill should sit beside updated label: ci={ci_badge:?} updated={updated:?}");
        let ci_mid = (ci_badge.top() + ci_badge.bottom()) / 2.0;
        let upd_mid = (updated.top() + updated.bottom()) / 2.0;
        assert!((ci_mid - upd_mid).abs() <= px(3.), "CI status pill and updated label must share the same row: ci={ci_badge:?} updated={updated:?}");
        assert!(ci_badge.top() >= row.top() && ci_badge.bottom() <= row.bottom());
        // Footer ("Show 20 items") sits directly below the last MR row, not pinned to window bottom.
        assert!(footer.top() >= row.bottom() && (footer.top() - row.bottom()) <= px(24.), "footer={footer:?} too far below row={row:?}");

        win.click("mr-tab-merged", cx);
        assert_eq!(mrs_view.read(cx).filter(), MrFilter::Merged);
        win.click("mr-tab-opened", cx);
        assert_eq!(mrs_view.read(cx).filter(), MrFilter::Open);

        // 2. Click row -> open full-width MR Detail View.
        win.click("mr-list-row-53", cx);
        // Re-hydrate fixture collections for offline mode after click cleared detail.
        mrs_view.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr.clone()], counts, Some(mr.clone()), vec![sys_disc], commits,
                mr.head_pipeline.clone().into_iter().collect(), jobs, diffs,
                Some(MrApprovals { user_can_approve: true, ..MrApprovals::default() }),
                vec![sample_user()], vec![], cx,
            );
        });
        win.render_frame(cx);

        assert_eq!(mrs_view.read(cx).selected_iid(), Some(53));
        assert_eq!(state.read(cx).history.current().target, NavTarget::MrDetail { iid: 53 });

        let title = win.find("mr-detail-title").bounds();
        let desc = win.find("mr-desc-body").bounds();
        let widgets = win.find("mr-widgets-stack").bounds();
        let activity = win.find("mr-activity-section").bounds();
        let sidebar = win.find("mr-metadata-sidebar").bounds();
        let tabs_right = win.find("mr-tabs-right").bounds();

        assert!(win.find("mr-add-todo").visible());
        assert!(win.find("mr-your-review-btn").visible());
        assert!(win.find("mr-widget-pipeline").visible());
        assert!(win.find("mr-widget-approval").visible());
        assert!(win.find("mr-widget-merge").visible());
        assert!(title.bottom() <= desc.top());
        assert!(desc.bottom() <= widgets.top());
        assert!(widgets.bottom() <= activity.top());
        // All main-column items inside virtualized list must span full main-column width cleanly up to sidebar gap.
        assert_eq!(desc.left(), title.left());
        assert_eq!(widgets.left(), title.left());
        assert_eq!(activity.left(), title.left());
        assert_eq!(desc.size.width, widgets.size.width, "desc width shrank vs widgets");
        assert_eq!(activity.size.width, widgets.size.width, "activity width shrank vs widgets");
        assert!((sidebar.left() - widgets.right() - px(24.)).abs() <= px(1.), "gap between widgets={widgets:?} and sidebar={sidebar:?} drifted");
        assert_eq!(sidebar.right(), tabs_right.right(), "sidebar right edge misaligned with header right edge");

        // 3. Switch detail tabs (Overview -> Commits -> Pipelines -> Changes -> Overview).
        win.click("mr-detail-tab-1", cx);
        win.render_frame(cx);
        assert_eq!(mrs_view.read(cx).active_tab(), 1);
        assert!(win.find("mr-commits-scroll").visible());

        win.click("mr-detail-tab-2", cx);
        win.render_frame(cx);
        assert_eq!(mrs_view.read(cx).active_tab(), 2);
        assert!(win.find("mr-pipelines-scroll").visible());

        win.click("mr-detail-tab-3", cx);
        win.render_frame(cx);
        assert_eq!(mrs_view.read(cx).active_tab(), 3);

        win.click("mr-detail-tab-0", cx);
        win.render_frame(cx);

        // 4. Assignee & Reviewer Edit pickers open inline member list and toggle selection.
        let bob = User { id: 9, username: "bob".into(), name: "Bob Reviewer".into(), avatar_url: None };
        mrs_view.update(cx, |v, cx| v.set_fixture_members(vec![sample_user(), bob], cx));
        win.render_frame(cx);

        win.click("mr-sb-edit-assignees", cx);
        win.render_frame(cx);
        assert!(win.find("mr-member-picker").visible());
        win.click("mr-member-opt-9", cx);
        win.render_frame(cx);
        assert_eq!(mrs_view.read(cx).selected_mr().unwrap().assignees.len(), 1);
        assert_eq!(mrs_view.read(cx).selected_mr().unwrap().assignees[0].id, 9);
        win.click("mr-member-unassign", cx);
        win.render_frame(cx);
        assert!(mrs_view.read(cx).selected_mr().unwrap().assignees.is_empty());

        win.click("mr-sb-edit-reviewers", cx);
        win.render_frame(cx);
        assert!(win.find("mr-member-picker").visible());
        win.click("mr-member-opt-9", cx);
        win.render_frame(cx);
        assert_eq!(mrs_view.read(cx).selected_mr().unwrap().reviewers.len(), 1);
        assert_eq!(mrs_view.read(cx).selected_mr().unwrap().reviewers[0].id, 9);
        win.click("mr-sb-edit-reviewers", cx);
        win.render_frame(cx);
        assert!(win.try_find("mr-member-picker").is_none());

        // 5. Edit button opens native Edit MR dialog.
        win.click("mr-edit-btn", cx);
        assert!(win.has_active_dialog(cx));
        win.close_all_dialogs(cx);

        // 5. Clicking breadcrumb "Merge requests" returns to List View.
        win.click("mr-bc-list", cx);
        win.render_frame(cx);
        assert_eq!(mrs_view.read(cx).selected_iid(), None);
        assert_eq!(state.read(cx).history.current().target, NavTarget::Screen);
        assert!(win.find("mr-list-row-53").visible());

        // 6. Browser-style Back/Forward returns to !53 and back to List.
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        win.dispatch_action(Box::new(NavBack), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let mrs_view = shell.read(cx).mrs_view().clone();
        assert_eq!(state.read(cx).history.current().target, NavTarget::MrDetail { iid: 53 }, "history did not navigate back");
        assert_eq!(mrs_view.read(cx).selected_iid(), Some(53), "mrs_view did not open !53 on NavBack");
        win.dispatch_action(Box::new(NavForward), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, _, cx| {
        let mrs_view = shell.read(cx).mrs_view().clone();
        assert_eq!(mrs_view.read(cx).selected_iid(), None);
    })
    .unwrap();
}

#[gpui_kit::test]
fn mr_detail_renders_test_summary_open_threads_auto_merge_and_composer(cx: &mut TestAppContext) {
    let (window, shell) = mount_mr_shell(cx);
    let mut mr = sample_mr();
    mr.detailed_merge_status = Some("discussions_not_resolved".into());
    mr.blocking_discussions_resolved = Some(false);
    let thread = Discussion {
        id: "thread99".into(),
        individual_note: false,
        notes: vec![DiscussionNote {
            id: 42,
            body: "LGTM".into(),
            created_at: Some("2026-10-27T08:20:00Z".into()),
            author: Some(sample_user()),
            system: false,
            resolvable: true,
            resolved: false,
            position: None,
        }],
    };
    let test_summary = PipelineTestSummary {
        total: PipelineTestTotals { time: 12.5, count: 684, success: 684, failed: 0, skipped: 0, error: 0 },
        test_suites: vec![TestSuiteSummary {
            name: "JUnit".into(), total_time: 12.5, total_count: 684,
            success_count: 684, failed_count: 0, skipped_count: 0, error_count: 0,
        }],
    };

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::MergeRequests, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_MAINTAINER, false), cx);
        });
        let mrs = shell.read(cx).mrs_view().clone();
        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr.clone()], MrListCounts::default(), Some(mr.clone()), vec![thread], vec![],
                mr.head_pipeline.clone().into_iter().collect(), vec![], vec![],
                Some(MrApprovals { user_can_approve: true, ..MrApprovals::default() }),
                vec![sample_user()], vec![], cx,
            );
            v.set_fixture_test_summary(Some(test_summary), cx);
        });
        win.render_frame(cx);

        assert!(win.find("mr-open-threads-nav").visible());
        assert!(win.find("mr-your-review-btn").visible());
        assert!(win.find("mr-widget-approve-btn").visible());
        assert!(win.find("mr-widget-test-summary").visible());
        assert!(win.find("mr-test-full-report").visible());
        assert!(win.find("mr-go-first-open-thread").visible());
        assert!(win.find("mr-widget-auto-merge-btn").visible());

        // Clicking "Go to first open thread" reveals the unresolved discussion card and inline Reply bar.
        win.click("mr-go-first-open-thread", cx);
        win.render_frame(cx);
        assert!(win.find("mr-disc-card-thread99").visible());
        assert!(win.find("rep-thread99").visible());
        win.click("rep-thread99", cx);
        win.render_frame(cx);

        // Reveal composer and toggle Write / Preview tabs.
        mrs.update(cx, |v, cx| v.jump_to_comment_composer(cx));
        win.render_frame(cx);
        assert!(win.find("mr-comment-composer").visible());
        win.click("mr-composer-preview", cx);
        win.render_frame(cx);
        win.click("mr-composer-write", cx);
        win.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn mr_list_ci_status_badge_supports_running_spinner_and_opens_pipeline(cx: &mut TestAppContext) {
    let (window, shell) = mount_mr_shell(cx);
    let mut mr = sample_mr();
    mr.iid = 247;
    if let Some(p) = mr.head_pipeline.as_mut() {
        p.id = 99001;
        p.status = "running".into();
    }

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::MergeRequests, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let mrs_view = shell.read(cx).mrs_view().clone();
        mrs_view.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr.clone()], MrListCounts::default(), None, vec![], vec![],
                vec![], vec![], vec![], None, vec![], vec![], cx,
            );
        });
        win.render_frame(cx);

        let ci = win.find("mr-list-ci-247").bounds();
        let updated = win.find("mr-list-updated-247").bounds();
        assert!(ci.right() < updated.left());
        win.click("mr-list-ci-247", cx);
        assert_eq!(mrs_view.read(cx).selected_iid(), None, "clicking CI pill must not open MR detail");
        assert_eq!(state.read(cx).screen, Screen::Pipelines, "clicking CI pill should open pipeline view");
    })
    .unwrap();
}
