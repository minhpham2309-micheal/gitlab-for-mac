use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{match_ref_pipeline, Pipeline, RepoTag};
use crate::nav::Screen;

use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::AppState;
use super::tags::TagsView;

fn clip_annotation(msg: &str) -> String {
    let lines: Vec<&str> = msg.trim().lines().take(3).collect();
    let mut joined = lines.join("\n");
    if msg.trim().lines().count() > 3 {
        joined.push_str(" …");
    }
    joined
}

pub fn render_tag_row(
    ix: usize,
    tag: &RepoTag,
    pipelines: &[Pipeline],
    project_web: Option<&str>,
    state: &Entity<AppState>,
    cx: &Context<TagsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let name = tag.name.clone();
    let (name_click, name_copy, name_browse, name_hist, name_sha) = (
        name.clone(), name.clone(), name.clone(), name.clone(), name.clone(),
    );
    let (st_name, st_copy, st_browse, st_hist, st_sha, st_rel, st_pipe) = (
        state.clone(), state.clone(), state.clone(), state.clone(), state.clone(), state.clone(), state.clone(),
    );
    let sha_full = tag.commit.as_ref().map(|c| c.id.as_str()).filter(|s| !s.is_empty()).or(tag.target.as_deref());
    let pipe = match_ref_pipeline(&tag.name, sha_full, pipelines);
    let commit_line = tag.commit.as_ref().map(|c| {
        let short = c.display_short_id().to_string();
        let rel = c.timestamp().map(format_relative).unwrap_or_default();
        let tail = if rel.is_empty() { c.title.clone() } else { format!("{} · {rel}", c.title) };
        (short, tail)
    });
    let release_label = tag.release.as_ref().map(|r| {
        if r.tag_name.is_empty() { tag.name.clone() } else { r.tag_name.clone() }
    });
    let message_box = tag.message.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(clip_annotation);
    let tag_web_url = project_web.map(|u| format!("{}/-/tags/{}", u.trim_end_matches('/'), urlencoding::encode(&tag.name)));

    let pipe_el = pipe.map(|p| {
        let (pid, sc, icon_p) = (p.id, status_color(&p.status, dark), status_icon_path(&p.status));
        div()
            .id(SharedString::from(format!("tag-pipe-{ix}")))
            .cursor_pointer()
            .on_click(move |_, _, cx| st_pipe.update(cx, |s, cx| s.open_pipeline(pid, cx)))
            .child(Icon::empty().path(icon_p).size(px(18.)).text_color(sc))
            .test_support()
    });

    div()
        .id(SharedString::from(format!("tag-row-{ix}")))
        .w_full().px_3().py_3p5().border_b_1().border_color(theme.border.opacity(0.6))
        .hover(|s| s.bg(theme.list_hover.opacity(0.45)))
        .flex().items_start().justify_between().gap_4()
        .child(
            div().flex_1().min_w_0().flex().flex_col().gap_1()
                .child(
                    div().flex().items_center().gap_1p5().min_w_0()
                        .child(Icon::empty().path("icons/glab-tag.svg").size_3p5().text_color(theme.muted_foreground))
                        .child(
                            div().id(SharedString::from(format!("tag-name-{ix}")))
                                .cursor_pointer().text_sm().font_semibold().text_color(theme.foreground).truncate()
                                .hover(|s| s.text_color(theme.link))
                                .on_click(move |_, _, cx| st_name.update(cx, |s, cx| s.open_overview_ref(name_click.clone(), String::new(), cx)))
                                .child(name).test_support(),
                        )
                        .child(Button::new(SharedString::from(format!("tag-copy-{ix}"))).ghost().xsmall().icon(Icon::new(IconName::Copy).size_3())
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(name_copy.clone()));
                                st_copy.update(cx, |s, cx| s.set_status("Copied tag name", cx));
                            }))
                        .children(tag.protected.then(|| {
                            div().px_1p5().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border)
                                .text_xs().text_color(theme.foreground).child("protected")
                        })),
                )
                .children(commit_line.map(|(short, tail)| {
                    div().flex().items_center().gap_1p5().min_w_0().text_xs().text_color(theme.muted_foreground)
                        .child(
                            div().id(SharedString::from(format!("tag-sha-{ix}")))
                                .cursor_pointer().flex_shrink_0().font_family(theme.mono_font_family.clone()).text_color(theme.link)
                                .on_click(move |_, _, cx| st_sha.update(cx, |s, cx| s.open_commit_history(Some(name_sha.clone()), None, cx)))
                                .child(short).test_support(),
                        )
                        .child("·")
                        .child(div().flex_1().min_w_0().truncate().child(tail))
                }))
                .children(release_label.map(|rel_name| {
                    div().flex().items_center().gap_1p5().text_xs()
                        .child(Icon::empty().path("icons/glab-rocket.svg").size_3().text_color(theme.muted_foreground))
                        .child(div().text_color(theme.muted_foreground).child("Release:"))
                        .child(
                            div().id(SharedString::from(format!("tag-release-{ix}")))
                                .cursor_pointer().text_color(theme.link).hover(|s| s.text_color(theme.link_hover))
                                .on_click(move |_, _, cx| st_rel.update(cx, |s, cx| s.set_screen(Screen::Releases, cx)))
                                .child(rel_name).test_support(),
                        )
                }))
                .children(message_box.map(|msg| {
                    div().id(SharedString::from(format!("tag-msg-{ix}")))
                        .mt_1().px_3().py_2().rounded_md().bg(theme.list_head.opacity(0.65))
                        .border_1().border_color(theme.border).font_family(theme.mono_font_family.clone())
                        .text_xs().text_color(theme.foreground).child(msg).test_support()
                })),
        )
        .child(
            div().flex().items_center().gap_1p5().flex_shrink_0()
                .children(pipe_el)
                .child(Button::new(SharedString::from(format!("tag-browse-{ix}"))).outline().xsmall().label("Browse files")
                    .on_click(move |_, _, cx| st_browse.update(cx, |s, cx| s.open_overview_ref(name_browse.clone(), String::new(), cx))))
                .child(Button::new(SharedString::from(format!("tag-hist-{ix}"))).ghost().xsmall().label("Commits")
                    .on_click(move |_, _, cx| st_hist.update(cx, |s, cx| s.open_commit_history(Some(name_hist.clone()), None, cx))))
                .children(tag_web_url.map(|u| {
                    Button::new(SharedString::from(format!("tag-web-{ix}"))).ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3())
                        .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                })),
        )
        .test_support()
}
