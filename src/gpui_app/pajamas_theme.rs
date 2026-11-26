use gpui_kit::{
    component::{Theme, ThemeColor, ThemeMode},
    px, rgb, App, Hsla,
};

use crate::prefs::Prefs;

pub const TANUKI_HEX: u32 = 0xFC6D26;

pub fn tanuki_color() -> Hsla {
    rgb(TANUKI_HEX).into()
}

pub fn apply_theme_from_prefs(prefs: &Prefs, cx: &mut App) {
    let dark = prefs.is_dark();
    let mode = if dark { ThemeMode::Dark } else { ThemeMode::Light };
    Theme::change(mode, None, cx);
    let palette = if dark { pajamas_dark() } else { pajamas_light() };
    let ui_font = prefs.ui_font_family().to_string();
    let ui_size = prefs.ui_font_size() as f32;
    let mono_font = prefs.mono_font_family().to_string();
    let mono_size = prefs.mono_font_size() as f32;
    let radius = prefs.radius() as f32;
    Theme::update(cx, |theme| {
        theme.colors = palette;
        theme.font_family = ui_font.into();
        theme.font_size = px(ui_size);
        theme.mono_font_family = mono_font.into();
        theme.mono_font_size = px(mono_size);
        theme.radius = px(radius);
        theme.radius_lg = px((radius + 4.).min(18.));
    });
    cx.refresh_windows();
}

pub fn pajamas_dark() -> ThemeColor {
    let mut c = *ThemeColor::dark();
    c.background = rgb(0x18171D).into();
    c.foreground = rgb(0xECECEF).into();
    c.sidebar = rgb(0x1F1E24).into();
    c.sidebar_foreground = rgb(0xECECEF).into();
    c.sidebar_border = rgb(0x3A383F).into();
    c.sidebar_accent = rgb(0x2E2D35).into();
    c.sidebar_accent_foreground = rgb(0xFFFFFF).into();
    c.sidebar_primary = rgb(0x1F75CB).into();
    c.sidebar_primary_foreground = rgb(0xFFFFFF).into();
    c.title_bar = rgb(0x1F1E24).into();
    c.title_bar_border = rgb(0x3A383F).into();
    c.status_bar = rgb(0x1F1E24).into();
    c.status_bar_border = rgb(0x3A383F).into();
    c.border = rgb(0x3A383F).into();
    c.input = rgb(0x4C4B51).into();
    c.popover = rgb(0x24232A).into();
    c.popover_foreground = rgb(0xECECEF).into();
    c.secondary = rgb(0x28272D).into();
    c.secondary_hover = rgb(0x333238).into();
    c.secondary_active = rgb(0x3E3D45).into();
    c.secondary_foreground = rgb(0xECECEF).into();
    c.accent = rgb(0x333238).into();
    c.accent_foreground = rgb(0xFFFFFF).into();
    c.muted = rgb(0x28272D).into();
    c.muted_foreground = rgb(0x89888D).into();
    c.list = rgb(0x18171D).into();
    c.list_head = rgb(0x24232A).into();
    c.list_hover = rgb(0x292830).into();
    c.list_active = rgb(0x1E2F44).into();
    c.list_active_border = rgb(0x428FDC).into();
    c.primary = rgb(0x1F75CB).into();
    c.primary_hover = rgb(0x428FDC).into();
    c.primary_active = rgb(0x1068BF).into();
    c.primary_foreground = rgb(0xFFFFFF).into();
    c.button_primary = c.primary;
    c.button_primary_hover = c.primary_hover;
    c.button_primary_active = c.primary_active;
    c.button_primary_foreground = c.primary_foreground;
    c.link = rgb(0x63A6E9).into();
    c.link_hover = rgb(0x9DC7F1).into();
    c.link_active = rgb(0x428FDC).into();
    c.ring = rgb(0x63A6E9).into();
    c.caret = rgb(TANUKI_HEX).into();
    c.success = rgb(0x2DA160).into();
    c.success_foreground = rgb(0xFFFFFF).into();
    c.warning = rgb(0xD99530).into();
    c.warning_foreground = rgb(0x18171D).into();
    c.danger = rgb(0xDD2B0E).into();
    c.danger_hover = rgb(0xEC5941).into();
    c.danger_foreground = rgb(0xFFFFFF).into();
    c.button_danger = c.danger;
    c.button_danger_hover = c.danger_hover;
    c.button_danger_foreground = c.danger_foreground;
    c.info = rgb(0x428FDC).into();
    c.info_foreground = rgb(0xFFFFFF).into();
    c.tab_bar = rgb(0x1F1E24).into();
    c.tab_active = rgb(0x18171D).into();
    c.tab_active_foreground = rgb(0xFFFFFF).into();
    c
}

pub fn pajamas_light() -> ThemeColor {
    let mut c = *ThemeColor::light();
    c.background = rgb(0xFFFFFF).into();
    c.foreground = rgb(0x28272D).into();
    c.sidebar = rgb(0xFBFBFD).into();
    c.sidebar_foreground = rgb(0x28272D).into();
    c.sidebar_border = rgb(0xDCDCDE).into();
    c.sidebar_accent = rgb(0xECECEF).into();
    c.sidebar_accent_foreground = rgb(0x18171D).into();
    c.sidebar_primary = rgb(0x1F75CB).into();
    c.sidebar_primary_foreground = rgb(0xFFFFFF).into();
    c.title_bar = rgb(0xFBFBFD).into();
    c.title_bar_border = rgb(0xDCDCDE).into();
    c.status_bar = rgb(0xFBFBFD).into();
    c.status_bar_border = rgb(0xDCDCDE).into();
    c.border = rgb(0xDCDCDE).into();
    c.input = rgb(0xBFBFBF).into();
    c.popover = rgb(0xFFFFFF).into();
    c.popover_foreground = rgb(0x28272D).into();
    c.secondary = rgb(0xF0F0F4).into();
    c.secondary_hover = rgb(0xE6E6EC).into();
    c.secondary_active = rgb(0xDCDCDE).into();
    c.secondary_foreground = rgb(0x28272D).into();
    c.accent = rgb(0xECECEF).into();
    c.accent_foreground = rgb(0x18171D).into();
    c.muted = rgb(0xF4F4F7).into();
    c.muted_foreground = rgb(0x626168).into();
    c.list = rgb(0xFFFFFF).into();
    c.list_head = rgb(0xFBFBFD).into();
    c.list_hover = rgb(0xF0F0F4).into();
    c.list_active = rgb(0xE9F3FC).into();
    c.list_active_border = rgb(0x1F75CB).into();
    c.primary = rgb(0x1F75CB).into();
    c.primary_hover = rgb(0x1068BF).into();
    c.primary_active = rgb(0x0B5CAD).into();
    c.primary_foreground = rgb(0xFFFFFF).into();
    c.button_primary = c.primary;
    c.button_primary_hover = c.primary_hover;
    c.button_primary_active = c.primary_active;
    c.button_primary_foreground = c.primary_foreground;
    c.link = rgb(0x1068BF).into();
    c.link_hover = rgb(0x0B5CAD).into();
    c.link_active = rgb(0x1F75CB).into();
    c.ring = rgb(0x1F75CB).into();
    c.caret = rgb(TANUKI_HEX).into();
    c.success = rgb(0x108548).into();
    c.success_foreground = rgb(0xFFFFFF).into();
    c.warning = rgb(0xAB6100).into();
    c.warning_foreground = rgb(0xFFFFFF).into();
    c.danger = rgb(0xDD2B0E).into();
    c.danger_hover = rgb(0xC91C00).into();
    c.danger_foreground = rgb(0xFFFFFF).into();
    c.button_danger = c.danger;
    c.button_danger_hover = c.danger_hover;
    c.button_danger_foreground = c.danger_foreground;
    c.info = rgb(0x1F75CB).into();
    c.info_foreground = rgb(0xFFFFFF).into();
    c.tab_bar = rgb(0xFBFBFD).into();
    c.tab_active = rgb(0xFFFFFF).into();
    c.tab_active_foreground = rgb(0x18171D).into();
    c
}

pub fn status_color(status: &str, dark: bool) -> Hsla {
    match status {
        "success" | "passed" | "merged" | "active" | "online" => {
            rgb(if dark { 0x52B87A } else { 0x108548 }).into()
        }
        "failed" | "closed" => rgb(if dark { 0xEC5941 } else { 0xDD2B0E }).into(),
        "running" | "pending" | "created" | "waiting_for_resource" | "preparing"
        | "waiting_for_callback" | "canceling" => {
            rgb(if dark { 0x63A6E9 } else { 0x1F75CB }).into()
        }
        "manual" | "opened" | "open" | "locked" | "warning" => {
            rgb(if dark { 0xE9BE74 } else { 0xAB6100 }).into()
        }
        _ => rgb(if dark { 0x89888D } else { 0x626168 }).into(),
    }
}

pub fn status_icon_path(status: &str) -> &'static str {
    match status {
        "success" | "passed" | "merged" => "icons/glab-status-success.svg",
        "failed" => "icons/glab-status-failed.svg",
        "running" | "preparing" | "canceling" => "icons/glab-status-running.svg",
        "canceled" | "cancelled" => "icons/glab-status-canceled.svg",
        "manual" => "icons/glab-status-manual.svg",
        "pending" | "created" | "waiting_for_resource" | "waiting_for_callback" => {
            "icons/glab-status-pending.svg"
        }
        _ => "icons/glab-status-skipped.svg",
    }
}
