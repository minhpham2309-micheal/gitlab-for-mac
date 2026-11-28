use gpui_kit::{
    component::{Theme, ThemeColor, ThemeMode},
    px, rgb, App, Hsla,
};

use crate::prefs::Prefs;

pub const TANUKI_HEX: u32 = 0xFC6D26;

#[derive(Debug, Clone, Copy)]
pub struct StatusBadgePalette {
    pub bg: Hsla,
    pub icon: Hsla,
    pub text: Hsla,
}

pub fn tanuki_color() -> Hsla {
    rgb(TANUKI_HEX).into()
}

pub fn studio_shell_bg(dark: bool) -> Hsla {
    rgb(if dark { 0x050505 } else { 0xF0F0F2 }).into()
}

pub fn studio_panel_border(dark: bool) -> Hsla {
    rgb(if dark { 0x383839 } else { 0xDCDCDE }).into()
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
    // GitLab 18 Studio (`gl-dark ui-gray page-with-panels`) neutral scale.
    c.background = rgb(0x171717).into();
    c.foreground = rgb(0xECECEC).into();
    c.sidebar = rgb(0x050505).into();
    c.sidebar_foreground = rgb(0xECECEC).into();
    c.sidebar_border = rgb(0x272727).into();
    c.sidebar_accent = rgb(0x272727).into();
    c.sidebar_accent_foreground = rgb(0xFFFFFF).into();
    c.sidebar_primary = rgb(0xECECEC).into();
    c.sidebar_primary_foreground = rgb(0x171717).into();
    c.title_bar = rgb(0x050505).into();
    c.title_bar_border = rgb(0x050505).into();
    c.status_bar = rgb(0x050505).into();
    c.status_bar_border = rgb(0x1F1F20).into();
    c.border = rgb(0x383839).into();
    c.input = rgb(0x4B4B4B).into();
    c.popover = rgb(0x272727).into();
    c.popover_foreground = rgb(0xECECEC).into();
    c.secondary = rgb(0x272727).into();
    c.secondary_hover = rgb(0x383839).into();
    c.secondary_active = rgb(0x4B4B4B).into();
    c.secondary_foreground = rgb(0xECECEC).into();
    c.accent = rgb(0x383839).into();
    c.accent_foreground = rgb(0xFFFFFF).into();
    c.muted = rgb(0x272727).into();
    c.muted_foreground = rgb(0x898889).into();
    c.list = rgb(0x171717).into();
    c.list_head = rgb(0x272727).into();
    c.list_hover = rgb(0x272727).into();
    c.list_active = rgb(0x2E2E30).into();
    c.list_active_border = rgb(0xECECEC).into();
    c.primary = rgb(0xECECEC).into();
    c.primary_hover = rgb(0xF8F8F8).into();
    c.primary_active = rgb(0xDCDCDC).into();
    c.primary_foreground = rgb(0x171717).into();
    c.button_primary = c.primary;
    c.button_primary_hover = c.primary_hover;
    c.button_primary_active = c.primary_active;
    c.button_primary_foreground = c.primary_foreground;
    c.link = rgb(0x9DC8F2).into();
    c.link_hover = rgb(0xCBE2F9).into();
    c.link_active = rgb(0x64A7E9).into();
    c.ring = rgb(0xECECEC).into();
    c.caret = rgb(TANUKI_HEX).into();
    c.success = rgb(0x35B597).into();
    c.success_foreground = rgb(0x171717).into();
    c.warning = rgb(0xFB8017).into();
    c.warning_foreground = rgb(0x171717).into();
    c.danger = rgb(0xDD2B0E).into();
    c.danger_hover = rgb(0xEC5941).into();
    c.danger_foreground = rgb(0xFFFFFF).into();
    c.button_danger = c.danger;
    c.button_danger_hover = c.danger_hover;
    c.button_danger_foreground = c.danger_foreground;
    c.info = rgb(0x64A7E9).into();
    c.info_foreground = rgb(0x171717).into();
    c.tab_bar = rgb(0x171717).into();
    c.tab_active = rgb(0x272727).into();
    c.tab_active_foreground = rgb(0xFFFFFF).into();
    c
}

pub fn pajamas_light() -> ThemeColor {
    let mut c = *ThemeColor::light();
    c.background = rgb(0xFFFFFF).into();
    c.foreground = rgb(0x28272D).into();
    c.sidebar = rgb(0xF0F0F2).into();
    c.sidebar_foreground = rgb(0x28272D).into();
    c.sidebar_border = rgb(0xDCDCDE).into();
    c.sidebar_accent = rgb(0xE2E2E6).into();
    c.sidebar_accent_foreground = rgb(0x171717).into();
    c.sidebar_primary = rgb(0x28272D).into();
    c.sidebar_primary_foreground = rgb(0xFFFFFF).into();
    c.title_bar = rgb(0xF0F0F2).into();
    c.title_bar_border = rgb(0xF0F0F2).into();
    c.status_bar = rgb(0xF0F0F2).into();
    c.status_bar_border = rgb(0xDCDCDE).into();
    c.border = rgb(0xDCDCDE).into();
    c.input = rgb(0xBFBFBF).into();
    c.popover = rgb(0xFFFFFF).into();
    c.popover_foreground = rgb(0x28272D).into();
    c.secondary = rgb(0xF4F4F6).into();
    c.secondary_hover = rgb(0xE8E8EC).into();
    c.secondary_active = rgb(0xDCDCDE).into();
    c.secondary_foreground = rgb(0x28272D).into();
    c.accent = rgb(0xECECEF).into();
    c.accent_foreground = rgb(0x171717).into();
    c.muted = rgb(0xF4F4F7).into();
    c.muted_foreground = rgb(0x626168).into();
    c.list = rgb(0xFFFFFF).into();
    c.list_head = rgb(0xF8F8FA).into();
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
    c.tab_bar = rgb(0xFFFFFF).into();
    c.tab_active = rgb(0xF4F4F6).into();
    c.tab_active_foreground = rgb(0x171717).into();
    c
}

pub fn status_badge_palette(status: &str, dark: bool) -> StatusBadgePalette {
    let (bg, icon, text) = match status {
        "success" | "passed" | "opened" | "open" | "active" | "online" => {
            if dark { (0x075048, 0x35B597, 0x84D4B6) } else { (0xD7F5E5, 0x108548, 0x0A5E32) }
        }
        "merged" | "running" | "pending" | "created" | "waiting_for_resource"
        | "preparing" | "waiting_for_callback" | "canceling" => {
            if dark { (0x254581, 0x64A7E9, 0x9DC8F2) } else { (0xDFEEFC, 0x1F75CB, 0x104E8B) }
        }
        "failed" | "closed" => {
            if dark { (0x891C2F, 0xFC7988, 0xFFAFB8) } else { (0xFDDAD6, 0xDD2B0E, 0xA81D08) }
        }
        "manual" | "locked" | "warning" => {
            if dark { (0x7D2D0E, 0xFB8017, 0xF7B951) } else { (0xFDF1DD, 0xAB6100, 0x7A4400) }
        }
        _ => if dark { (0x383839, 0xA3A3A3, 0xBFBFBF) } else { (0xECECEF, 0x626168, 0x4A4950) },
    };
    StatusBadgePalette { bg: rgb(bg).into(), icon: rgb(icon).into(), text: rgb(text).into() }
}

pub fn status_color(status: &str, dark: bool) -> Hsla {
    status_badge_palette(status, dark).icon
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
