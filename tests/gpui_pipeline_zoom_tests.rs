use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{Job, Pipeline, Project};
use gitlab_for_mac::gpui_app::{init_app, shell::AppShell, state::AppState};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::{
    Prefs, DEFAULT_PIPELINE_GRAPH_ZOOM, MAX_PIPELINE_GRAPH_ZOOM, MIN_PIPELINE_GRAPH_ZOOM,
};

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
            id: 126129,
            iid: Some(12),
            status: "success".into(),
            git_ref: "main".into(),
            sha: "c4facbf7112233445566".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/pipelines/126129".into(),
            created_at: Some("2026-11-26T08:00:00Z".into()),
            updated_at: Some("2026-11-26T08:05:00Z".into()),
            source: Some("push".into()),
        },
        Pipeline {
            id: 126101,
            iid: Some(11),
            status: "success".into(),
            git_ref: "refs/merge-requests/186/head".into(),
            sha: "b5eb6151112233445566".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/pipelines/126101".into(),
            created_at: Some("2026-11-26T07:50:00Z".into()),
            updated_at: Some("2026-11-26T07:55:00Z".into()),
            source: Some("merge_request_event".into()),
        },
    ]
}

fn sample_jobs() -> Vec<Job> {
    vec![
        Job {
            id: 1,
            name: "check-english-only-comments".into(),
            status: "success".into(),
            stage: "check".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/jobs/1".into(),
            duration: Some(38.0),
            started_at: None,
            finished_at: None,
            allow_failure: Some(false),
        },
        Job {
            id: 2,
            name: "test-job".into(),
            status: "success".into(),
            stage: "test".into(),
            web_url: "https://gitlab.example.com/acme/backend/demo-app/-/jobs/2".into(),
            duration: Some(81.0),
            started_at: None,
            finished_at: None,
            allow_failure: Some(false),
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
            AppState::from_parts(
                vec![host.clone()],
                Some(host),
                vec![sample_project()],
                Prefs::default(),
                "Ready",
            )
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
fn pipeline_stage_graph_zoom_controls_scale_columns_and_clamp(cx: &mut TestAppContext) {
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
        let pipes = shell.read(cx).pipelines_view().clone();

        // Empty stage graph hides zoom bar.
        win.render_frame(cx);
        assert!(win.try_find("pipeline-graph-zoom-bar").is_none());

        // Populate stage graph -> zoom bar, aligned pipeline rows, and columns render at 100%.
        pipes.update(cx, |v, cx| v.set_fixture_stage_graph(sample_pipelines(), sample_jobs(), vec![], cx));
        win.render_frame(cx);

        // Verify pipeline list rows span full width and names/IDs align cleanly across short & long refs.
        let row_short = win.find("pipe-row-126129").bounds();
        let row_long = win.find("pipe-row-126101").bounds();
        let name_short = win.find("pipe-name-126129").bounds();
        let name_long = win.find("pipe-name-126101").bounds();
        let id_short = win.find("pipe-id-126129").bounds();
        let id_long = win.find("pipe-id-126101").bounds();
        assert_eq!(row_short.left(), row_long.left());
        assert_eq!(row_short.right(), row_long.right(), "pipeline rows must span full list width");
        assert_eq!(name_short.left(), name_long.left(), "pipeline names must align on left edge");
        assert_eq!(id_short.right(), id_long.right(), "pipeline IDs must align flush-right");
        assert!(name_long.right() < id_long.left(), "long ref name must not overlap pipeline ID");

        assert!(win.find("pipeline-graph-zoom-bar").visible());
        assert_eq!(pipes.read(cx).graph_zoom_pct(), DEFAULT_PIPELINE_GRAPH_ZOOM);
        let w_100 = win.find("stage-col-0").bounds().size.width;
        let card_100 = win.find("job-card-1").bounds().size.width;

        // Zoom in twice (+10% each -> 120%).
        win.click("pipe-zoom-in", cx);
        win.click("pipe-zoom-in", cx);
        win.render_frame(cx);
        assert_eq!(pipes.read(cx).graph_zoom_pct(), 120);
        assert_eq!(state.read(cx).prefs.pipeline_graph_zoom, Some(120));
        let w_120 = win.find("stage-col-0").bounds().size.width;
        let card_120 = win.find("job-card-1").bounds().size.width;
        assert!(w_120 > w_100, "stage column should widen on zoom in: {w_120:?} vs {w_100:?}");
        assert!(card_120 > card_100, "job card should widen on zoom in: {card_120:?} vs {card_100:?}");

        // Reset back to 100%.
        win.click("pipe-zoom-reset", cx);
        win.render_frame(cx);
        assert_eq!(pipes.read(cx).graph_zoom_pct(), DEFAULT_PIPELINE_GRAPH_ZOOM);
        assert_eq!(win.find("stage-col-0").bounds().size.width, w_100);

        // Zoom out twice (-10% each -> 80%).
        win.click("pipe-zoom-out", cx);
        win.click("pipe-zoom-out", cx);
        win.render_frame(cx);
        assert_eq!(pipes.read(cx).graph_zoom_pct(), 80);
        let w_80 = win.find("stage-col-0").bounds().size.width;
        assert!(w_80 < w_100, "stage column should shrink on zoom out: {w_80:?} vs {w_100:?}");

        // Verify clamping at both ends.
        pipes.update(cx, |v, cx| v.step_graph_zoom(-500, cx));
        assert_eq!(pipes.read(cx).graph_zoom_pct(), MIN_PIPELINE_GRAPH_ZOOM);
        pipes.update(cx, |v, cx| v.step_graph_zoom(500, cx));
        assert_eq!(pipes.read(cx).graph_zoom_pct(), MAX_PIPELINE_GRAPH_ZOOM);
    })
    .unwrap();
}
