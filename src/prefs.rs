use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_UI_FONT_SIZE: u16 = 16;
pub const MIN_UI_FONT_SIZE: u16 = 12;
pub const MAX_UI_FONT_SIZE: u16 = 22;
pub const DEFAULT_MONO_FONT_SIZE: u16 = 13;
pub const MIN_MONO_FONT_SIZE: u16 = 10;
pub const MAX_MONO_FONT_SIZE: u16 = 22;
pub const DEFAULT_RADIUS: u16 = 6;
pub const DEFAULT_SIDEBAR_WIDTH: u16 = 248;
pub const DEFAULT_UI_FONT: &str = ".SystemUIFont";
pub const DEFAULT_MONO_FONT: &str = "JetBrains Mono";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prefs {
    pub host: Option<String>,
    pub project: Option<String>,
    #[serde(default)]
    pub theme_mode: Option<String>,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub ui_font_size: Option<u16>,
    #[serde(default)]
    pub ui_font_family: Option<String>,
    #[serde(default)]
    pub mono_font_size: Option<u16>,
    #[serde(default)]
    pub mono_font_family: Option<String>,
    #[serde(default)]
    pub radius: Option<u16>,
    #[serde(default)]
    pub sidebar_width: Option<u16>,
    #[serde(default)]
    pub default_diff_split: bool,
    #[serde(default)]
    pub default_log_wrap: bool,
    #[serde(default)]
    pub pinned_screens: Vec<String>,
}

impl Prefs {
    pub fn is_dark(&self) -> bool {
        self.theme_mode.as_deref() != Some("light")
    }

    pub fn ui_font_size(&self) -> u16 {
        self.ui_font_size
            .unwrap_or(DEFAULT_UI_FONT_SIZE)
            .clamp(MIN_UI_FONT_SIZE, MAX_UI_FONT_SIZE)
    }

    pub fn ui_font_family(&self) -> &str {
        self.ui_font_family
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_UI_FONT)
    }

    pub fn mono_font_size(&self) -> u16 {
        self.mono_font_size
            .unwrap_or(DEFAULT_MONO_FONT_SIZE)
            .clamp(MIN_MONO_FONT_SIZE, MAX_MONO_FONT_SIZE)
    }

    pub fn mono_font_family(&self) -> &str {
        self.mono_font_family
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_MONO_FONT)
    }

    pub fn radius(&self) -> u16 {
        self.radius.unwrap_or(DEFAULT_RADIUS).min(14)
    }

    pub fn sidebar_width(&self) -> u16 {
        self.sidebar_width
            .unwrap_or(DEFAULT_SIDEBAR_WIDTH)
            .clamp(200, 340)
    }

    pub fn pinned(&self) -> Vec<crate::nav::Screen> {
        let resolved: Vec<_> = self
            .pinned_screens
            .iter()
            .filter_map(|k| crate::nav::Screen::from_key(k))
            .collect();
        if resolved.is_empty() {
            vec![crate::nav::Screen::Issues, crate::nav::Screen::MergeRequests]
        } else {
            resolved
        }
    }

    pub fn reset_ui_defaults(&mut self) {
        self.theme_mode = Some("dark".into());
        self.sidebar_collapsed = false;
        self.ui_font_size = Some(DEFAULT_UI_FONT_SIZE);
        self.ui_font_family = None;
        self.mono_font_size = Some(DEFAULT_MONO_FONT_SIZE);
        self.mono_font_family = None;
        self.radius = Some(DEFAULT_RADIUS);
        self.sidebar_width = Some(DEFAULT_SIDEBAR_WIDTH);
        self.default_diff_split = false;
        self.default_log_wrap = false;
    }
}

pub fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("gitlab-for-mac").join("prefs.json"))
}

fn legacy_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("glab-mac").join("prefs.json"))
}

pub fn load() -> Prefs {
    if let Some(p) = path().filter(|p| p.exists()) {
        return load_from(&p);
    }
    legacy_path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn load_from(path: &Path) -> Prefs {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(prefs: &Prefs) {
    if let Some(p) = path() {
        save_to(&p, prefs);
    }
}

pub fn save_to(path: &Path, prefs: &Prefs) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(prefs) {
        let _ = fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_legacy_prefs_with_defaults() {
        let prefs: Prefs = serde_json::from_str(r#"{"host":"gitlab.com","project":"g/p"}"#).unwrap();
        assert_eq!(prefs.host.as_deref(), Some("gitlab.com"));
        assert_eq!(prefs.project.as_deref(), Some("g/p"));
        assert_eq!(prefs.theme_mode, None);
        assert!(!prefs.sidebar_collapsed);
        assert_eq!(prefs.ui_font_size(), DEFAULT_UI_FONT_SIZE);
        assert_eq!(prefs.mono_font_size(), DEFAULT_MONO_FONT_SIZE);
        assert_eq!(prefs.ui_font_family(), DEFAULT_UI_FONT);
        assert_eq!(prefs.mono_font_family(), DEFAULT_MONO_FONT);
        assert_eq!(prefs.radius(), DEFAULT_RADIUS);
        assert_eq!(prefs.sidebar_width(), DEFAULT_SIDEBAR_WIDTH);
    }

    #[test]
    fn clamps_out_of_range_font_and_layout_values() {
        let mut p = Prefs {
            ui_font_size: Some(99),
            mono_font_size: Some(4),
            radius: Some(50),
            sidebar_width: Some(50),
            ..Prefs::default()
        };
        assert_eq!(p.ui_font_size(), MAX_UI_FONT_SIZE);
        assert_eq!(p.mono_font_size(), MIN_MONO_FONT_SIZE);
        assert_eq!(p.radius(), 14);
        assert_eq!(p.sidebar_width(), 200);
        p.reset_ui_defaults();
        assert_eq!(p.ui_font_size(), DEFAULT_UI_FONT_SIZE);
    }
}
