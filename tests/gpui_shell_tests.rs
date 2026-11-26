use gpui_kit::{
    component::{diff::DiffMode, WindowExt as _},
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::Project;
use gitlab_for_mac::gpui_app::{
    actions::{GoPreferences, MergeMrAction, NavBack, NavForward, TogglePalette, ZoomIn, ZoomReset},
    init_app,
    nav_history::NavTarget,
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
        id: 101,
        name: "demo-app".into(),
        path_with_namespace: "team/demo-app".into(),
        default_branch: Some("main".into()),
        web_url: "https://gitlab.example.com/team/demo-app".into(),
        last_activity_at: Some("2026-11-26T00:00:00Z".into()),
        star_count: 7,
        avatar_url: None,
    }]
}

fn mount_shell_with_prefs(
    cx: &mut TestAppContext,
    prefs: Prefs,
) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    cx.update(|cx| {
        let hosts = sample_hosts();
        let active = hosts.first().cloned();
        let state = cx.new(|_| {
            AppState::from_parts(hosts, active, sample_projects(), prefs, "Ready")
        });
        let bounds = Bounds::new(gpui_kit::point(px(0.), px(0.)), size(px(1280.), px(800.)));
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            move |window, cx| {
                let state = state.clone();
                cx.new(|cx| AppShell::new(state, window, cx))
            },
        )
        .expect("open headless shell")
    })
}

fn mount_shell(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    mount_shell_with_prefs(cx, Prefs::default())
}

#[gpui_kit::test]
fn shell_mounts_routes_screens_and_toggles_sidebar_collapse(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        win.draw(cx).clear(cx);
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::Projects);
        assert!(!state.read(cx).sidebar_collapsed);

        state.update(cx, |s, cx| s.toggle_sidebar(cx));
        assert!(state.read(cx).sidebar_collapsed);

        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project(proj, cx));
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);

        state.update(cx, |s, cx| s.set_screen(Screen::Repository, cx));
        assert_eq!(state.read(cx).screen, Screen::Repository);

        state.update(cx, |s, cx| s.set_screen(Screen::MergeRequests, cx));
        assert_eq!(state.read(cx).screen, Screen::MergeRequests);
        win.draw(cx).clear(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn host_switch_bumps_epochs_and_resets_project_state(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project(proj, cx));
        let h_before = state.read(cx).host_epoch;
        let p_before = state.read(cx).project_epoch;
        assert!(state.read(cx).project.is_some());

        state.update(cx, |s, cx| s.select_host("code.internal.test", cx));
        assert!(state.read(cx).host_epoch > h_before);
        assert!(state.read(cx).project_epoch > p_before);
        assert!(state.read(cx).project.is_none());
        assert_eq!(state.read(cx).screen, Screen::Projects);
        win.draw(cx).clear(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn palette_toggle_confirm_dialog_and_diff_mode_toggle_work(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        win.draw(cx).clear(cx);
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        assert!(!shell.read(cx).is_palette_open());
        win.dispatch_action(Box::new(TogglePalette), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        assert!(shell.read(cx).is_palette_open());
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        // Without an open MR active, destructive MergeMrAction must be rejected.
        win.dispatch_action(Box::new(MergeMrAction), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        assert!(!win.has_active_dialog(cx));
        let state = shell.read(cx).state().clone();
        state.update(cx, |s, _| {
            s.summary.can_review_mr = true;
            s.summary.active_mr_iid = Some(42);
            s.summary.active_mr_label = Some("!42 Fix race".into());
        });
        win.dispatch_action(Box::new(MergeMrAction), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        assert!(win.has_active_dialog(cx));
        win.close_all_dialogs(cx);

        let mrs = shell.read(cx).mrs_view().clone();
        assert_eq!(mrs.read(cx).diff_mode(cx), DiffMode::Unified);
        mrs.update(cx, |v, cx| v.toggle_diff_mode(cx));
        assert_eq!(mrs.read(cx).diff_mode(cx), DiffMode::Split);

        // Preferences navigation and live UI font scaling shortcuts.
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.dispatch_action(Box::new(GoPreferences), cx);
        win.dispatch_action(Box::new(ZoomIn), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::Preferences);
        assert_eq!(state.read(cx).prefs.ui_font_size(), 17);
        win.draw(cx).clear(cx);
        win.dispatch_action(Box::new(ZoomReset), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).prefs.ui_font_size(), 16);
    })
    .unwrap();
}

#[gpui_kit::test]
fn back_and_forward_navigation_tracks_screens_dirs_files_and_clears_on_host_switch(
    cx: &mut TestAppContext,
) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        let state = shell.read(cx).state().clone();
        assert!(!state.read(cx).can_go_back());
        assert!(!state.read(cx).can_go_forward());

        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project(proj, cx));
        state.update(cx, |s, cx| s.record_overview_dir("main".into(), "src".into(), cx));
        state.update(cx, |s, cx| s.set_screen(Screen::MergeRequests, cx));
        state.update(cx, |s, cx| {
            s.open_repo_file(101, Some("main".into()), "src/main.rs".into(), None, None, cx)
        });
        assert_eq!(state.read(cx).screen, Screen::Repository);
        assert!(state.read(cx).can_go_back());
        assert!(!state.read(cx).can_go_forward());

        win.dispatch_action(Box::new(NavBack), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::MergeRequests);
        assert!(state.read(cx).can_go_forward());
        win.dispatch_action(Box::new(NavBack), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        assert_eq!(
            state.read(cx).history.current().target,
            NavTarget::OverviewDir { git_ref: "main".into(), path: "src".into() }
        );
        win.dispatch_action(Box::new(NavBack), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        assert_eq!(
            state.read(cx).history.current().target,
            NavTarget::OverviewDir { git_ref: "main".into(), path: String::new() }
        );
        win.dispatch_action(Box::new(NavBack), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::Projects);
        assert!(!state.read(cx).can_go_back());
        win.dispatch_action(Box::new(NavForward), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        assert!(state.read(cx).can_go_back());
        assert!(state.read(cx).can_go_forward());

        state.update(cx, |s, cx| s.select_host("code.internal.test", cx));
        assert!(!state.read(cx).can_go_back());
        assert!(!state.read(cx).can_go_forward());
        win.draw(cx).clear(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn sidebar_project_block_has_individual_targets_and_collapsed_layout_is_aligned(
    cx: &mut TestAppContext,
) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project(proj, cx));
        win.render_frame(cx);

        let card = win.find("sidebar-project-card");
        let pinned_toggle = win.find("sidebar-pinned-toggle");
        let pinned_issues = win.find("pinned-issues");
        let pinned_mrs = win.find("pinned-merge_requests");
        assert!(card.visible());
        assert!(pinned_toggle.visible());
        assert!(pinned_issues.visible());
        assert!(pinned_mrs.visible());
        assert!(card.bounds().bottom() <= pinned_toggle.bounds().top());
        assert!(pinned_toggle.bounds().bottom() <= pinned_issues.bounds().top());
        assert!(pinned_issues.bounds().bottom() <= pinned_mrs.bounds().top());

        win.click("pinned-issues", cx);
        assert_eq!(state.read(cx).screen, Screen::Issues);
        win.click("sidebar-project-card", cx);
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        win.click("sidebar-pinned-toggle", cx);
        win.render_frame(cx);
        assert!(win.try_find("pinned-issues").is_none());

        win.click("sidebar-collapse-button", cx);
        assert!(state.read(cx).sidebar_collapsed);
    })
    .unwrap();

    let collapsed_prefs = Prefs { sidebar_collapsed: true, ..Prefs::default() };
    let (cwin, cshell) = mount_shell_with_prefs(cx, collapsed_prefs);
    cx.update_window(cwin, |_, win, cx| {
        let state = cshell.read(cx).state().clone();
        let proj = state.read(cx).projects[0].clone();
        state.update(cx, |s, cx| s.select_project(proj, cx));
        win.render_frame(cx);

        let card = win.find("sidebar-project-card").bounds();
        let avatar = win.find("sidebar-project-avatar").bounds();
        let first_nav = win.find("0-0-0").bounds();
        let last_proj_nav = win.find("0-0-8").bounds();
        let first_tools_nav = win.find("1-0-0").bounds();
        let theme_btn = win.find("toggle-theme").bounds();
        let expand_btn = win.find("collapse").bounds();

        let axis = first_nav.center().x;
        for (name, b) in [
            ("project-card", card),
            ("project-avatar", avatar),
            ("first-tools-nav", first_tools_nav),
            ("toggle-theme", theme_btn),
            ("expand-btn", expand_btn),
        ] {
            assert!(
                b.left() >= px(0.) && b.right() <= px(48.),
                "{name} overflows collapsed 48px sidebar: {b:?}"
            );
            let dx = (b.center().x - axis).abs();
            assert!(
                dx <= px(1.0),
                "{name} center {:?} misaligned from nav icon axis {axis:?} (bounds: {b:?})",
                b.center().x
            );
        }
        assert!(
            expand_btn.top() >= theme_btn.bottom(),
            "collapsed footer buttons should stack vertically: theme={theme_btn:?}, expand={expand_btn:?}"
        );
        assert!(
            first_tools_nav.top() - last_proj_nav.bottom() >= px(8.),
            "collapsed Tools group should be separated from project nav: last={last_proj_nav:?}, tools={first_tools_nav:?}"
        );

        win.click("collapse", cx);
        assert!(!state.read(cx).sidebar_collapsed);
    })
    .unwrap();
}
