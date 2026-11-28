use gpui_kit::{
    component::{
        button::Button,
        input::Editor,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, img, px, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::RepoBlobKind;

use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::super::super::safe_url::safe_open_url;
use super::viewer_header::{encode_web_blob_url, format_bytes, is_markdown_path};
use super::RepositoryView;

pub fn render_code_body(
    view: &RepositoryView,
    md_ctx: MarkdownProjectContext,
    cx: &mut Context<RepositoryView>,
) -> impl IntoElement {
    let theme = cx.theme();
    if view.blob_loading {
        return div()
            .size_full().flex().items_center().justify_center()
            .text_xs().text_color(theme.muted_foreground)
            .child("Loading file…")
            .into_any_element();
    }

    let Some(path) = view.active_path.as_deref() else {
        return div()
            .size_full().flex().items_center().justify_center()
            .text_sm().text_color(theme.muted_foreground)
            .child("Select a file from the tree or press ⌘T to find a file")
            .into_any_element();
    };

    let Some(blob) = view.active_blob.as_ref() else {
        return div()
            .size_full().flex().items_center().justify_center()
            .text_xs().text_color(theme.muted_foreground)
            .child(view.blob_error.clone().unwrap_or_else(|| "Unable to load file".into()))
            .into_any_element();
    };

    let raw_web = view
        .state
        .read(cx)
        .project
        .as_ref()
        .map(|p| encode_web_blob_url(&p.web_url, &view.active_ref, path));

    match &blob.kind {
        RepoBlobKind::Text(text) if is_markdown_path(path) && view.markdown_preview => {
            let md = render_markdown_with_uploads(format!("repo-md-{path}"), text.clone(), md_ctx, cx);
            div().id("repo-md-preview-scroll").size_full().overflow_y_scroll().p_6().child(md).into_any_element()
        }
        RepoBlobKind::Text(_) => Editor::new(&view.editor_state).readonly(true).bordered(false).size_full().into_any_element(),
        RepoBlobKind::Image { .. } => {
            if let Some(decoded) = view.decoded_image.clone() {
                div()
                    .id("repo-img-scroll")
                    .size_full().overflow_y_scroll().p_6()
                    .flex().flex_col().items_center().justify_center().gap_3()
                    .child(
                        div().p_4().rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
                            .child(img(decoded).max_w(px(760.)).max_h(px(560.))),
                    )
                    .child(
                        div().text_xs().text_color(theme.muted_foreground)
                            .child(format!("{path} · {}", format_bytes(blob.size_bytes))),
                    )
                    .into_any_element()
            } else {
                guard_card("Unsupported image format", blob.size_bytes, raw_web, cx)
            }
        }
        RepoBlobKind::Binary => {
            guard_card("Binary file cannot be rendered inline", blob.size_bytes, raw_web, cx)
        }
        RepoBlobKind::TooLarge { limit_bytes } => {
            let msg = format!(
                "File ({}) exceeds inline viewer limit ({})",
                format_bytes(blob.size_bytes),
                format_bytes(*limit_bytes)
            );
            guard_card(&msg, blob.size_bytes, raw_web, cx)
        }
    }
}

fn guard_card(
    reason: &str,
    size_bytes: usize,
    web_url: Option<String>,
    cx: &Context<RepositoryView>,
) -> gpui_kit::AnyElement {
    let theme = cx.theme();
    div()
        .size_full().flex().flex_col().items_center().justify_center().gap_3()
        .child(Icon::new(IconName::File).size_6().text_color(theme.muted_foreground))
        .child(div().text_sm().text_color(theme.foreground).child(reason.to_string()))
        .child(div().text_xs().text_color(theme.muted_foreground).child(format_bytes(size_bytes)))
        .children(web_url.map(|url| {
            Button::new("repo-guard-open-web").outline().small()
                .icon(Icon::new(IconName::ExternalLink).size_3p5())
                .label("Open in GitLab Web")
                .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
        }))
        .into_any_element()
}
