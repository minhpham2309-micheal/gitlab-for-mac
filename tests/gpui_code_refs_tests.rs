use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{
    Branch, BranchCommit, BranchScope, MergeRequest, Pipeline, Project, RepoCommit, RepoTag,
    RepoTagCommit, RepoTagRelease, SortDir,
};
use gitlab_for_mac::gpui_app::{init_app, shell::AppShell, state::AppState};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_project() -> Project {
    Project {
        id: 404,
        name: "demo-api".into(),
        path_with_namespace: "acme/backend/demo-api".into(),
        default_branch: Some("main".into()),
        web_url: "https://gitlab.example.com/acme/backend/demo-api".into(),
        last_activity_at: Some("2026-11-28T00:00:00Z".into()),
        star_count: 5,
        avatar_url: None,
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
            user: Some("alice".into()),
        };
        let state = cx.new(|_| {
            AppState::from_parts(vec![host.clone()], Some(host), vec![sample_project()], Prefs::default(), "Ready")
        });
        let bounds = Bounds::new(gpui_kit::point(px(0.), px(0.)), size(px(1440.), px(900.)));
        gpui_kit::open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            cx,
            move |window, cx| cx.new(|cx| AppShell::new(state.clone(), window, cx)),
        )
        .expect("open headless shell")
    })
}

fn sample_pipeline(id: u64, git_ref: &str, sha: &str, status: &str) -> Pipeline {
    Pipeline {
        id,
        iid: Some(id),
        status: status.into(),
        git_ref: git_ref.into(),
        sha: sha.into(),
        web_url: format!("https://gitlab.example.com/acme/backend/demo-api/-/pipelines/{id}"),
        created_at: Some("2026-11-26T10:00:00Z".into()),
        updated_at: Some("2026-11-26T10:05:00Z".into()),
        source: Some("push".into()),
    }
}

fn sample_mr(iid: u64, branch: &str, state: &str) -> MergeRequest {
    serde_json::from_value(serde_json::json!({
        "id": 9000 + iid,
        "iid": iid,
        "title": format!("MR for {branch}"),
        "state": state,
        "source_branch": branch,
        "target_branch": "main",
        "web_url": format!("https://gitlab.example.com/acme/backend/demo-api/-/merge_requests/{iid}")
    }))
    .expect("deserialize fixture MR")
}

#[gpui_kit::test]
fn commits_timeline_groups_by_date_and_toggles_split_diff(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Commits, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let commits = vec![
            RepoCommit {
                id: "1111aaaa2222bbbb".into(), short_id: "1111aaaa".into(),
                title: "Merge branch 'feat/timeline' into 'main'".into(),
                message: "Merge branch 'feat/timeline' into 'main'".into(),
                author_name: "alice".into(), author_email: None,
                authored_date: Some("2026-11-26T10:00:00Z".into()), committed_date: None, created_at: None,
                web_url: None, last_pipeline: None,
            },
            RepoCommit {
                id: "2222bbbb3333cccc".into(), short_id: "2222bbbb".into(),
                title: "feat: group commits by day".into(), message: "feat: group commits by day".into(),
                author_name: "bob".into(), author_email: None,
                authored_date: Some("2026-11-26T08:00:00Z".into()), committed_date: None, created_at: None,
                web_url: None, last_pipeline: None,
            },
            RepoCommit {
                id: "3333cccc4444dddd".into(), short_id: "3333cccc".into(),
                title: "chore: init repo".into(), message: "chore: init repo".into(),
                author_name: "carol".into(), author_email: None,
                authored_date: Some("2026-11-25T15:00:00Z".into()), committed_date: None, created_at: None,
                web_url: None, last_pipeline: None,
            },
        ];
        let tags = vec![RepoTag {
            name: "v1.1.0".into(),
            commit: Some(RepoTagCommit { id: "1111aaaa2222bbbb".into(), short_id: "1111aaaa".into(), ..Default::default() }),
            ..Default::default()
        }];
        let pipes = vec![sample_pipeline(701, "main", "1111aaaa2222bbbb", "success")];

        let view = shell.read(cx).commits_view().clone();
        view.update(cx, |v, cx| {
            v.set_fixture_data(commits, None, vec![], None, cx);
            v.set_fixture_enrichments(vec![], tags, pipes, cx);
        });
        win.render_frame(cx);

        assert!(!view.read(cx).is_split_mode());
        assert!(win.find("commits-timeline-view").visible());
        assert!(win.find("commit-date-group-0").visible());
        assert!(win.find("commit-date-group-1").visible());
        assert!(win.find("commit-pipe-0").visible());

        // Clicking a commit card opens the split diff view cleanly.
        win.click("commit-row-0", cx);
        win.render_frame(cx);
        assert!(view.read(cx).is_split_mode());

        // Clicking Timeline collapses back to the full-width timeline.
        win.click("commits-back-timeline", cx);
        win.render_frame(cx);
        assert!(!view.read(cx).is_split_mode());
        assert!(win.find("commits-timeline-view").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn branches_screen_splits_active_and_stale_and_navigates_to_mr(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Branches, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let mk = |name: &str, default: bool, protected: bool, merged: bool, sha: &str, date: &str| Branch {
            name: name.into(), default, protected, merged,
            web_url: Some(format!("https://gitlab.example.com/acme/backend/demo-api/-/tree/{name}")),
            commit: Some(BranchCommit {
                id: sha.into(), short_id: sha[..8].into(), title: format!("commit on {name}"),
                committed_date: Some(date.into()), ..Default::default()
            }),
            ..Default::default()
        };
        let branches = vec![
            mk("main", true, true, false, "aaaa111100000000", "2026-11-27T10:00:00Z"),
            mk("feat/one", false, false, false, "bbbb222200000000", "2026-11-26T10:00:00Z"),
            mk("feat/two", false, false, true, "cccc333300000000", "2026-11-25T10:00:00Z"),
            mk("feat/three", false, false, false, "dddd444400000000", "2026-11-24T10:00:00Z"),
            mk("feat/four", false, false, false, "eeee555500000000", "2026-11-23T10:00:00Z"),
            mk("feat/five", false, false, false, "ffff666600000000", "2026-11-22T10:00:00Z"),
            mk("legacy/old-work", false, false, true, "9999000011112222", "2025-01-10T10:00:00Z"),
        ];
        let pipes = vec![sample_pipeline(801, "main", "aaaa111100000000", "success")];
        let mrs = vec![sample_mr(42, "feat/one", "opened"), sample_mr(41, "feat/two", "merged")];

        let view = shell.read(cx).branches_view().clone();
        view.update(cx, |v, cx| v.set_fixture_data(branches, pipes, mrs, cx));
        win.render_frame(cx);

        assert_eq!(view.read(cx).scope(), BranchScope::Overview);
        assert!(win.find("branch-section-active").visible());
        assert!(win.find("branch-section-stale").visible());
        assert!(win.find("branch-more-active").visible());
        assert!(win.find("br-pipe-active-0").visible());
        assert!(win.find("br-mr-active-1").visible());

        // "Show more active branches" switches to the Active tab with all 6 active rows.
        win.click("branch-more-active", cx);
        win.render_frame(cx);
        assert_eq!(view.read(cx).scope(), BranchScope::Active);
        assert!(win.find("branch-row-active-5").visible());

        // Switching to Stale and All tabs works cleanly.
        win.click("branch-tab-Stale", cx);
        win.render_frame(cx);
        assert_eq!(view.read(cx).scope(), BranchScope::Stale);
        assert!(win.find("branch-row-stale-0").visible());

        win.click("branch-tab-All", cx);
        win.render_frame(cx);
        assert_eq!(view.read(cx).scope(), BranchScope::All);
        assert!(win.find("branch-section-all").visible());

        // Clicking a linked MR pill jumps to MergeRequests screen.
        win.click("br-mr-all-1", cx);
        win.render_frame(cx);
        assert_eq!(shell.read(cx).state().read(cx).screen, Screen::MergeRequests);
    })
    .unwrap();
}

#[gpui_kit::test]
fn tags_screen_renders_release_link_annotation_and_sorts(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Tags, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let tags = vec![
            RepoTag {
                name: "v1.0.0".into(),
                commit: Some(RepoTagCommit {
                    id: "11110000aaaa".into(), short_id: "11110000".into(),
                    title: "Initial release".into(), committed_date: Some("2026-11-01T10:00:00Z".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            RepoTag {
                name: "v1.1.0".into(),
                message: Some("Release v1.1.0: add code refs polish".into()),
                protected: true,
                commit: Some(RepoTagCommit {
                    id: "22220000bbbb".into(), short_id: "22220000".into(),
                    title: "Merge release branch".into(), committed_date: Some("2026-11-26T10:00:00Z".into()),
                    ..Default::default()
                }),
                release: Some(RepoTagRelease { tag_name: "v1.1.0".into(), description: Some("Notes".into()) }),
                ..Default::default()
            },
        ];
        let pipes = vec![sample_pipeline(901, "v1.1.0", "22220000bbbb", "success")];
        let view = shell.read(cx).tags_view().clone();
        view.update(cx, |v, cx| v.set_fixture_data(tags, pipes, cx));
        win.render_frame(cx);

        assert!(win.find("tags-screen").visible());
        assert!(win.find("tag-row-0").visible());
        assert!(win.find("tag-release-0").visible());
        assert!(win.find("tag-msg-0").visible());
        assert!(win.find("tag-pipe-0").visible());
        assert_eq!(view.read(cx).sort_dir(), SortDir::Desc);

        win.click("tags-sort-dir", cx);
        win.render_frame(cx);
        assert_eq!(view.read(cx).sort_dir(), SortDir::Asc);

        win.click("tags-sort-dir", cx);
        win.render_frame(cx);
        win.click("tag-release-0", cx);
        win.render_frame(cx);
        assert_eq!(shell.read(cx).state().read(cx).screen, Screen::Releases);
    })
    .unwrap();
}
