use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{Project, Runner, Schedule, SearchBlob};
use gitlab_for_mac::gpui_app::{init_app, shell::AppShell, state::AppState};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_projects() -> Vec<Project> {
    vec![
        Project {
            id: 601, name: "alpha-service".into(), path_with_namespace: "acme/platform/alpha-service".into(),
            default_branch: Some("main".into()), web_url: "https://gitlab.example.com/acme/platform/alpha-service".into(),
            last_activity_at: Some("2026-11-27T12:00:00Z".into()), star_count: 12, avatar_url: None,
        },
        Project {
            id: 602, name: "beta-worker".into(), path_with_namespace: "acme/platform/beta-worker".into(),
            default_branch: Some("develop".into()), web_url: "https://gitlab.example.com/acme/platform/beta-worker".into(),
            last_activity_at: Some("2026-11-20T08:00:00Z".into()), star_count: 0, avatar_url: None,
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
            let mut st = AppState::from_parts(vec![host.clone()], Some(host), sample_projects(), Prefs::default(), "Ready");
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
fn projects_screen_filters_starred_and_opens_group_namespace(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        win.render_frame(cx);
        assert!(win.find("projects-screen").visible());
        assert!(win.find("project-row-601").visible());
        assert!(win.find("project-row-602").visible());

        win.click("projects-tab-starred", cx);
        win.render_frame(cx);
        assert!(win.find("project-row-601").visible());
        assert!(win.try_find("project-row-602").is_none());

        // Clicking namespace prefix navigates directly into the Groups Explorer for that group.
        win.click("proj-ns-601", cx);
        win.render_frame(cx);
        assert_eq!(shell.read(cx).state().read(cx).screen, Screen::Groups);
        assert_eq!(shell.read(cx).state().read(cx).active_group_path.as_deref(), Some("acme/platform"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn schedules_and_runners_screens_filter_by_status_tabs(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let st = shell.read(cx).state().clone();
        let proj = st.read(cx).projects[0].clone();
        st.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Schedules, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let scheds = shell.read(cx).schedules_view().clone();
        scheds.update(cx, |v, cx| {
            v.set_fixture_data(vec![
                Schedule { id: 1, description: "Nightly build".into(), git_ref: "main".into(), cron: "0 2 * * *".into(), active: true },
                Schedule { id: 2, description: "Paused sync".into(), git_ref: "release".into(), cron: "0 4 * * 1".into(), active: false },
            ], cx);
        });
        win.render_frame(cx);
        assert!(win.find("schedules-screen").visible());
        assert!(win.find("sched-row-1").visible());
        assert!(win.find("sched-row-2").visible());

        win.click("sched-tab-active", cx);
        win.render_frame(cx);
        assert!(win.find("sched-row-1").visible());
        assert!(win.try_find("sched-row-2").is_none());

        // Runners screen status filtering.
        let st = shell.read(cx).state().clone();
        st.update(cx, |s, cx| s.set_screen(Screen::Runners, cx));
        let runners = shell.read(cx).runners_view().clone();
        runners.update(cx, |v, cx| {
            v.set_fixture_data(vec![
                Runner { id: 10, description: Some("mac-arm64".into()), status: Some("online".into()), is_shared: Some(false), paused: Some(false) },
                Runner { id: 11, description: Some("linux-docker".into()), status: Some("online".into()), is_shared: Some(true), paused: Some(true) },
                Runner { id: 12, description: Some("stale-box".into()), status: Some("offline".into()), is_shared: Some(false), paused: Some(false) },
            ], cx);
        });
        win.render_frame(cx);
        assert!(win.find("runners-screen").visible());
        assert!(win.find("runner-row-10").visible());

        win.click("runner-tab-paused", cx);
        win.render_frame(cx);
        assert!(win.find("runner-row-11").visible());
        assert!(win.try_find("runner-row-10").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn search_and_console_screens_render_gitlab18_surfaces_and_actions(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, _, cx| {
        let st = shell.read(cx).state().clone();
        let proj = st.read(cx).projects[0].clone();
        st.update(cx, |s, cx| s.select_project_on_screen(proj, Screen::Search, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        win.render_frame(cx);
        assert!(win.find("search-screen").visible());
        assert!(win.find("search-empty").visible());

        let search = shell.read(cx).search_view().clone();
        search.update(cx, |v, cx| {
            v.set_fixture_data(vec![SearchBlob {
                basename: Some("main.rs".into()), data: Some("fn main() {}".into()),
                filename: Some("main.rs".into()), path: Some("src/main.rs".into()),
                git_ref: Some("main".into()), startline: Some(1),
            }], cx);
        });
        win.render_frame(cx);
        assert!(win.find("search-hit-0").visible());

        // Console screen output and clear action.
        let st = shell.read(cx).state().clone();
        st.update(cx, |s, cx| s.set_screen(Screen::Console, cx));
        win.render_frame(cx);
        assert!(win.find("console-screen").visible());
        assert!(win.find("console-empty").visible());

        let console = shell.read(cx).console_view().clone();
        console.update(cx, |v, cx| v.set_fixture_output(vec!["Pipeline #42 passed".into()], cx));
        win.render_frame(cx);
        assert_eq!(console.read(cx).line_count(), 1);
        win.click("console-clear", cx);
        win.render_frame(cx);
        assert_eq!(console.read(cx).line_count(), 0);
        assert!(win.find("console-empty").visible());
    })
    .unwrap();
}
