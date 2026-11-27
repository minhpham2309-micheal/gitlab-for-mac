use gpui_kit::{
    component::WindowExt as _,
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{
    MergeRequest, MrApprovals, MrListCounts, MrUserPermissions, Project, ProjectPermissions, User,
    ACCESS_DEVELOPER, ACCESS_MAINTAINER, ACCESS_OWNER, ACCESS_REPORTER,
};
use gitlab_for_mac::gpui_app::{
    actions::{CloseMrAction, MergeMrAction},
    init_app,
    shell::AppShell,
    state::AppState,
};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

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

fn sample_mr(author_id: u64, can_merge: Option<bool>) -> MergeRequest {
    MergeRequest {
        id: 500,
        iid: 53,
        title: "fix: enforce deploy user permissions".into(),
        state: "opened".into(),
        source_branch: "fix/deploy-perms".into(),
        target_branch: "main".into(),
        web_url: "https://gitlab.example.com/acme/backend/demo-app/-/merge_requests/53".into(),
        merge_status: Some("can_be_merged".into()),
        has_conflicts: Some(false),
        author: Some(User {
            id: author_id,
            username: "author".into(),
            name: "Author Dev".into(),
            avatar_url: None,
        }),
        description: Some("Verify permission gating.".into()),
        labels: vec![],
        draft: false,
        created_at: Some("2026-10-27T08:00:00Z".into()),
        updated_at: Some("2026-10-27T09:00:00Z".into()),
        diff_refs: None,
        head_pipeline: None,
        reviewers: vec![],
        assignees: vec![],
        user_notes_count: 0,
        upvotes: 1,
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
        user: can_merge.map(|v| MrUserPermissions { can_merge: Some(v) }),
    }
}

fn mount_shell(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    cx.update(|cx| {
        let host = HostAuth {
            hostname: "gitlab.example.com".into(),
            api_host: "gitlab.example.com".into(),
            api_protocol: "https".into(),
            token: "glpat-fixture".into(),
            user: Some("viewer".into()),
        };
        let state = cx.new(|_| {
            let mut s = AppState::from_parts(
                vec![host.clone()],
                Some(host),
                vec![sample_project()],
                Prefs::default(),
                "Ready",
            );
            s.current_user = Some(User {
                id: 7,
                username: "viewer".into(),
                name: "Viewer User".into(),
                avatar_url: None,
            });
            s
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
fn mr_controls_hide_when_user_lacks_permission_and_show_when_permitted(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    let counts = MrListCounts { open: Some(1), merged: Some(0), closed: Some(0), all: Some(1) };

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::MergeRequests, cx));
    })
    .unwrap();
    cx.run_until_parked();

    // 1. Maintainer with can_merge=true & user_can_approve=true sees every control.
    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let mrs = shell.read(cx).mrs_view().clone();
        let mr_full = sample_mr(99, Some(true));
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_MAINTAINER, false), cx);
        });
        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr_full.clone()], counts.clone(), None, vec![], vec![], vec![], vec![],
                vec![], None, vec![], vec![], cx,
            );
        });
        win.render_frame(cx);
        assert!(win.find("mr-new-btn").visible());
        assert!(win.find("mr-bulk-edit").visible());

        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr_full.clone()], counts.clone(), Some(mr_full), vec![], vec![], vec![],
                vec![], vec![],
                Some(MrApprovals { user_can_approve: true, ..MrApprovals::default() }),
                vec![], vec![], cx,
            );
        });
        win.render_frame(cx);
        assert!(win.find("mr-edit-btn").visible());
        assert!(win.find("mr-add-todo").visible());
        assert!(win.find("mr-widget-approve-btn").visible());
        assert!(win.find("mr-widget-merge-btn").visible());
        assert!(win.find("mr-merge-options").visible());
        assert!(win.find("mr-rebase-link").visible());
        assert!(win.find("mr-sb-edit-assignees").visible());
        assert!(win.find("mr-sb-edit-reviewers").visible());
        assert!(win.find("mr-sb-edit-labels").visible());
        assert!(win.find("mr-sb-edit-milestone").visible());
        assert!(win.find("mr-sb-time-add").visible());
        assert!(win.find("mr-assign-self").visible());
        assert!(win.find("mr-reviewer-self").visible());
        assert!(win.find("mr-comment-composer").visible());
        assert!(state.read(cx).summary.can_merge_mr);
        assert!(state.read(cx).summary.can_approve_mr);
        assert!(state.read(cx).summary.can_close_mr);

        // 2. Developer on protected branch (can_merge=false): Merge & checkboxes hidden, Edit/Approve stay visible.
        let mr_no_merge = sample_mr(99, Some(false));
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_DEVELOPER, false), cx);
        });
        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr_no_merge.clone()], counts.clone(), Some(mr_no_merge), vec![], vec![],
                vec![], vec![], vec![],
                Some(MrApprovals { user_can_approve: true, ..MrApprovals::default() }),
                vec![], vec![], cx,
            );
        });
        win.render_frame(cx);
        assert!(win.find("mr-widget-merge").visible());
        assert!(win.try_find("mr-widget-merge-btn").is_none());
        assert!(win.try_find("mr-merge-options").is_none());
        assert!(win.find("mr-widget-approve-btn").visible());
        assert!(win.find("mr-edit-btn").visible());
        assert!(win.find("mr-rebase-link").visible());
        assert!(win.find("mr-sb-edit-assignees").visible());
        assert!(!state.read(cx).summary.can_merge_mr);
        assert!(state.read(cx).summary.can_approve_mr);

        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        win.dispatch_action(Box::new(MergeMrAction), cx);
    })
    .unwrap();
    cx.run_until_parked();

    // 3. Reporter (non-author, approvals=None): all MR write/review mutations hidden; read-only + comment stay visible.
    cx.update_window(window, |_, win, cx| {
        assert!(!win.has_active_dialog(cx), "MergeMrAction must not open dialog when can_merge=false");
        let state = shell.read(cx).state().clone();
        let mrs = shell.read(cx).mrs_view().clone();
        let mr_rep = sample_mr(99, Some(false));
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_REPORTER, false), cx);
        });
        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr_rep.clone()], counts.clone(), None, vec![], vec![], vec![], vec![],
                vec![], None, vec![], vec![], cx,
            );
        });
        win.render_frame(cx);
        assert!(win.try_find("mr-new-btn").is_none());
        assert!(win.try_find("mr-bulk-edit").is_none());

        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr_rep.clone()], counts.clone(), Some(mr_rep), vec![], vec![], vec![],
                vec![], vec![], None, vec![], vec![], cx,
            );
        });
        win.render_frame(cx);
        assert!(win.find("mr-detail-title").visible());
        assert!(win.find("mr-widget-merge").visible());
        assert!(win.find("mr-add-todo").visible());
        assert!(win.find("mr-comment-composer").visible());
        for hidden_id in [
            "mr-edit-btn",
            "mr-widget-merge-btn",
            "mr-merge-options",
            "mr-widget-approve-btn",
            "mr-rebase-link",
            "mr-sb-edit-assignees",
            "mr-sb-edit-reviewers",
            "mr-sb-edit-labels",
            "mr-sb-edit-milestone",
            "mr-sb-time-add",
            "mr-assign-self",
            "mr-reviewer-self",
        ] {
            assert!(win.try_find(hidden_id).is_none(), "{hidden_id} should be hidden for Reporter");
        }
        assert!(!state.read(cx).summary.can_close_mr);
        win.dispatch_action(Box::new(CloseMrAction), cx);
    })
    .unwrap();
    cx.run_until_parked();

    // 4. Archived project hides EVERY mutation even for Owner with can_merge=true.
    cx.update_window(window, |_, win, cx| {
        assert!(!win.has_active_dialog(cx), "CloseMrAction must not open dialog for Reporter");
        let state = shell.read(cx).state().clone();
        let mrs = shell.read(cx).mrs_view().clone();
        let mr_full = sample_mr(7, Some(true));
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_OWNER, true), cx);
        });
        mrs.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mr_full.clone()], counts, Some(mr_full), vec![], vec![], vec![], vec![],
                vec![], Some(MrApprovals { user_can_approve: true, ..MrApprovals::default() }),
                vec![], vec![], cx,
            );
        });
        win.render_frame(cx);
        for hidden_id in [
            "mr-edit-btn",
            "mr-add-todo",
            "mr-widget-merge-btn",
            "mr-merge-options",
            "mr-widget-approve-btn",
            "mr-rebase-link",
            "mr-sb-edit-assignees",
            "mr-sb-edit-reviewers",
            "mr-comment-composer",
        ] {
            assert!(win.try_find(hidden_id).is_none(), "{hidden_id} should be hidden on archived project");
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn pipelines_and_overview_write_buttons_hide_for_reporter_and_show_for_developer(
    cx: &mut TestAppContext,
) {
    let (window, shell) = mount_shell(cx);

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Pipelines, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_REPORTER, false), cx);
        });
        win.render_frame(cx);
        assert!(win.try_find("pipe-run").is_none());

        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_DEVELOPER, false), cx);
        });
        win.render_frame(cx);
        assert!(win.find("pipe-run").visible());

        // Project overview repo "+" menu follows push permission.
        state.update(cx, |s, cx| {
            s.set_screen(Screen::ProjectOverview, cx);
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_REPORTER, false), cx);
        });
        win.render_frame(cx);
        assert!(win.try_find("ov-repo-plus").is_none());

        state.update(cx, |s, cx| {
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_DEVELOPER, false), cx);
        });
        win.render_frame(cx);
        assert!(win.find("ov-repo-plus").visible());
    })
    .unwrap();
}
