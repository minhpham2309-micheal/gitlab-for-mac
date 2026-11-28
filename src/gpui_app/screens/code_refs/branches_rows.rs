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
use crate::gitlab::{
    match_branch_mr, match_ref_pipeline, Branch, BranchScope, MergeRequest, Pipeline,
};
use crate::nav::Screen;

use super::super::super::pajamas_theme::{status_badge_palette, status_color, status_icon_path};
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppState, ScreenCommand};
use super::branches::BranchesView;

pub struct BranchSectionSpec {
    pub key: &'static str,
    pub title: &'static str,
    pub total: usize,
    pub limit: Option<usize>,
    pub show_more: Option<(&'static str, BranchScope)>,
}

pub fn render_branch_section(
    spec: BranchSectionSpec,
    branches: &[Branch],
    pipelines: &[Pipeline],
    mrs: &[MergeRequest],
    project_id: u64,
    state: &Entity<AppState>,
    cx: &mut Context<BranchesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let (border, head_bg, bg, hover_bg, fg, muted, link, sec_bg, mono) = (
        theme.border, theme.list_head, theme.background, theme.list_hover,
        theme.foreground, theme.muted_foreground, theme.link, theme.secondary,
        theme.mono_font_family.clone(),
    );
    let default_badge = status_badge_palette("running", dark);
    let merged_badge = status_badge_palette("merged", dark);
    let visible = spec.limit.map_or(branches, |n| &branches[..branches.len().min(n)]);

    let rows_el = if visible.is_empty() {
        div().px_4().py_6().text_xs().text_color(muted).child("No branches in this section.").into_any_element()
    } else {
        let mut row_els = Vec::with_capacity(visible.len());
        for (ix, br) in visible.iter().enumerate() {
            let name = br.name.clone();
            let (name_click, name_copy, name_browse, name_hist, name_sha) = (
                name.clone(), name.clone(), name.clone(), name.clone(), name.clone(),
            );
            let (st_name, st_copy, st_browse, st_hist, st_sha, st_pipe, st_mr) = (
                state.clone(), state.clone(), state.clone(), state.clone(), state.clone(), state.clone(), state.clone(),
            );
            let sha_full = br.commit.as_ref().map(|c| c.id.as_str());
            let pipe = match_ref_pipeline(&br.name, sha_full, pipelines);
            let mr = match_branch_mr(&br.name, mrs);
            let commit_line = br.commit.as_ref().map(|c| {
                let short = c.display_short_id().to_string();
                let rel = c.timestamp().map(format_relative).unwrap_or_default();
                let tail = if rel.is_empty() { c.title.clone() } else { format!("{} · {rel}", c.title) };
                (short, tail)
            });

            let mr_pill = mr.map(|m| {
                let pal = status_badge_palette(&m.state, dark);
                let iid = m.iid;
                div()
                    .id(SharedString::from(format!("br-mr-{}-{ix}", spec.key)))
                    .px_2().py(px(1.5)).rounded_full().bg(pal.bg).text_color(pal.text)
                    .cursor_pointer().flex().items_center().gap_1().text_xs().font_semibold()
                    .on_click(move |_, _, cx| {
                        st_mr.update(cx, |s, cx| {
                            s.set_screen(Screen::MergeRequests, cx);
                            s.dispatch_command(ScreenCommand::OpenMr { project_id, iid }, cx);
                        });
                    })
                    .child(Icon::empty().path("icons/glab-mr.svg").size_3().text_color(pal.icon))
                    .child(format!("!{iid}"))
                    .test_support()
            });

            let pipe_el = pipe.map(|p| {
                let (pid, sc, icon_p) = (p.id, status_color(&p.status, dark), status_icon_path(&p.status));
                div()
                    .id(SharedString::from(format!("br-pipe-{}-{ix}", spec.key)))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| st_pipe.update(cx, |s, cx| s.open_pipeline(pid, cx)))
                    .child(Icon::empty().path(icon_p).size(px(18.)).text_color(sc))
                    .test_support()
            });

            row_els.push(
                div()
                    .id(SharedString::from(format!("branch-row-{}-{ix}", spec.key)))
                    .w_full().px_4().py_3().bg(bg)
                    .border_b_1().border_color(border.opacity(0.6))
                    .hover(move |s| s.bg(hover_bg.opacity(0.45)))
                    .flex().items_center().justify_between().gap_3()
                    .child(
                        div().flex_1().min_w_0().flex().flex_col().gap_1()
                            .child(
                                div().flex().items_center().gap_1p5().min_w_0()
                                    .child(
                                        div().id(SharedString::from(format!("br-name-{}-{ix}", spec.key)))
                                            .cursor_pointer().text_sm().font_semibold().text_color(fg).truncate()
                                            .hover(move |s| s.text_color(link))
                                            .on_click(move |_, _, cx| st_name.update(cx, |s, cx| s.open_overview_ref(name_click.clone(), String::new(), cx)))
                                            .child(name).test_support(),
                                    )
                                    .child(Button::new(SharedString::from(format!("br-copy-{}-{ix}", spec.key))).ghost().xsmall().icon(Icon::new(IconName::Copy).size_3())
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(name_copy.clone()));
                                            st_copy.update(cx, |s, cx| s.set_status("Copied branch name", cx));
                                        }))
                                    .children(br.default.then(|| {
                                        div().px_1p5().py(px(1.)).rounded_full().bg(default_badge.bg).text_color(default_badge.text).text_xs().font_semibold().child("default")
                                    }))
                                    .children(br.protected.then(|| {
                                        div().px_1p5().py(px(1.)).rounded_full().bg(sec_bg).border_1().border_color(border).text_color(fg).text_xs().child("protected")
                                    }))
                                    .children((br.merged && !br.default).then(|| {
                                        div().px_1p5().py(px(1.)).rounded_full().bg(merged_badge.bg).text_color(merged_badge.text).text_xs().font_semibold().child("merged")
                                    })),
                            )
                            .children(commit_line.map(|(short, tail)| {
                                div().flex().items_center().gap_1p5().min_w_0().text_xs().text_color(muted)
                                    .child(
                                        div().id(SharedString::from(format!("br-sha-{}-{ix}", spec.key)))
                                            .cursor_pointer().flex_shrink_0().font_family(mono.clone()).text_color(link)
                                            .on_click(move |_, _, cx| st_sha.update(cx, |s, cx| s.open_commit_history(Some(name_sha.clone()), None, cx)))
                                            .child(short).test_support(),
                                    )
                                    .child("·")
                                    .child(div().flex_1().min_w_0().truncate().child(tail))
                            })),
                    )
                    .child(
                        div().flex().items_center().gap_1p5().flex_shrink_0()
                            .children(pipe_el)
                            .children(mr_pill)
                            .child(Button::new(SharedString::from(format!("br-browse-{}-{ix}", spec.key))).outline().xsmall().label("Browse files")
                                .on_click(move |_, _, cx| st_browse.update(cx, |s, cx| s.open_overview_ref(name_browse.clone(), String::new(), cx))))
                            .child(Button::new(SharedString::from(format!("br-hist-{}-{ix}", spec.key))).ghost().xsmall().label("Commits")
                                .on_click(move |_, _, cx| st_hist.update(cx, |s, cx| s.open_commit_history(Some(name_hist.clone()), None, cx))))
                            .children(br.web_url.clone().filter(|u| !u.is_empty()).map(|u| {
                                Button::new(SharedString::from(format!("br-web-{}-{ix}", spec.key))).ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3())
                                    .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                            })),
                    )
                    .test_support(),
            );
        }
        div().w_full().flex().flex_col().children(row_els).into_any_element()
    };

    let footer = spec.show_more.map(|(lbl, target_scope)| {
        div()
            .id(SharedString::from(format!("branch-more-{}", spec.key)))
            .w_full().px_4().py_2p5().bg(head_bg).cursor_pointer().text_xs().text_color(link)
            .hover(move |s| s.bg(hover_bg))
            .on_click(cx.listener(move |v, _, _, cx| v.set_scope(target_scope, cx)))
            .child(lbl)
            .test_support()
    });

    div()
        .id(SharedString::from(format!("branch-section-{}", spec.key)))
        .w_full().rounded_md().border_1().border_color(border).overflow_hidden().flex().flex_col()
        .child(
            div().w_full().px_4().py_2p5().bg(head_bg).border_b_1().border_color(border)
                .flex().items_center().justify_between()
                .child(div().text_sm().font_semibold().text_color(fg).child(spec.title))
                .child(div().text_xs().text_color(muted).child(format!("{}", spec.total))),
        )
        .child(rows_el)
        .children(footer)
        .test_support()
}
