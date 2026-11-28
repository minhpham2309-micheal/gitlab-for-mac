use std::collections::HashMap;

use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gitlab::{Group, GroupChildren, GroupTab, Project};
use gitlab_for_mac::gpui_app::{
    actions::{NavBack, NavForward},
    init_app,
    nav_history::NavTarget,
    shell::AppShell,
    state::AppState,
};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_group(id: u64, name: &str, full_path: &str) -> Group {
    Group {
        id,
        name: name.into(),
        path: name.into(),
        full_name: full_path.replace('/', " / "),
        full_path: full_path.into(),
        web_url: format!("https://gitlab.example.com/{full_path}"),
        description: Some(format!("Group {name}")),
        visibility: Some("private".into()),
        avatar_url: None,
        parent_id: None,
        created_at: Some("2026-06-01T00:00:00Z".into()),
        marked_for_deletion_on: None,
    }
}

fn sample_project(id: u64, name: &str, full_path: &str) -> Project {
    Project {
        id,
        name: name.into(),
        path_with_namespace: full_path.into(),
        default_branch: Some("main".into()),
        web_url: format!("https://gitlab.example.com/{full_path}"),
        last_activity_at: Some("2026-11-26T00:00:00Z".into()),
        star_count: 3,
        avatar_url: None,
    }
}

fn mount_groups_shell(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
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
            AppState::from_parts(vec![host.clone()], Some(host), vec![], Prefs::default(), "Ready")
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
fn groups_explorer_supports_inline_expand_drilldown_project_open_and_breadcrumb_return(
    cx: &mut TestAppContext,
) {
    let (window, shell) = mount_groups_shell(cx);

    let backend = sample_group(10, "backend", "acme/backend");
    let game = sample_group(20, "game", "acme/backend/game");
    let utility = sample_group(21, "utility", "acme/backend/utility");
    let gateway = sample_project(301, "gateway", "acme/backend/gateway");
    let service_alpha = sample_project(302, "service-alpha", "acme/backend/game/service-alpha");

    let mut children = HashMap::new();
    children.insert(20, GroupChildren { subgroups: vec![], projects: vec![service_alpha.clone()] });

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        state.update(cx, |s, cx| s.set_screen(Screen::Groups, cx));
        let gv = shell.read(cx).groups_view().clone();
        gv.update(cx, |v, cx| {
            v.set_fixture_data(
                Some(backend.clone()),
                vec![game.clone(), utility.clone()],
                vec![gateway.clone()],
                children.clone(),
                cx,
            );
        });
        win.render_frame(cx);

        let header = win.find("group-detail-header").bounds();
        let filter_bar = win.find("groups-filter-bar").bounds();
        let game_row = win.find("group-row-20").bounds();
        let gateway_row = win.find("group-proj-row-301").bounds();

        assert!(header.bottom() <= filter_bar.top());
        assert!(filter_bar.bottom() <= game_row.top());
        assert!(game_row.bottom() <= gateway_row.top());
        assert!(win.try_find("group-proj-row-302").is_none());

        // 1. Click inline expand chevron -> reveals nested project without navigating away.
        win.click("group-expand-20", cx);
        win.render_frame(cx);
        assert!(gv.read(cx).is_expanded(20));
        assert_eq!(gv.read(cx).active_group().map(|g| g.id), Some(10));
        let nested_row = win.find("group-proj-row-302").bounds();
        assert!(game_row.bottom() <= nested_row.top());
        assert!(nested_row.bottom() <= gateway_row.top());

        // 2. Switch tabs cleanly.
        win.click("group-tab-Shared projects", cx);
        assert_eq!(gv.read(cx).tab(), GroupTab::SharedProjects);
        win.click("group-tab-Subgroups and projects", cx);
        assert_eq!(gv.read(cx).tab(), GroupTab::SubgroupsAndProjects);

        // 3. Click subgroup row -> drills into acme/backend/game and updates breadcrumb.
        win.click("group-row-20", cx);
        win.render_frame(cx);
        assert_eq!(state.read(cx).active_group_path.as_deref(), Some("acme/backend/game"));
        assert_eq!(
            state.read(cx).history.current().target,
            NavTarget::Group { full_path: Some("acme/backend/game".into()) }
        );
        assert!(win.find("bc-groups-root").visible());
        assert!(win.find("bc-group-seg-1").visible());
        assert!(win.find("group-proj-row-302").visible());

        // 4. Click project row inside subgroup -> opens ProjectOverview.
        win.click("group-proj-row-302", cx);
        win.render_frame(cx);
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        assert_eq!(state.read(cx).project.as_ref().map(|p| p.id), Some(302));

        // 5. Clicking ancestor namespace segment ("backend") in ProjectOverview breadcrumb jumps to Groups!
        win.click("ns-seg-1", cx);
        win.render_frame(cx);
        assert_eq!(state.read(cx).screen, Screen::Groups);
        assert_eq!(state.read(cx).active_group_path.as_deref(), Some("acme/backend"));

        // 6. Browser-style Back/Forward returns to ProjectOverview and back to Group.
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);
        win.dispatch_action(Box::new(NavBack), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::ProjectOverview);
        win.dispatch_action(Box::new(NavForward), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        assert_eq!(state.read(cx).screen, Screen::Groups);
        assert_eq!(state.read(cx).active_group_path.as_deref(), Some("acme/backend"));
    })
    .unwrap();
}
