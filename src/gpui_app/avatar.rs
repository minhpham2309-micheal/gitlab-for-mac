use gpui_kit::{
    component::ActiveTheme as _, div, img, px, App, IntoElement, ObjectFit, ParentElement as _,
    Pixels, Styled as _, StyledImage as _,
};

use crate::gitlab::GitLab;

use super::image_cache::ImageCache;

pub fn render_avatar(
    avatar_url: Option<&str>,
    fallback_name: &str,
    size_px: Pixels,
    client: Option<&GitLab>,
    cx: &mut App,
) -> impl IntoElement {
    let initial = fallback_name
        .trim_start_matches('@')
        .chars()
        .next()
        .unwrap_or('G')
        .to_ascii_uppercase()
        .to_string();

    let cached = avatar_url
        .filter(|u| !u.trim().is_empty())
        .and_then(|url| {
            let cache = cx.try_global::<ImageCache>().cloned()?;
            let client = client?;
            cache.get_or_fetch(client, url, None, cx)
        });

    let primary = cx.theme().primary;
    if let Some(image) = cached {
        div()
            .size(size_px)
            .rounded_full()
            .overflow_hidden()
            .flex_shrink_0()
            .child(
                img(image)
                    .size(size_px)
                    .rounded_full()
                    .object_fit(ObjectFit::Cover),
            )
    } else {
        div()
            .size(size_px)
            .rounded_full()
            .flex_shrink_0()
            .bg(primary.opacity(0.16))
            .text_color(primary)
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .child(initial)
    }
}

pub fn avatar_size_sm() -> Pixels {
    px(22.)
}
