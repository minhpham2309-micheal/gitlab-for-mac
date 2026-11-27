use std::path::PathBuf;

use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gpui_app::{
    actions::{CheckForUpdates, DismissOverlay},
    init_app,
    shell::AppShell,
    state::{AppState, UpdatePhase},
};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;
use gitlab_for_mac::updater::{InstallBlocker, InstallTarget, UpdateChannel, UpdateInfo};

fn sample_update() -> UpdateInfo {
    UpdateInfo {
        tag: "v0.2.0".into(),
        version: "0.2.0".into(),
        name: "GitLab for Mac v0.2.0".into(),
        notes: "## What's Changed\n- Auto-update support\n- Faster pipeline graph".into(),
        published_at: Some("2026-11-27T06:00:00Z".into()),
        html_url: "https://github.com/example/gitlab-for-mac/releases/tag/v0.2.0".into(),
        asset_name: "GitLab-for-Mac-v0.2.0-arm64.app.zip".into(),
        asset_download_url: "https://github.com/example/gitlab-for-mac/releases/download/v0.2.0/GitLab-for-Mac-v0.2.0-arm64.app.zip".into(),
        asset_api_url: String::new(),
        asset_size: 14_500_000,
        checksums_download_url: Some("https://github.com/example/gitlab-for-mac/releases/download/v0.2.0/SHA256SUMS.txt".into()),
        checksums_api_url: None,
        dmg_download_url: Some("https://github.com/example/gitlab-for-mac/releases/download/v0.2.0/GitLab-for-Mac-v0.2.0-arm64.dmg".into()),
        is_prerelease: false,
        canary_id: None,
    }
}

fn mount_shell(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    cx.update(|cx| {
        let host = HostAuth {
            hostname: "gitlab.example.com".into(),
            api_host: "gitlab.example.com".into(),
            api_protocol: "https".into(),
            token: "test-token-fixture".into(),
            user: Some("alice".into()),
        };
        let state = cx.new(|_| {
            AppState::from_parts(vec![host.clone()], Some(host), Vec::new(), Prefs::default(), "Ready")
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

#[gpui_kit::test]
fn update_modal_lifecycle_dismissal_and_install_lock(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.draw(cx).clear(cx);

        let state = shell.read(cx).state().clone();
        assert!(!state.read(cx).update.modal_open);
        assert!(!state.read(cx).update.should_show_topbar_pill(None));

        // Command palette CheckForUpdates action opens modal cleanly (offline mode leaves Idle).
        win.dispatch_action(Box::new(CheckForUpdates), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert!(state.read(cx).update.modal_open);
        assert!(shell.read(cx).has_overlay_open(cx));
        win.draw(cx).clear(cx);

        // Escape dismisses update modal.
        win.dispatch_action(Box::new(DismissOverlay), cx);
    })
    .unwrap();
    cx.run_until_parked();

    let info = sample_update();
    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert!(!state.read(cx).update.modal_open);

        // Available update shows TopBar pill until dismissed.
        state.update(cx, |s, cx| {
            s.update.install_target =
                InstallTarget::WritableBundle(PathBuf::from("/Applications/GitLab for Mac.app"));
            s.update.phase = UpdatePhase::Available(Box::new(info.clone()));
            cx.notify();
        });
        assert!(state.read(cx).update.should_show_topbar_pill(None));
        assert_eq!(state.read(cx).update.topbar_label().as_deref(), Some("Update v0.2.0"));
        win.draw(cx).clear(cx);

        // Skipping this version hides TopBar pill and persists version in prefs.
        state.update(cx, |s, cx| s.dismiss_available_update(cx));
        assert_eq!(state.read(cx).prefs.dismissed_update_version.as_deref(), Some("0.2.0"));
        assert!(!state
            .read(cx)
            .update
            .should_show_topbar_pill(state.read(cx).prefs.dismissed_update_version.as_deref()));

        // During Installing, modal cannot be dismissed via Escape.
        state.update(cx, |s, cx| {
            s.update.phase = UpdatePhase::Installing { info: Box::new(info.clone()) };
            s.update.modal_open = true;
            cx.notify();
        });
        win.draw(cx).clear(cx);
        win.dispatch_action(Box::new(DismissOverlay), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, _, cx| {
        let state = shell.read(cx).state().clone();
        assert!(state.read(cx).update.modal_open);
    })
    .unwrap();
}

#[gpui_kit::test]
fn renders_all_update_phases_and_preferences_updates_card(cx: &mut TestAppContext) {
    let (window, shell) = mount_shell(cx);
    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        let info = sample_update();

        // Navigate to Preferences to exercise the Software Updates card.
        state.update(cx, |s, cx| {
            s.set_screen(Screen::Preferences, cx);
            s.set_auto_check_updates(false, cx);
            s.set_update_channel(UpdateChannel::Canary, cx);
        });
        assert!(!state.read(cx).prefs.auto_check_updates());
        assert_eq!(state.read(cx).prefs.update_channel, UpdateChannel::Canary);
        win.draw(cx).clear(cx);

        // Draw every modal phase with both WritableBundle and Blocked install targets.
        let phases = vec![
            UpdatePhase::Checking,
            UpdatePhase::UpToDate {
                checked_at: 1_700_000_000,
                note: Some("No Stable release published yet · Switch to Canary for rolling builds.".into()),
            },
            UpdatePhase::Available(Box::new(info.clone())),
            UpdatePhase::Downloading {
                info: Box::new(info.clone()),
                downloaded: 6_000_000,
                total: Some(14_500_000),
            },
            UpdatePhase::Verifying { info: Box::new(info.clone()) },
            UpdatePhase::Installing { info: Box::new(info.clone()) },
            UpdatePhase::Error {
                message: "SHA-256 verification failed".into(),
                info: Some(Box::new(info.clone())),
            },
        ];

        for target in [
            InstallTarget::WritableBundle(PathBuf::from("/Applications/GitLab for Mac.app")),
            InstallTarget::Blocked(InstallBlocker::AppTranslocated),
        ] {
            for phase in &phases {
                state.update(cx, |s, cx| {
                    s.update.install_target = target.clone();
                    s.update.phase = phase.clone();
                    s.update.modal_open = true;
                    cx.notify();
                });
                win.draw(cx).clear(cx);
            }
        }
        // Verify long status/error messages wrap without overflowing Check for Updates outside card.
        state.update(cx, |s, cx| {
            s.update.modal_open = false;
            s.update.phase = UpdatePhase::Error {
                message: "No Stable release published yet — switch Release channel to Canary to test rolling builds and verify long wrapping text stays inside card bounds.".into(),
                info: None,
            };
            cx.notify();
        });
        win.render_frame(cx);
        let card = win.find("pref-updates-card").bounds();
        let controls = win.find("pref-update-controls").bounds();
        assert!(
            controls.right() <= card.right() && controls.left() >= card.left(),
            "Check for Updates controls overflowed Software Updates card: controls={controls:?}, card={card:?}"
        );
    })
    .unwrap();
}
