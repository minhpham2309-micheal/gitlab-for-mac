use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use gpui_kit::{
    px, size,
    test::TestWindowExt as _,
    AppContext as _, Bounds, Focusable as _, TestAppContext, WindowBounds, WindowOptions,
};

use gitlab_for_mac::accounts::{
    build_host_auth, load_saved_from, merge_hosts, personal_access_token_url, remove_account_in,
    upsert_account_in, StoredAccount,
};
use gitlab_for_mac::config::HostAuth;
use gitlab_for_mac::gpui_app::{
    actions::{AddAccount, DismissOverlay, SignOut},
    init_app,
    shell::AppShell,
    state::AppState,
};
use gitlab_for_mac::nav::Screen;
use gitlab_for_mac::prefs::Prefs;

fn sample_host(hostname: &str, user: &str) -> HostAuth {
    HostAuth {
        hostname: hostname.into(),
        api_host: hostname.into(),
        api_protocol: "https".into(),
        token: "test-pat-fixture".into(),
        user: Some(user.into()),
    }
}

fn mount_with_host(
    cx: &mut TestAppContext,
    active: Option<HostAuth>,
    saved: Vec<HostAuth>,
    glab: Vec<HostAuth>,
) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<AppShell>) {
    cx.update(init_app);
    cx.update(|cx| {
        let merged = merge_hosts(&saved, &glab);
        let state = cx.new(|_| {
            let mut st = AppState::from_parts(merged, active, Vec::new(), Prefs::default(), "Ready");
            st.auth.saved_hosts = saved;
            st.auth.glab_hosts = glab;
            st
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

#[test]
fn accounts_store_persists_with_0600_redacts_debug_and_validates_hosts() {
    let dir = std::env::temp_dir().join(format!("glab-auth-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("accounts.json");

    let host = build_host_auth("https://gitlab.example.com/", "test-token-value").unwrap();
    assert_eq!(host.hostname, "gitlab.example.com");
    assert_eq!(host.api_protocol, "https");

    // StoredAccount Debug never prints token.
    let stored = StoredAccount::from(&host);
    let dbg = format!("{stored:?}");
    assert!(dbg.contains("[REDACTED]"));
    assert!(!dbg.contains("test-token-value"));

    // Reject insecure non-loopback http and embedded credentials.
    assert!(build_host_auth("http://gitlab.example.com", "tok").is_err());
    assert!(build_host_auth("https://user:pass@gitlab.example.com", "tok").is_err());
    assert!(build_host_auth("gitlab.example.com", "   ").is_err());
    assert!(build_host_auth("http://127.0.0.1:8080", "tok").is_ok());

    let pat_url = personal_access_token_url("gitlab.example.com").unwrap();
    assert!(pat_url.starts_with("https://gitlab.example.com/-/user_settings/personal_access_tokens?"));

    let saved = upsert_account_in(&path, &host).unwrap();
    assert_eq!(saved.len(), 1);
    #[cfg(unix)]
    {
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "accounts.json must be 0600");
    }

    let loaded = load_saved_from(&path);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].hostname, "gitlab.example.com");

    let remaining = remove_account_in(&path, "gitlab.example.com").unwrap();
    assert!(remaining.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[gpui_kit::test]
fn fresh_install_starts_on_login_then_signs_in_and_logs_out(cx: &mut TestAppContext) {
    let glab_host = sample_host("gitlab.example.com", "alice");
    let (window, shell) = mount_with_host(cx, None, Vec::new(), vec![glab_host.clone()]);

    cx.update_window(window, |_, win, cx| {
        let focus = shell.read(cx).focus_handle(cx);
        win.focus(&focus, cx);
        win.render_frame(cx);

        let state = shell.read(cx).state().clone();
        assert!(!state.read(cx).is_authenticated());
        assert!(!state.read(cx).prefs.is_signed_in());

        // Sign in via discovered glab host.
        state.update(cx, |s, cx| s.sign_in_with_existing_host(glab_host.clone(), cx));
        assert!(state.read(cx).is_authenticated());
        assert!(state.read(cx).prefs.is_signed_in());
        assert_eq!(
            state.read(cx).active_host.as_ref().map(|h| h.hostname.as_str()),
            Some("gitlab.example.com")
        );
        win.render_frame(cx);

        // Open Preferences and verify Account card layout bounds stay inside card.
        state.update(cx, |s, cx| s.set_screen(Screen::Preferences, cx));
        win.render_frame(cx);
        let card = win.find("pref-account-card").bounds();
        let controls = win.find("pref-account-controls").bounds();
        assert!(
            controls.right() <= card.right() && controls.left() >= card.left(),
            "Account controls overflowed card: controls={controls:?}, card={card:?}"
        );

        // AddAccount switches to login overlay while keeping active session, Escape returns.
        win.dispatch_action(Box::new(AddAccount), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert!(state.read(cx).auth.adding_account);
        assert!(!state.read(cx).is_authenticated());
        win.render_frame(cx);
        win.dispatch_action(Box::new(DismissOverlay), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert!(state.read(cx).is_authenticated());
        assert!(!state.read(cx).auth.adding_account);
        win.render_frame(cx);

        // SignOut clears active session, marks signed_in=false, and returns to Login screen.
        win.dispatch_action(Box::new(SignOut), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, win, cx| {
        let state = shell.read(cx).state().clone();
        assert!(!state.read(cx).is_authenticated());
        assert!(state.read(cx).active_host.is_none());
        assert!(state.read(cx).gitlab.is_none());
        assert_eq!(state.read(cx).prefs.signed_in, Some(false));
        win.render_frame(cx);
    })
    .unwrap();
}
