use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{
    Issue, MergeRequest, MrListCounts, Project, RawDiff, Release, ReleaseAssetLink, ReleaseAssets,
    ReleaseCommit, ReleaseEvidence, ReleaseSource, RepoBlob, RepoBlobKind, RepoCommit, Todo,
    TodoProject, TodoTab, TodoTarget, User,
};
use gitlab_for_mac::gpui_app::{init_app, shell::AppShell, state::AppState};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_project() -> Project {
    Project {
        id: 505,
        name: "demo-platform".into(),
        path_with_namespace: "acme/backend/demo-platform".into(),
        default_branch: Some("main".into()),
        web_url: "https://gitlab.example.com/acme/backend/demo-platform".into(),
        last_activity_at: Some("2026-11-28T00:00:00Z".into()),
        star_count: 8,
        avatar_url: None,
    }
}

fn sample_user() -> User {
    User { id: 11, username: "alice".into(), name: "Alice Dev".into(), avatar_url: None }
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
            let mut st = AppState::from_parts(vec![host.clone()], Some(host), vec![sample_project()], Prefs::default(), "Ready");
            st.offline_mode = true;
            st
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

#[gpui_kit::test]
fn releases_2col_cards_toggle_assets_and_jump_to_tag(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Releases, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let release = Release {
            tag_name: "v2.0.0".into(),
            name: Some("Demo Platform v2.0.0".into()),
            released_at: Some("2026-11-27T10:00:00Z".into()),
            description: Some("- Shipped 2-column releases\n- Added assets grouping".into()),
            author: Some(sample_user()),
            commit: Some(ReleaseCommit { id: "28e5bb7900112233".into(), short_id: "28e5bb79".into(), ..Default::default() }),
            assets: Some(ReleaseAssets {
                count: 2,
                sources: vec![ReleaseSource { format: "zip".into(), url: "https://gitlab.example.com/archive.zip".into() }],
                links: vec![ReleaseAssetLink {
                    id: 1, name: "demo-v2.tar.gz".into(), url: "https://gitlab.example.com/pkg.tar.gz".into(),
                    link_type: Some("package".into()), ..Default::default()
                }],
            }),
            evidences: vec![ReleaseEvidence {
                sha: Some("a3da9101ff00".into()),
                filepath: Some("https://gitlab.example.com/v2.0.0-evidence.json".into()),
                collected_at: Some("2026-11-27T10:01:00Z".into()),
            }],
            ..Default::default()
        };

        let view = shell.read(cx).releases_view().clone();
        view.update(cx, |v, cx| v.set_fixture_data(vec![release], cx));
        win.render_frame(cx);

        assert!(win.find("releases-screen").visible());
        assert!(win.find("release-card-0").visible());
        assert!(win.find("rel-assets-body-0").visible());

        win.click("rel-assets-toggle-0", cx);
        win.render_frame(cx);
        assert!(view.read(cx).is_assets_collapsed("v2.0.0"));
        assert!(win.try_find("rel-assets-body-0").is_none());

        win.click("rel-tag-0", cx);
        win.render_frame(cx);
        assert_eq!(shell.read(cx).state().read(cx).screen, Screen::ProjectOverview);
    })
    .unwrap();
}

#[gpui_kit::test]
fn repo_blob_renders_commit_banner_and_opens_history(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Repository, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let repo = shell.read(cx).repo_view().clone();
        repo.update(cx, |v, cx| {
            v.set_fixture_blob(
                "src/main.rs".into(),
                RepoBlob { sha: Some("abc12345".into()), size_bytes: 32, kind: RepoBlobKind::Text("fn main() {}\n".into()) },
                None, win, cx,
            );
            v.set_fixture_last_commit(Some(RepoCommit {
                id: "e0e65ce511223344".into(), short_id: "e0e65ce5".into(),
                title: "fix(release): clean up env handling".into(),
                message: "fix(release): clean up env handling".into(),
                author_name: "Alice Dev".into(), author_email: None,
                authored_date: Some("2026-11-26T08:00:00Z".into()), committed_date: None, created_at: None,
                web_url: None, last_pipeline: None,
            }), cx);
        });
        win.render_frame(cx);

        assert!(win.find("repo-file-commit-banner").visible());
        assert!(win.find("repo-blob-card-header").visible());

        win.click("repo-commit-history", cx);
        win.render_frame(cx);
        assert_eq!(shell.read(cx).state().read(cx).screen, Screen::Commits);
    })
    .unwrap();
}

#[gpui_kit::test]
fn mr_changes_renders_nested_tree_stats_and_viewed_toggle(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::MergeRequests, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let mr: MergeRequest = serde_json::from_value(serde_json::json!({
            "id": 700, "iid": 77, "title": "feat: nested diff tree", "state": "opened",
            "source_branch": "feat/tree", "target_branch": "main",
            "web_url": "https://gitlab.example.com/acme/backend/demo-platform/-/merge_requests/77"
        })).unwrap();
        let diffs = vec![
            RawDiff {
                old_path: "app/src/main/alpha.rs".into(), new_path: "app/src/main/alpha.rs".into(),
                diff: "@@ -1,1 +1,2 @@\n fn a() {}\n+fn b() {}\n".into(),
                new_file: false, renamed_file: false, deleted_file: false,
                too_large: false, collapsed: false, generated_file: false,
            },
            RawDiff {
                old_path: "app/src/main/beta.rs".into(), new_path: "app/src/main/beta.rs".into(),
                diff: "@@ -0,0 +1,1 @@\n+pub const OK: bool = true;\n".into(),
                new_file: true, renamed_file: false, deleted_file: false,
                too_large: false, collapsed: false, generated_file: false,
            },
        ];

        let mrs = shell.read(cx).mrs_view().clone();
        mrs.update(cx, |v, cx| {
            v.set_fixture_data(vec![mr.clone()], MrListCounts::default(), Some(mr), vec![], vec![], vec![], vec![], diffs, None, vec![], vec![], cx);
        });
        win.render_frame(cx);
        win.click("mr-detail-tab-3", cx);
        win.render_frame(cx);

        assert!(win.find("mr-file-tree").visible());
        assert!(win.find("mr-dir-0").visible());
        assert!(win.find("mr-tree-0").visible());
        assert!(win.find("mr-diff-stats").visible());
        assert_eq!(mrs.read(cx).viewed_file_count(), 0);

        win.click("mr-file-viewed-0", cx);
        win.render_frame(cx);
        assert_eq!(mrs.read(cx).viewed_file_count(), 1);
        assert!(win.find("mr-diff-viewed-count").visible());

        win.click("mr-tree-mode-toggle", cx);
        win.render_frame(cx);
        assert!(win.try_find("mr-dir-0").is_none());
        assert!(win.find("mr-tree-0").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn todos_snooze_bulk_select_and_work_items_list_detail_work(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Todos, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let mk_todo = |id: u64, action: &str, state: &str, title: &str| Todo {
            id, action_name: action.into(), body: title.into(), state: "pending".into(),
            created_at: Some("2026-11-27T12:00:00Z".into()), author: Some(sample_user()),
            project: Some(TodoProject { id: 505, name: "demo-platform".into(), path_with_namespace: "acme/backend/demo-platform".into() }),
            target_type: Some("MergeRequest".into()),
            target: Some(TodoTarget { iid: Some(id), title: Some(title.into()), state: Some(state.into()) }),
            target_url: None,
        };
        let todos = shell.read(cx).todos_view().clone();
        todos.update(cx, |v, cx| {
            v.set_fixture_data(
                vec![mk_todo(10, "assigned", "opened", "chore: debug logging"), mk_todo(11, "unmergeable", "closed", "Add CI pipeline")],
                vec![], cx,
            );
        });
        win.render_frame(cx);
        assert!(win.find("todos-screen").visible());
        assert_eq!(todos.read(cx).pending_count(), 2);

        // Snooze one item and verify Snoozed tab.
        win.click("todo-snooze-10", cx);
        win.render_frame(cx);
        assert_eq!(todos.read(cx).pending_count(), 1);
        assert_eq!(todos.read(cx).snoozed_count(), 1);

        win.click("todos-tab-snoozed", cx);
        win.render_frame(cx);
        assert_eq!(todos.read(cx).tab(), TodoTab::Snoozed);
        assert!(win.find("todo-row-10").visible());

        // Select all & bulk mark done.
        win.click("todos-tab-pending", cx);
        win.render_frame(cx);
        win.click("todos-select-all", cx);
        win.render_frame(cx);
        assert_eq!(todos.read(cx).selected_count(), 1);
        win.click("todos-mark-selected", cx);
        win.render_frame(cx);
        assert_eq!(todos.read(cx).pending_count(), 0);

        // Switch to Work items (Issues): verify empty state then list + detail navigation.
        let st = shell.read(cx).state().clone();
        st.update(cx, |s, cx| s.set_screen(Screen::Issues, cx));
        win.render_frame(cx);
        assert!(win.find("issues-empty-state").visible());

        let issue = Issue {
            id: 1, iid: 12, title: "Improve dark mode polish".into(), state: "opened".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-platform/-/issues/12".into(),
            author: Some(sample_user()), description: Some("Details here".into()),
            labels: vec!["ui".into()], created_at: Some("2026-11-27T08:00:00Z".into()),
            updated_at: Some("2026-11-27T09:00:00Z".into()), assignees: vec![sample_user()], user_notes_count: 2,
        };
        let issues = shell.read(cx).issues_view().clone();
        issues.update(cx, |v, cx| v.set_fixture_data(vec![issue], vec![], None, cx));
        win.render_frame(cx);
        assert!(win.find("iss-row-12").visible());

        win.click("iss-row-12", cx);
        win.render_frame(cx);
        assert!(win.find("issue-detail-screen").visible());

        win.click("issue-back-list", cx);
        win.render_frame(cx);
        assert!(win.find("issues-screen").visible());
    })
    .unwrap();
}
