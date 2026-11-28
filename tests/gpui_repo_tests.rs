use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{
    Branch, Project, RepoBlob, RepoBlobKind, RepoTag, RepoTreeEntry, TreeLastCommit,
};
use gitlab_for_mac::gpui_app::{
    actions::{DismissOverlay, OpenFindFile},
    init_app,
    repo_cache::RepoCache,
    shell::AppShell,
    state::AppState,
};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_hosts() -> Vec<HostAuth> {
    vec![
        HostAuth {
            hostname: "gitlab.example.com".into(),
            api_host: "gitlab.example.com".into(),
            api_protocol: "https".into(),
            token: "glpat-fixture-1".into(),
            user: Some("alice".into()),
        },
        HostAuth {
            hostname: "code.internal.test".into(),
            api_host: "code.internal.test".into(),
            api_protocol: "https".into(),
            token: "glpat-fixture-2".into(),
            user: Some("bob".into()),
        },
    ]
}

fn sample_projects() -> Vec<Project> {
    vec![Project {
        id: 202,
        name: "Demo Service".into(),
        path_with_namespace: "acme/backend/demo-service".into(),
        default_branch: Some("main".into()),
        web_url: "https://gitlab.example.com/acme/backend/demo-service".into(),
        last_activity_at: Some("2026-11-26T00:00:00Z".into()),
        star_count: 0,
        avatar_url: None,
    }]
}

fn mount_with_project(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    let (win, shell) = cx.update(|cx| {
        let hosts = sample_hosts();
        let active = hosts.first().cloned();
        let state = cx.new(|_| {
            AppState::from_parts(hosts, active, sample_projects(), Prefs::default(), "Ready")
        });
        let bounds = Bounds::new(gpui_kit::point(px(0.), px(0.)), size(px(1440.), px(900.)));
        gpui_kit::open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            cx,
            move |window, cx| {
                let state = state.clone();
                cx.new(|cx| AppShell::new(state, window, cx))
            },
        )
        .expect("open headless shell")
    });
    cx.update_window(win, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project(proj, cx));
        win.draw(cx).clear(cx);
    })
    .unwrap();
    (win, shell)
}

#[gpui_kit::test]
fn project_overview_renders_all_zones_and_handles_missing_last_commit(cx: &mut TestAppContext) {
    let (win, shell) = mount_with_project(cx);
    cx.update_window(win, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        let ov = shell.read(cx).overview_view().clone();
        let entries = vec![
            RepoTreeEntry {
                id: "tree1".into(), name: "app".into(), entry_type: "tree".into(),
                path: "app".into(), mode: "040000".into(),
                last_commit: Some(TreeLastCommit {
                    id: "a1b2c3d4".into(), short_id: "a1b2c3d4".into(),
                    title: "feat: initial repository layout".into(),
                    author_name: "alice".into(),
                    committed_date: Some("2026-11-26T05:00:00Z".into()),
                    created_at: None,
                }),
            },
            RepoTreeEntry {
                id: "blob1".into(), name: "README.md".into(), entry_type: "blob".into(),
                path: "README.md".into(), mode: "100644".into(), last_commit: None,
            },
        ];
        ov.update(cx, |v, cx| v.set_fixture_tree(entries.clone(), true, cx));
        win.draw(cx).clear(cx);
        // Simulate older self-hosted GitLab without last_commit support.
        ov.update(cx, |v, cx| v.set_fixture_tree(entries, false, cx));
        win.draw(cx).clear(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn source_viewer_renders_5000_lines_guards_binary_and_uses_repo_cache(cx: &mut TestAppContext) {
    let (win, shell) = mount_with_project(cx);
    cx.update_window(win, |_, _win, cx| {
        let state = shell.read(cx).state().clone();

        // Populate RepoCache with an immutable blob SHA and open via deep-link (0 network calls).
        let big_code: String = (1..=5_000).map(|i| format!("pub fn line_{i}() -> usize {{ {i} }}\n")).collect();
        let cache = cx.global::<RepoCache>().clone();
        let origin = state.read(cx).gitlab.as_ref().unwrap().origin().ascii_serialization();
        let generation = cache.generation();
        cache.insert_blob(
            generation, &origin, 202, "sha-5k",
            RepoBlob { sha: Some("sha-5k".into()), size_bytes: big_code.len(), kind: RepoBlobKind::Text(big_code) },
        );

        state.update(cx, |s, cx| {
            s.open_repo_file(202, Some("main".into()), "src/large.rs".into(), Some("sha-5k".into()), Some(120), cx);
        });
        assert_eq!(state.read(cx).screen, Screen::Repository);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(win, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let repo = shell.read(cx).repo_view().clone();
        let cache = cx.global::<RepoCache>().clone();
        let origin = state.read(cx).gitlab.as_ref().unwrap().origin().ascii_serialization();
        win.draw(cx).clear(cx);
        assert_eq!(repo.read(cx).active_path(), Some("src/large.rs"));
        assert!(repo.read(cx).active_blob().is_some());

        // Markdown toggle + binary + oversize guards.
        repo.update(cx, |v, cx| {
            v.set_fixture_blob("README.md".into(), RepoBlob {
                sha: Some("sha-md".into()), size_bytes: 12, kind: RepoBlobKind::Text("# Hello".into()),
            }, None, win, cx);
            assert!(v.is_markdown_preview());
            v.toggle_markdown_preview(cx);
            assert!(!v.is_markdown_preview());
        });
        win.draw(cx).clear(cx);

        repo.update(cx, |v, cx| {
            v.set_fixture_blob("app.bin".into(), RepoBlob {
                sha: None, size_bytes: 4096, kind: RepoBlobKind::Binary,
            }, None, win, cx);
        });
        win.draw(cx).clear(cx);

        repo.update(cx, |v, cx| {
            v.set_fixture_blob("dump.sql".into(), RepoBlob {
                sha: None, size_bytes: 9_000_000, kind: RepoBlobKind::TooLarge { limit_bytes: 1_536 * 1024 },
            }, None, win, cx);
        });
        win.draw(cx).clear(cx);

        // Host switch clears RepoCache completely.
        state.update(cx, |s, cx| s.select_host("code.internal.test", cx));
        assert!(cache.get_blob(&origin, 202, "sha-5k").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn find_file_modal_and_code_ref_screens_mount_and_dismiss(cx: &mut TestAppContext) {
    let (win, shell) = mount_with_project(cx);
    cx.update_window(win, |_, win, cx| {
        assert!(!shell.read(cx).is_find_file_open(cx));
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        win.dispatch_action(Box::new(OpenFindFile), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(win, |_, win, cx| {
        assert!(shell.read(cx).is_find_file_open(cx));
        assert!(!shell.read(cx).is_palette_open());
        win.draw(cx).clear(cx);
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.dispatch_action(Box::new(DismissOverlay), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(win, |_, win, cx| {
        assert!(!shell.read(cx).is_find_file_open(cx));
        let state = shell.read(cx).state().clone();
        for screen in [Screen::Commits, Screen::Branches, Screen::Tags] {
            state.update(cx, |s, cx| s.set_screen(screen, cx));
            win.draw(cx).clear(cx);
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn overview_ref_switcher_scrolls_filters_and_selects_branch_or_tag(cx: &mut TestAppContext) {
    let (win, shell) = mount_with_project(cx);
    cx.run_until_parked();

    cx.update_window(win, |_, win, cx| {
        let ov = shell.read(cx).overview_view().clone();
        let mut branches = vec![
            Branch { name: "main".into(), default: true, protected: true, ..Default::default() },
            Branch { name: "release/v2.0".into(), default: false, protected: true, ..Default::default() },
        ];
        for i in 1..=35 {
            branches.push(Branch {
                name: format!("feat/module-{i:02}-long-branch-name"),
                default: false,
                protected: false,
                ..Default::default()
            });
        }
        let tags = vec![
            RepoTag { name: "v2.4.0".into(), protected: true, ..Default::default() },
            RepoTag { name: "v2.3.0".into(), protected: false, ..Default::default() },
        ];
        ov.update(cx, |v, cx| v.set_fixture_refs(branches, tags, cx));
        win.render_frame(cx);

        // Open GitLab 18 "Select Git revision" popover and verify bounded scrollable height.
        win.click("ov-ref-select", cx);
        win.render_frame(cx);
        let panel = win.find("ov-ref-panel");
        assert!(panel.visible());
        assert!(panel.bounds().size.height <= px(420.), "ref switcher popover must be bounded, got {:?}", panel.bounds().size.height);
        assert!(win.find("ov-ref-selected-row").visible());
        assert!(win.find("ov-ref-scroll").visible());

        // Filter branches live via search box and select matching branch.
        let search = ov.read(cx).ref_search_input().clone();
        search.update(cx, |inp, cx| inp.set_value("release/v2", win, cx));
        win.render_frame(cx);
        assert!(win.find("ov-ref-branch-0").visible());
        assert!(win.try_find("ov-ref-branch-1").is_none());

        win.click("ov-ref-branch-0", cx);
        win.render_frame(cx);
        assert_eq!(ov.read(cx).current_ref(), "release/v2.0");
        assert!(win.try_find("ov-ref-panel").is_none());

        // Re-open, filter for tag, and switch to tag.
        win.click("ov-ref-select", cx);
        win.render_frame(cx);
        search.update(cx, |inp, cx| inp.set_value("v2.4", win, cx));
        win.render_frame(cx);
        win.click("ov-ref-tag-0", cx);
        win.render_frame(cx);
        assert_eq!(ov.read(cx).current_ref(), "v2.4.0");
        assert!(win.try_find("ov-ref-panel").is_none());
    })
    .unwrap();
}
