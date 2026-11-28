use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{Job, Pipeline, Project, ProjectPermissions, ACCESS_DEVELOPER};
use gitlab_for_mac::gpui_app::{init_app, shell::AppShell, state::AppState};
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

fn sample_pipelines() -> Vec<Pipeline> {
    vec![
        Pipeline {
            id: 2001, iid: Some(20), status: "success".into(), git_ref: "main".into(),
            sha: "c4facbf7112233445566".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/pipelines/2001".into(),
            created_at: Some("2026-11-26T08:00:00Z".into()),
            updated_at: Some("2026-11-26T08:05:00Z".into()),
            source: Some("push".into()),
        },
        Pipeline {
            id: 2002, iid: Some(21), status: "running".into(), git_ref: "v1.1.0".into(),
            sha: "a1b2c3d4112233445566".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/pipelines/2002".into(),
            created_at: Some("2026-11-26T08:10:00Z".into()),
            updated_at: Some("2026-11-26T08:11:00Z".into()),
            source: Some("push".into()),
        },
    ]
}

fn sample_jobs() -> Vec<Job> {
    vec![
        Job {
            id: 11, name: "lint-job".into(), status: "success".into(), stage: "check".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/jobs/11".into(),
            duration: Some(42.0), started_at: None, finished_at: None, allow_failure: Some(false),
        },
        Job {
            id: 12, name: "unit-test".into(), status: "success".into(), stage: "test".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/jobs/12".into(),
            duration: Some(95.0), started_at: None, finished_at: None, allow_failure: Some(false),
        },
    ]
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
fn gitlab18_pipelines_table_detail_jobs_drawer_and_3pane_toggle_work(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| {
            s.select_project_on_screen(proj, Screen::Pipelines, cx);
            s.set_project_permissions(ProjectPermissions::with_level(ACCESS_DEVELOPER, false), cx);
        });
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let pipes = shell.read(cx).pipelines_view().clone();
        let list = sample_pipelines();
        let jobs = sample_jobs();
        pipes.update(cx, |v, cx| v.set_fixture_gitlab_pipelines(list.clone(), None, jobs.clone(), cx));
        win.render_frame(cx);

        // 1. Full-width GitLab 18 table mode by default.
        assert!(!pipes.read(cx).is_split_mode());
        assert!(win.find("pipelines-table-view").visible());
        assert!(win.find("pipe-run").visible());
        assert!(win.find("pipe-table-row-2001").visible());
        assert!(win.find("pipe-table-row-2002").visible());

        // 2. Scope filtering (Tags shows only v1.1.0, Finished shows only passed).
        win.click("pipe-scope-Tags", cx);
        win.render_frame(cx);
        assert!(win.try_find("pipe-table-row-2001").is_none());
        assert!(win.find("pipe-table-row-2002").visible());
        win.click("pipe-scope-All", cx);
        win.render_frame(cx);
        assert!(win.find("pipe-table-row-2001").visible());

        // 3. Click row -> Pipeline Detail view with Stage Graph.
        win.click("pipe-table-row-2001", cx);
        pipes.update(cx, |v, cx| v.set_fixture_gitlab_pipelines(list.clone(), Some(list[0].clone()), jobs.clone(), cx));
        win.render_frame(cx);
        assert!(win.find("pipeline-detail-view").visible());
        assert!(win.find("pipe-detail-header").visible());
        assert!(win.find("stage-col-0").visible());
        assert!(win.find("job-card-11").visible());

        // 4. Switch to Jobs tab, select a job -> opens Job Log drawer, then close drawer.
        win.click("pipe-detail-tab-1", cx);
        win.render_frame(cx);
        assert!(win.find("pipe-jobs-table").visible());
        win.click("pipe-jobs-row-11", cx);
        win.render_frame(cx);
        assert!(win.find("log-close-drawer").visible());
        win.click("log-close-drawer", cx);
        win.render_frame(cx);
        assert!(win.try_find("log-close-drawer").is_none());

        // 5. Toggle to 3-pane split mode and back to GitLab 18 Web mode.
        win.click("pipe-layout-toggle", cx);
        win.render_frame(cx);
        assert!(pipes.read(cx).is_split_mode());
        assert!(win.find("pipe-row-2001").visible());
        win.click("pipe-layout-toggle", cx);
        win.render_frame(cx);
        assert!(!pipes.read(cx).is_split_mode());

        // 6. Back to All pipelines list.
        win.click("pipe-back-list", cx);
        win.render_frame(cx);
        assert!(win.find("pipelines-table-view").visible());
    })
    .unwrap();
}
