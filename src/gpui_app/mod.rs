pub mod actions;
pub mod assets;
pub mod avatar;
pub mod breadcrumb;
pub mod confirm;
pub mod diff_hl;
pub mod image_cache;
pub mod markdown;
pub mod nav_history;
pub mod pajamas_theme;
pub mod palette;
pub mod ref_switcher;
pub mod repo_cache;
pub mod safe_url;
pub mod screens;
pub mod shell;
mod shell_actions;
pub mod sidebar;
mod sidebar_footer;
mod sidebar_project_card;
pub mod state;
pub mod state_auth;
mod state_loaders;
mod state_nav;
pub mod state_types;
pub mod state_updater;
mod state_updater_install;
pub mod statusbar;
pub mod topbar;
mod topbar_right;
pub mod update_modal;
mod update_modal_body;

use std::borrow::Cow;

use gpui_kit::{
    component::TitleBar, px, size, AppContext as _, Bounds, WindowBounds, WindowOptions,
};

use crate::runtime::TokioBridge;

use self::assets::GlabAssets;
use self::shell::AppShell;
use self::state::AppState;

const JETBRAINS_MONO_REGULAR: &[u8] = include_bytes!("../../fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../../fonts/JetBrainsMono-Bold.ttf");

pub fn register_bundled_fonts(cx: &gpui_kit::App) {
    let _ = cx.text_system().add_fonts(vec![
        Cow::Borrowed(JETBRAINS_MONO_REGULAR),
        Cow::Borrowed(JETBRAINS_MONO_BOLD),
    ]);
}

pub fn init_app(cx: &mut gpui_kit::App) {
    gpui_kit::init(cx);
    TokioBridge::init(cx);
    image_cache::ImageCache::init(cx);
    repo_cache::RepoCache::init(cx);
    actions::bind_keys(cx);
    register_bundled_fonts(cx);
}

pub fn run() {
    gpui_kit::application()
        .with_assets(GlabAssets)
        .run(|cx| {
            init_app(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1024.), px(680.))),
                ..TitleBar::window_options()
            };

            let state = cx.new(AppState::boot);
            gpui_kit::open_window(options, cx, move |window, cx| {
                let state = state.clone();
                cx.new(|cx| AppShell::new(state, window, cx))
            })
            .expect("failed to open gitlab-for-mac window");
            cx.activate(true);
        });
}
