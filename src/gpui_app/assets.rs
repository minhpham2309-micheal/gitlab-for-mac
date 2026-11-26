use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

pub struct GlabAssets;

const CUSTOM_ICONS: &[(&str, &[u8])] = &[
    ("icons/glab-tanuki.svg", include_bytes!("../../assets/icons/glab-tanuki.svg")),
    ("icons/glab-status-success.svg", include_bytes!("../../assets/icons/glab-status-success.svg")),
    ("icons/glab-status-failed.svg", include_bytes!("../../assets/icons/glab-status-failed.svg")),
    ("icons/glab-status-running.svg", include_bytes!("../../assets/icons/glab-status-running.svg")),
    ("icons/glab-status-canceled.svg", include_bytes!("../../assets/icons/glab-status-canceled.svg")),
    ("icons/glab-status-manual.svg", include_bytes!("../../assets/icons/glab-status-manual.svg")),
    ("icons/glab-status-pending.svg", include_bytes!("../../assets/icons/glab-status-pending.svg")),
    ("icons/glab-status-skipped.svg", include_bytes!("../../assets/icons/glab-status-skipped.svg")),
    ("icons/glab-mr.svg", include_bytes!("../../assets/icons/glab-mr.svg")),
    ("icons/glab-issue.svg", include_bytes!("../../assets/icons/glab-issue.svg")),
    ("icons/glab-todo.svg", include_bytes!("../../assets/icons/glab-todo.svg")),
    ("icons/glab-branch.svg", include_bytes!("../../assets/icons/glab-branch.svg")),
    ("icons/glab-tag.svg", include_bytes!("../../assets/icons/glab-tag.svg")),
    ("icons/glab-rocket.svg", include_bytes!("../../assets/icons/glab-rocket.svg")),
    ("icons/glab-terminal.svg", include_bytes!("../../assets/icons/glab-terminal.svg")),
    ("icons/glab-search.svg", include_bytes!("../../assets/icons/glab-search.svg")),
    ("icons/glab-settings.svg", include_bytes!("../../assets/icons/glab-settings.svg")),
];

impl AssetSource for GlabAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = CUSTOM_ICONS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut items = gpui_kit::assets::Assets.list(path)?;
        for (icon_path, _) in CUSTOM_ICONS {
            if icon_path.starts_with(path) {
                items.push(SharedString::from(*icon_path));
            }
        }
        Ok(items)
    }
}
