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
use crate::gitlab::{GitLab, Release};

use super::super::super::avatar::render_avatar;
use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::AppState;
use super::releases::ReleasesView;

pub fn render_release_card(
    ix: usize,
    rel: &Release,
    assets_collapsed: bool,
    md_ctx: MarkdownProjectContext,
    client: Option<&GitLab>,
    state: &Entity<AppState>,
    cx: &mut Context<ReleasesView>,
) -> impl IntoElement + use<> {
    let title = rel.name.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| rel.tag_name.clone());
    let tag_name = rel.tag_name.clone();
    let tag_toggle = tag_name.clone();
    let tag_browse = tag_name.clone();
    let tag_hist = tag_name.clone();
    let rel_time = rel.released_at.as_deref().or(rel.created_at.as_deref()).map(format_relative).unwrap_or_default();
    let notes = rel.description.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "_No release notes._".into());
    let self_url = rel.links.as_ref().and_then(|l| l.self_url.clone());

    let author_el = rel.author.as_ref().map(|u| {
        let label = if u.name.is_empty() { u.username.clone() } else { u.name.clone() };
        let av = render_avatar(u.avatar_url.as_deref(), &label, px(20.), client, cx);
        (av, label)
    });
    let md_el = render_markdown_with_uploads(format!("rel-md-{ix}"), notes, md_ctx, cx);

    let (border, head_bg, bg, fg, muted, link, sec_bg, mono) = {
        let theme = cx.theme();
        (theme.border, theme.list_head, theme.background, theme.foreground, theme.muted_foreground, theme.link, theme.secondary, theme.mono_font_family.clone())
    };
    let (st_browse, st_hist) = (state.clone(), state.clone());

    let meta_col = div()
        .w(px(210.)).flex_shrink_0().flex().flex_col().gap_2().text_xs().text_color(muted)
        .children(author_el.map(|(av, lbl)| {
            div().flex().items_center().gap_2().child(av).child(div().text_color(fg).truncate().child(lbl))
        }))
        .children((!rel_time.is_empty()).then(|| {
            div().flex().items_center().gap_1p5()
                .child(Icon::new(IconName::Calendar).size_3p5().text_color(muted))
                .child(format!("Released {rel_time}"))
        }))
        .children(rel.commit.as_ref().map(|c| {
            let short = c.display_short_id().to_string();
            div().id(SharedString::from(format!("rel-commit-{ix}"))).cursor_pointer()
                .flex().items_center().gap_1p5()
                .on_click(move |_, _, cx| st_hist.update(cx, |s, cx| s.open_commit_history(Some(tag_hist.clone()), None, cx)))
                .child(div().size(px(8.)).rounded_full().border_1().border_color(muted))
                .child(div().font_family(mono.clone()).text_color(link).child(short))
                .test_support()
        }))
        .child(
            div().id(SharedString::from(format!("rel-tag-{ix}"))).cursor_pointer()
                .flex().items_center().gap_1p5()
                .on_click(move |_, _, cx| st_browse.update(cx, |s, cx| s.open_overview_ref(tag_browse.clone(), String::new(), cx)))
                .child(Icon::empty().path("icons/glab-tag.svg").size_3p5().text_color(muted))
                .child(div().font_family(mono.clone()).text_color(fg).truncate().child(tag_name))
                .test_support(),
        );

    let assets_section = rel.assets.as_ref().filter(|a| a.total_count() > 0).map(|assets| {
        let total = assets.total_count();
        let chevron = if assets_collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
        let grouped = assets.grouped_links();
        let body = (!assets_collapsed).then(|| {
            div().id(SharedString::from(format!("rel-assets-body-{ix}"))).pl_4().pt_1().flex().flex_col().gap_2()
                .children(assets.sources.iter().enumerate().map(|(si, src)| {
                    let u = src.url.clone();
                    div().id(SharedString::from(format!("rel-src-{ix}-{si}"))).cursor_pointer()
                        .flex().items_center().gap_1p5().text_xs().text_color(link)
                        .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                        .child(Icon::new(IconName::File).size_3().text_color(muted))
                        .child(format!("Source code ({})", src.format))
                        .child(Icon::new(IconName::ExternalLink).size_3().text_color(muted))
                }))
                .children(grouped.into_iter().map(|(grp_name, links)| {
                    div().flex().flex_col().gap_1().pt_1()
                        .child(div().text_xs().font_semibold().text_color(fg).child(grp_name))
                        .children(links.into_iter().enumerate().map(|(li, item)| {
                            let u = item.open_url().to_string();
                            div().id(SharedString::from(format!("rel-link-{ix}-{grp_name}-{li}"))).cursor_pointer()
                                .flex().items_center().gap_1p5().text_xs().text_color(link)
                                .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                                .child(Icon::new(IconName::ExternalLink).size_3().text_color(muted))
                                .child(item.name.clone())
                        }))
                }))
                .test_support()
        });
        div().pt_3().border_t_1().border_color(border.opacity(0.65)).flex().flex_col().gap_1()
            .child(
                div().id(SharedString::from(format!("rel-assets-toggle-{ix}"))).cursor_pointer()
                    .flex().items_center().gap_1p5()
                    .on_click(cx.listener(move |v, _, _, cx| v.toggle_assets(tag_toggle.clone(), cx)))
                    .child(Icon::new(chevron).size_3p5().text_color(muted))
                    .child(div().text_xs().font_semibold().text_color(fg).child("Assets"))
                    .child(div().px_1p5().py(px(1.)).rounded_full().bg(sec_bg).text_xs().text_color(muted).child(total.to_string()))
                    .test_support(),
            )
            .children(body)
    });

    let evidence_section = (!rel.evidences.is_empty()).then(|| {
        div().pt_3().border_t_1().border_color(border.opacity(0.65)).flex().flex_col().gap_1p5()
            .child(div().text_xs().font_semibold().text_color(fg).child("Evidence collection"))
            .children(rel.evidences.iter().enumerate().map(|(ei, ev)| {
                let name = ev.display_name();
                let short_sha = ev.short_sha().map(str::to_owned);
                let full_sha = ev.sha.clone();
                let url = ev.filepath.clone();
                let collected = ev.collected_at.as_deref().map(format_relative);
                let st_cp = state.clone();
                div().flex().flex_col().gap_0p5().text_xs()
                    .child(
                        div().flex().items_center().gap_2()
                            .child(div().id(SharedString::from(format!("rel-ev-{ix}-{ei}"))).cursor_pointer().text_color(link)
                                .on_click(move |_, _, _| { if let Some(u) = url.as_deref() { let _ = safe_open_url(u); } })
                                .child(name))
                            .children(short_sha.map(|s| div().font_family(mono.clone()).text_color(muted).child(s)))
                            .children(full_sha.map(|sha| {
                                Button::new(SharedString::from(format!("rel-ev-cp-{ix}-{ei}"))).ghost().xsmall().icon(Icon::new(IconName::Copy).size_3())
                                    .on_click(move |_, _, cx| { cx.write_to_clipboard(ClipboardItem::new_string(sha.clone())); st_cp.update(cx, |s, cx| s.set_status("Copied evidence SHA", cx)); })
                            })),
                    )
                    .children(collected.map(|c| div().text_color(muted).child(format!("Collected {c}"))))
            }))
    });

    div()
        .id(SharedString::from(format!("release-card-{ix}")))
        .w_full().rounded_md().border_1().border_color(border).overflow_hidden().flex().flex_col()
        .child(
            div().w_full().px_4().py_3().bg(head_bg).border_b_1().border_color(border)
                .flex().items_center().justify_between().gap_3()
                .child(div().text_lg().font_bold().text_color(fg).truncate().child(title))
                .children(self_url.map(|u| {
                    Button::new(SharedString::from(format!("rel-web-{ix}"))).outline().xsmall().label("Open in GitLab")
                        .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                })),
        )
        .child(
            div().w_full().p_4().bg(bg).flex().items_start().gap_6()
                .child(meta_col)
                .child(
                    div().flex_1().min_w_0().flex().flex_col().gap_3()
                        .child(div().text_sm().font_semibold().text_color(fg).child("Release notes"))
                        .child(md_el)
                        .children(assets_section)
                        .children(evidence_section),
                ),
        )
        .test_support()
}
