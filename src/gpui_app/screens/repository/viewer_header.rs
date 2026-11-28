use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::RepoBlobKind;

use super::super::super::avatar::render_avatar;
use super::super::super::diff_hl::language_from_path;
use super::super::super::safe_url::safe_open_url;
use super::RepositoryView;

pub fn format_bytes(n: usize) -> String {
    if n < 1024 { format!("{n} B") }
    else if n < 1024 * 1024 { format!("{:.1} KiB", n as f64 / 1024.0) }
    else { format!("{:.2} MiB", n as f64 / (1024.0 * 1024.0)) }
}

pub fn is_markdown_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

pub fn encode_web_blob_url(web_url: &str, git_ref: &str, path: &str) -> String {
    let encoded_path = path.trim_start_matches('/').split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>().join("/");
    format!("{}/-/blob/{}/{}", web_url.trim_end_matches('/'), urlencoding::encode(git_ref), encoded_path)
}

pub fn render_viewer_header(view: &RepositoryView, cx: &mut Context<RepositoryView>) -> impl IntoElement + use<> {
    let has_file = view.active_path.is_some();
    let path = view.active_path.clone().unwrap_or_default();
    let basename = path.rsplit('/').next().unwrap_or(&path).to_string();
    let is_md = has_file && is_markdown_path(&path);
    let is_text = matches!(view.active_blob.as_ref().map(|b| &b.kind), Some(RepoBlobKind::Text(_)));
    let lang = language_from_path(&path);
    let git_ref = view.active_ref.clone();
    let web_url = has_file
        .then(|| view.state.read(cx).project.as_ref().map(|p| encode_web_blob_url(&p.web_url, &git_ref, &path)))
        .flatten();

    let commit_banner = view.file_last_commit.as_ref().map(|c| {
        let av = render_avatar(None, &c.author_name, px(28.), None, cx);
        let rel = c.timestamp().map(format_relative).unwrap_or_default();
        let sub = if rel.is_empty() { c.author_name.clone() } else { format!("{} authored {rel}", c.author_name) };
        let short = c.display_short_id().to_string();
        let full_sha = c.id.clone();
        let (st_copy, st_hist, r_hist, p_hist) = (view.state.clone(), view.state.clone(), git_ref.clone(), path.clone());
        let theme = cx.theme();
        div()
            .id("repo-file-commit-banner")
            .mx_3().mt_2p5().px_3p5().py_2p5().rounded_md().bg(theme.list_head)
            .border_1().border_color(theme.border).flex().items_center().justify_between().gap_3()
            .child(
                div().flex_1().min_w_0().flex().items_center().gap_2p5()
                    .child(av)
                    .child(
                        div().flex_1().min_w_0().flex().flex_col().gap_0p5()
                            .child(div().text_xs().font_semibold().text_color(theme.foreground).truncate().child(c.title.clone()))
                            .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(sub)),
                    ),
            )
            .child(
                div().flex().items_center().gap_1p5().flex_shrink_0()
                    .child(
                        Button::new("repo-commit-copy-sha").outline().xsmall()
                            .icon(Icon::new(IconName::Copy).size_3()).label(short)
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone()));
                                st_copy.update(cx, |s, cx| s.set_status("Copied commit SHA", cx));
                            }),
                    )
                    .child(
                        Button::new("repo-commit-history").outline().xsmall().label("History")
                            .on_click(move |_, _, cx| {
                                st_hist.update(cx, |s, cx| s.open_commit_history(Some(r_hist.clone()), Some(p_hist.clone()), cx));
                            }),
                    ),
            )
            .test_support()
    });

    let theme = cx.theme();
    let mut crumb = div().flex().items_center().gap_1().min_w_0().overflow_hidden().child(
        Button::new("repo-toggle-tree").ghost().xsmall()
            .icon(Icon::new(if view.show_tree { IconName::PanelLeftClose } else { IconName::PanelLeftOpen }).size_3p5())
            .on_click(cx.listener(|this, _, _, cx| { this.show_tree = !this.show_tree; cx.notify(); })),
    );
    if has_file {
        let segs: Vec<&str> = path.split('/').collect();
        let last = segs.len().saturating_sub(1);
        let mut acc = String::new();
        for (ix, seg) in segs.into_iter().enumerate() {
            if ix > 0 { crumb = crumb.child(div().text_xs().text_color(theme.muted_foreground).child("/")); }
            if !acc.is_empty() { acc.push('/'); }
            acc.push_str(seg);
            if ix == last {
                crumb = crumb.child(div().text_xs().font_semibold().text_color(theme.foreground).truncate().child(seg.to_string()));
            } else {
                let (target_dir, target_ref, st) = (acc.clone(), git_ref.clone(), view.state.clone());
                crumb = crumb.child(
                    Button::new(SharedString::from(format!("repo-crumb-{ix}"))).ghost().xsmall().label(seg.to_string())
                        .on_click(move |_, _, cx| st.update(cx, |s, cx| s.open_overview_ref(target_ref.clone(), target_dir.clone(), cx))),
                );
            }
        }
    }

    let meta_text = view.active_blob.as_ref().map(|b| {
        let lines = match &b.kind { RepoBlobKind::Text(t) => format!("{} lines · ", t.lines().count()), _ => String::new() };
        format!("{lines}{} · {lang}", format_bytes(b.size_bytes))
    });

    let (copy_path, copy_state, copy_body_state) = (path.clone(), view.state.clone(), view.state.clone());
    let copy_body = view.active_blob.as_ref().and_then(|b| match &b.kind { RepoBlobKind::Text(t) => Some(t.clone()), _ => None });
    let (hist_state, hist_ref, hist_path) = (view.state.clone(), git_ref, path);

    let top_crumb_bar = div()
        .w_full().px_3().py_2().border_b_1().border_color(theme.border)
        .flex().items_center().justify_between().gap_2()
        .child(crumb)
        .child(
            div().flex().items_center().gap_1().flex_shrink_0()
                .children(has_file.then(|| {
                    Button::new("repo-copy-path").ghost().xsmall().label("Copy path").on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
                        copy_state.update(cx, |s, cx| s.set_status("Copied file path", cx));
                    })
                }))
                .children(has_file.then(|| {
                    Button::new("repo-file-history").outline().xsmall().label("History").on_click(move |_, _, cx| {
                        hist_state.update(cx, |s, cx| s.open_commit_history(Some(hist_ref.clone()), Some(hist_path.clone()), cx));
                    })
                }))
                .children(web_url.map(|url| {
                    Button::new("repo-open-web").ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3p5())
                        .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
                })),
        );

    let file_card_head = has_file.then(|| {
        div()
            .id("repo-blob-card-header")
            .mx_3().mt_2p5().px_3p5().py_2().rounded_t_md().bg(theme.list_head)
            .border_1().border_color(theme.border).flex().items_center().justify_between().gap_2()
            .child(
                div().flex().items_center().gap_2().min_w_0()
                    .child(Icon::new(IconName::File).size_3p5().text_color(theme.muted_foreground))
                    .child(div().text_xs().font_semibold().text_color(theme.foreground).truncate().child(basename))
                    .children(meta_text.map(|m| div().text_xs().text_color(theme.muted_foreground).child(m))),
            )
            .child(
                div().flex().items_center().gap_1().flex_shrink_0()
                    .children(is_md.then(|| {
                        Button::new("repo-md-toggle").outline().xsmall()
                            .label(if view.markdown_preview { "Source" } else { "Preview" })
                            .on_click(cx.listener(|this, _, _, cx| { this.markdown_preview = !this.markdown_preview; cx.notify(); }))
                    }))
                    .children(is_text.then(|| {
                        Button::new("repo-wrap-toggle").ghost().xsmall()
                            .label(if view.soft_wrap { "No wrap" } else { "Wrap" })
                            .on_click(cx.listener(|this, _, window, cx| this.toggle_wrap(window, cx)))
                    }))
                    .children(is_text.then(|| {
                        Button::new("repo-find-in-file").ghost().xsmall().icon(Icon::new(IconName::Search).size_3p5())
                            .on_click(cx.listener(|this, _, _, cx| this.open_in_file_search(cx)))
                    }))
                    .children(copy_body.map(|body| {
                        Button::new("repo-copy-file").ghost().xsmall().icon(Icon::new(IconName::Copy).size_3p5())
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(body.clone()));
                                copy_body_state.update(cx, |s, cx| s.set_status("Copied file contents", cx));
                            })
                    })),
            )
            .test_support()
    });

    div().w_full().flex().flex_col().child(top_crumb_bar).children(commit_banner).children(file_card_head)
}
