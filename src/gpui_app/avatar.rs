use gpui_kit::{
    base::StyledExt as _,
    component::ActiveTheme as _,
    div, img, px, rgb, App, Hsla, IntoElement, ObjectFit, ParentElement as _, Pixels, Styled as _,
    StyledImage as _,
};

use crate::gitlab::GitLab;

use super::image_cache::ImageCache;

fn initial_char(name: &str) -> String {
    name.trim_start_matches('@')
        .chars()
        .next()
        .unwrap_or('G')
        .to_ascii_uppercase()
        .to_string()
}

fn entity_tint(name: &str, dark: bool) -> (Hsla, Hsla) {
    let hash = name.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    const DARK_TINTS: [(u32, u32); 6] = [
        (0x373742, 0xD8D8E6),
        (0x2D3C3A, 0xBEE3DC),
        (0x3D3547, 0xDDD0EC),
        (0x423A2E, 0xE6D7BE),
        (0x2E3845, 0xC6DAF0),
        (0x3F3338, 0xE8CED8),
    ];
    const LIGHT_TINTS: [(u32, u32); 6] = [
        (0xE8E8F2, 0x3A3A52),
        (0xDFF2EE, 0x1E5248),
        (0xEFE6F8, 0x4E336E),
        (0xF6EFE0, 0x6B4E1E),
        (0xE2EEFA, 0x1E4670),
        (0xF7E6EC, 0x6E2E44),
    ];
    let (bg, fg) = if dark { DARK_TINTS[hash % DARK_TINTS.len()] } else { LIGHT_TINTS[hash % LIGHT_TINTS.len()] };
    (rgb(bg).into(), rgb(fg).into())
}

fn fetch_cached(
    avatar_url: Option<&str>,
    client: Option<&GitLab>,
    cx: &mut App,
) -> Option<std::sync::Arc<gpui_kit::Image>> {
    avatar_url.filter(|u| !u.trim().is_empty()).and_then(|url| {
        let cache = cx.try_global::<ImageCache>().cloned()?;
        let client = client?;
        cache.get_or_fetch(client, url, None, cx)
    })
}

/// Circular avatar for people (GitLab users, authors, assignees, reviewers).
pub fn render_avatar(
    avatar_url: Option<&str>,
    fallback_name: &str,
    size_px: Pixels,
    client: Option<&GitLab>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let initial = initial_char(fallback_name);
    let cached = fetch_cached(avatar_url, client, cx);
    let dark = cx.theme().is_dark();
    let (bg, fg) = entity_tint(fallback_name, dark);
    if let Some(image) = cached {
        div()
            .size(size_px)
            .rounded_full()
            .overflow_hidden()
            .flex_shrink_0()
            .child(img(image).size(size_px).rounded_full().object_fit(ObjectFit::Cover))
    } else {
        div()
            .size(size_px)
            .rounded_full()
            .flex_shrink_0()
            .bg(bg)
            .text_color(fg)
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .font_semibold()
            .child(initial)
    }
}

/// Rounded-square identicon avatar for GitLab Projects and Groups.
pub fn render_entity_avatar(
    avatar_url: Option<&str>,
    fallback_name: &str,
    size_px: Pixels,
    client: Option<&GitLab>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let initial = initial_char(fallback_name);
    let cached = fetch_cached(avatar_url, client, cx);
    let theme = cx.theme();
    let dark = theme.is_dark();
    let border = theme.border.opacity(0.7);
    let (bg, fg) = entity_tint(fallback_name, dark);
    let radius = px((f32::from(size_px) * 0.22).clamp(4.0, 10.0));
    let font_sz = px((f32::from(size_px) * 0.46).clamp(11.0, 20.0));
    if let Some(image) = cached {
        div()
            .size(size_px)
            .rounded(radius)
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .flex_shrink_0()
            .child(img(image).size(size_px).rounded(radius).object_fit(ObjectFit::Cover))
    } else {
        div()
            .size(size_px)
            .rounded(radius)
            .border_1()
            .border_color(border)
            .flex_shrink_0()
            .bg(bg)
            .text_color(fg)
            .flex()
            .items_center()
            .justify_center()
            .text_size(font_sz)
            .font_semibold()
            .child(initial)
    }
}

pub fn avatar_size_sm() -> Pixels {
    px(22.)
}
