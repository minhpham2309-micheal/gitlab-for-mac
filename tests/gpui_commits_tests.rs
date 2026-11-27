use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{Project, RawDiff, RepoCommit};
use gitlab_for_mac::gpui_app::{init_app, shell::AppShell, state::AppState};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::{Prefs, DEFAULT_UI_FONT_SIZE, MAX_UI_FONT_SIZE};

fn sample_project() -> Project {
    Project {
        id: 303,
        name: "demo-service".into(),
        path_with_namespace: "acme/backend/demo-service".into(),
        default_branch: Some("main".into()),
        web_url: "https://gitlab.example.com/acme/backend/demo-service".into(),
        last_activity_at: Some("2026-11-26T00:00:00Z".into()),
        star_count: 3,
        avatar_url: None,
    }
}

fn sample_commits() -> Vec<RepoCommit> {
    vec![
        RepoCommit {
            id: "a4f042ad11223344556677889900aabbccddeeff".into(),
            short_id: "a4f042ad".into(),
            title: "Merge branch 'feat/very-long-feature-branch-name-for-layout-truncation' into 'main'".into(),
            message: "Merge branch 'feat/very-long-feature-branch-name-for-layout-truncation' into 'main'\n\nfeat(api): add pagination support for commit history".into(),
            author_name: "alice-with-a-very-long-display-name-for-truncation".into(),
            author_email: None,
            authored_date: Some("2026-10-27T08:00:00Z".into()),
            committed_date: None,
            created_at: None,
            web_url: Some("https://gitlab.example.com/acme/backend/demo-service/-/commit/a4f042ad".into()),
            last_pipeline: None,
        },
        RepoCommit {
            id: "d5cf1a1211223344556677889900aabbccddeeff".into(),
            short_id: "d5cf1a12".into(),
            title: "Edit deploy.sh".into(),
            message: "Edit deploy.sh".into(),
            author_name: "bob".into(),
            author_email: None,
            authored_date: Some("2026-10-27T07:00:00Z".into()),
            committed_date: None,
            created_at: None,
            web_url: None,
            last_pipeline: None,
        },
    ]
}

fn mount_shell(
    cx: &mut TestAppContext,
    ui_font_size: u16,
) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    cx.update(|cx| {
        let host = HostAuth {
            hostname: "gitlab.example.com".into(),
            api_host: "gitlab.example.com".into(),
            api_protocol: "https".into(),
            token: "glpat-fixture".into(),
            user: Some("alice".into()),
        };
        let prefs = Prefs { ui_font_size: Some(ui_font_size), ..Prefs::default() };
        let state = cx.new(|_| {
            AppState::from_parts(vec![host.clone()], Some(host), vec![sample_project()], prefs, "Ready")
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

fn assert_commit_rows_aligned_and_unclipped(cx: &mut TestAppContext, ui_font_size: u16) {
    let (window, shell) = mount_shell(cx, ui_font_size);

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Commits, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let commits = sample_commits();
        let sel = Some(commits[0].id.clone());
        let diffs: Vec<RawDiff> = serde_json::from_str(
            r#"[{
                "old_path": "deploy.sh",
                "new_path": "deploy.sh",
                "diff": "--- a/deploy.sh\n+++ b/deploy.sh\n@@ -1,2 +1,3 @@\n #!/bin/sh\n+echo hi\n echo ok\n",
                "new_file": false,
                "renamed_file": false,
                "deleted_file": false,
                "generated_file": null
            }]"#,
        )
        .expect("deserialize GitLab commit diff with null generated_file");
        let view = shell.read(cx).commits_view().clone();
        view.update(cx, |v, cx| v.set_fixture_data(commits.clone(), sel.clone(), diffs, None, cx));
        win.render_frame(cx);

        let r0 = win.find("commit-row-0").bounds();
        let r1 = win.find("commit-row-1").bounds();
        let t0 = win.find("commit-title-0").bounds();
        let t1 = win.find("commit-title-1").bounds();
        let m0 = win.find("commit-meta-0").bounds();
        let m1 = win.find("commit-meta-1").bounds();
        let a0 = win.find("commit-author-0").bounds();
        let s0 = win.find("commit-sha-0").bounds();
        let s1 = win.find("commit-sha-1").bounds();

        assert_eq!(r0.left(), r1.left());
        assert_eq!(r0.right(), r1.right(), "rows must span full width at {ui_font_size}px");
        assert_eq!(s0.right(), s1.right(), "SHAs must align flush-right at {ui_font_size}px");
        assert!(a0.right() <= s0.left(), "author must not overlap SHA: a0={a0:?} s0={s0:?}");
        assert!(t0.size.height >= m0.size.height - px(1.), "title vertically clipped at {ui_font_size}px: t0={t0:?} m0={m0:?}");
        assert!(t1.size.height >= m1.size.height - px(1.), "short title vertically clipped at {ui_font_size}px: t1={t1:?} m1={m1:?}");
        assert!(t0.bottom() <= m0.top(), "title overlaps meta at {ui_font_size}px: t0={t0:?} m0={m0:?}");
        assert!(t0.top() >= r0.top() && m0.bottom() <= r0.bottom(), "row children overflow row bounds at {ui_font_size}px");

        assert_eq!(view.read(cx).diff_file_count(cx), 1);
        assert!(win.find("commit-diff-toolbar").visible());
        assert!(win.find("commit-diff-viewer").visible());
        win.click("commit-diff-mode", cx);
        win.render_frame(cx);
        assert!(win.find("commit-diff-viewer").visible());

        // Error state renders an explicit error panel instead of a blank box.
        view.update(cx, |v, cx| v.set_fixture_data(commits.clone(), sel.clone(), vec![], Some("HTTP 500".into()), cx));
        win.render_frame(cx);
        assert!(win.find("commit-diff-error").visible());

        // Fallback notices (e.g. too_large) render explicitly.
        let omitted: Vec<RawDiff> = serde_json::from_str(
            r#"[{"old_path":"big.bin","new_path":"big.bin","diff":"","too_large":true,"generated_file":null}]"#,
        ).unwrap();
        view.update(cx, |v, cx| v.set_fixture_data(commits.clone(), sel.clone(), omitted, None, cx));
        win.render_frame(cx);
        assert!(win.find("commit-diff-notices").visible());

        // Empty commit diff renders an explicit empty state.
        view.update(cx, |v, cx| v.set_fixture_data(commits, sel, vec![], None, cx));
        win.render_frame(cx);
        assert!(win.find("commit-diff-empty").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn commits_history_renders_diff_and_unclipped_rows_at_default_font(cx: &mut TestAppContext) {
    assert_commit_rows_aligned_and_unclipped(cx, DEFAULT_UI_FONT_SIZE);
}

#[gpui_kit::test]
fn commits_history_keeps_rows_unclipped_at_max_ui_font(cx: &mut TestAppContext) {
    assert_commit_rows_aligned_and_unclipped(cx, MAX_UI_FONT_SIZE);
}
