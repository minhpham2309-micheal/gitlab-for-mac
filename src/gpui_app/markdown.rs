use std::net::IpAddr;
use std::sync::Arc;

use gpui_kit::{
    component::{button::Button, text::TextView, ActiveTheme as _, Sizable as _, WindowExt as _},
    div, App, ElementId, Image, ImageFormat, ImageSource, IntoElement, ParentElement as _,
    SharedString, SharedUri, Styled as _,
};
use url::{Host, Url};

use crate::gitlab::media::{is_blocked_local_domain, is_private_or_local_ip};
use crate::gitlab::GitLab;

use super::image_cache::{CacheEntry, ImageCache};
use super::safe_url::safe_open_url;

const MAX_DATA_URI_BYTES: usize = 256 * 1024;

#[derive(Clone, Default)]
pub struct MarkdownProjectContext {
    pub project_id: Option<u64>,
    pub project_web_url: Option<String>,
    pub gitlab: Option<GitLab>,
    pub cache: Option<ImageCache>,
}

impl MarkdownProjectContext {
    pub fn from_state(st: &super::state::AppState, cx: &App) -> Self {
        Self {
            project_id: st.project.as_ref().map(|p| p.id),
            project_web_url: st.project.as_ref().map(|p| p.web_url.clone()),
            gitlab: st.gitlab.clone(),
            cache: cx.try_global::<ImageCache>().cloned(),
        }
    }
}

pub fn is_safe_data_image_uri(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.strip_prefix("data:").is_some_and(|rest| {
        trimmed.len() <= MAX_DATA_URI_BYTES
            && ["image/png;", "image/jpeg;", "image/gif;", "image/webp;"]
                .iter()
                .any(|p| rest.starts_with(p))
    })
}

pub fn is_safe_markdown_image_uri(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() { return false; }
    if is_safe_data_image_uri(trimmed) { return true; }
    let Ok(url) = Url::parse(trimmed) else { return false; };
    if !matches!(url.scheme(), "https" | "http") { return false; }
    match url.host() {
        Some(Host::Domain(domain)) => !is_blocked_local_domain(domain),
        Some(Host::Ipv4(ip)) => !is_private_or_local_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => !is_private_or_local_ip(IpAddr::V6(ip)),
        None => false,
    }
}

pub fn extract_image_sources(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = markdown;
    while let Some(idx) = rest.find("![") {
        let Some(mid) = rest[idx + 2..].find("](") else { break; };
        let url_slice = &rest[idx + 2 + mid + 2..];
        let end = url_slice
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '>'))
            .unwrap_or(url_slice.len());
        let candidate = url_slice[..end].trim();
        if !candidate.is_empty() && !out.iter().any(|s| s == candidate) {
            out.push(candidate.to_string());
        }
        let Some(first_ch) = url_slice.chars().next() else { break; };
        rest = &url_slice[end.max(first_ch.len_utf8())..];
    }
    out
}

pub fn prefetch_markdown_uploads(source: &str, ctx: &MarkdownProjectContext, cx: &mut App) -> Vec<(String, String)> {
    let mut failed_links = Vec::new();
    let (Some(client), Some(cache)) = (&ctx.gitlab, &ctx.cache) else { return failed_links; };
    let origin = client.origin().ascii_serialization();
    for raw_src in extract_image_sources(source) {
        if raw_src.starts_with("/uploads/") {
            if let Some(CacheEntry::Failed(_)) = cache.lookup(&origin, ctx.project_id, &raw_src) {
                if let Some(base) = ctx.project_web_url.as_deref() {
                    failed_links.push((raw_src.clone(), format!("{}{}", base.trim_end_matches('/'), raw_src)));
                }
            } else {
                let _ = cache.get_or_fetch(client, &raw_src, ctx.project_id, cx);
            }
        } else if is_safe_markdown_image_uri(&raw_src) && !is_safe_data_image_uri(&raw_src) {
            let _ = cache.get_or_fetch(client, &raw_src, ctx.project_id, cx);
        }
    }
    failed_links
}

fn empty_image_source() -> ImageSource {
    ImageSource::Image(Arc::new(Image::from_bytes(ImageFormat::Png, Vec::new())))
}

pub fn safe_markdown(
    id: impl Into<ElementId>,
    source: impl Into<SharedString>,
    ctx: MarkdownProjectContext,
) -> TextView {
    let origin = ctx.gitlab.as_ref().map(|g| g.origin().ascii_serialization());
    let project_id = ctx.project_id;
    let cache = ctx.cache.clone();

    TextView::markdown(id, source)
        .selectable(true)
        .on_link_click(|href, _, window, cx| {
            if let Err(err) = safe_open_url(href.as_ref()) {
                window.push_notification(err, cx);
            }
        })
        .image_source(move |uri: &SharedUri| {
            let raw = uri.as_ref();
            if let (Some(cache), Some(origin)) = (&cache, &origin) {
                if let Some(CacheEntry::Ready(img)) = cache.lookup(origin, project_id, raw) {
                    return ImageSource::Image(img);
                }
            }
            // Never pass remote HTTP/HTTPS URLs to GPUI's unhardened HttpClient;
            // only bounded inline data:image/* URIs render without ImageCache.
            if is_safe_data_image_uri(raw) {
                ImageSource::from(uri.clone())
            } else {
                empty_image_source()
            }
        })
}

pub fn render_markdown_with_uploads(
    id_prefix: impl Into<String>,
    source: String,
    ctx: MarkdownProjectContext,
    cx: &mut App,
) -> impl IntoElement {
    let prefix = id_prefix.into();
    let failed = prefetch_markdown_uploads(&source, &ctx, cx);
    let theme = cx.theme();
    let fb = (!failed.is_empty()).then(|| {
        div().flex().flex_wrap().gap_1().pt_1()
            .children(failed.into_iter().enumerate().map(|(ix, (path, web))| {
                let label = path.rsplit('/').next().unwrap_or(&path).to_string();
                Button::new(SharedString::from(format!("{prefix}-upfb-{ix}")))
                    .outline().xsmall().label(format!("📎 {label}"))
                    .on_click(move |_, _, _| { let _ = safe_open_url(&web); })
            }))
            .text_color(theme.muted_foreground)
    });
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(safe_markdown(SharedString::from(prefix), source, ctx))
        .children(fb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_ssrf_ipv4_mapped_localhost_and_oversized_data_images() {
        assert!(is_safe_markdown_image_uri("https://cdn.example.com/a.png"));
        for bad in [
            "http://127.0.0.1/secret.png",
            "http://169.254.169.254/latest/meta-data",
            "http://192.168.1.10/cam.jpg",
            "http://[::ffff:127.0.0.1]/a.png",
            "http://sub.localhost/a.png",
            "http://localhost./a.png",
            "file:///etc/passwd",
            "/uploads/abc/img.png",
        ] {
            assert!(!is_safe_markdown_image_uri(bad), "should reject {bad}");
        }
        let huge = format!("data:image/png;base64,{}", "A".repeat(MAX_DATA_URI_BYTES + 10));
        assert!(!is_safe_markdown_image_uri(&huge));
        assert!(is_safe_data_image_uri("data:image/png;base64,iVBORw0KGgo="));
        assert_eq!(
            extract_image_sources("![s](/uploads/abc123/shot.png) and ![b](https://cdn.example.com/b.jpg)"),
            vec!["/uploads/abc123/shot.png", "https://cdn.example.com/b.jpg"]
        );
        assert!(extract_image_sources("![unclosed](").is_empty());
        assert!(extract_image_sources("![wide](\u{3000}https://cdn.example.com/a.png)").is_empty());
    }
}
