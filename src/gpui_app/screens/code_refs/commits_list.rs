use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{group_commits_by_date, RepoCommit};

use super::super::super::avatar::render_avatar;
use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::commits::CommitsView;

fn filtered_commits(view: &CommitsView, cx: &Context<CommitsView>) -> Vec<RepoCommit> {
    let q = view.filter_input.read(cx).value().trim().to_ascii_lowercase();
    view.commits.iter().filter(|c| {
        q.is_empty() || c.title.to_ascii_lowercase().contains(&q)
            || c.author_name.to_ascii_lowercase().contains(&q) || c.id.starts_with(&q)
    }).cloned().collect()
}

fn commit_pipeline(view: &CommitsView, c: &RepoCommit) -> Option<(u64, String)> {
    c.last_pipeline.as_ref().map(|p| (p.id, p.status.clone()))
        .or_else(|| view.sha_pipelines.get(&c.id).cloned())
}

fn render_ref_picker(view: &CommitsView, id: &'static str, cx: &mut Context<CommitsView>) -> impl IntoElement + use<> {
    let (git_ref, branches, weak) = (view.git_ref.clone(), view.branches.clone(), cx.entity().downgrade());
    let btn = Button::new(id).outline().small()
        .icon(Icon::empty().path("icons/glab-branch.svg").size_3p5().text_color(cx.theme().muted_foreground))
        .label(format!("{git_ref} ▾"));
    if branches.is_empty() { return btn.into_any_element(); }
    btn.dropdown_menu(move |mut menu, _, _| {
        for b in &branches {
            let (name, w) = (b.name.clone(), weak.clone());
            menu = menu.item(PopupMenuItem::new(name.clone()).checked(name == git_ref).on_click(move |_, _, cx| {
                let _ = w.update(cx, |v, cx| v.switch_ref(name.clone(), cx));
            }));
        }
        menu
    }).into_any_element()
}

fn render_path_chip(view: &CommitsView, cx: &mut Context<CommitsView>) -> Option<impl IntoElement + use<>> {
    view.path_filter.clone().map(|p| {
        Button::new("commits-clear-path").outline().xsmall().label(format!("{p} ×"))
            .on_click(cx.listener(|v, _, _, cx| { (v.path_filter, v.selected_sha) = (None, None); v.clear_diff(cx); v.load_page(1, false, cx); }))
    })
}

pub fn render_commits_timeline_view(view: &CommitsView, cx: &mut Context<CommitsView>) -> impl IntoElement + use<> {
    let groups = group_commits_by_date(&filtered_commits(view, cx));
    let proj_name = view.state.read(cx).project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let (next_page, ref_picker, path_chip) = (view.page + 1, render_ref_picker(view, "commits-ref-picker", cx), render_path_chip(view, cx));
    let theme = cx.theme();
    let (dark, border, muted, fg, link, head_bg, bg, input_b, mono) = (
        theme.is_dark(), theme.border, theme.muted_foreground, theme.foreground,
        theme.link, theme.list_head, theme.background, theme.input, theme.mono_font_family.clone(),
    );

    let mut group_els = Vec::with_capacity(groups.len());
    for (g_ix, grp) in groups.into_iter().enumerate() {
        let mut cards = Vec::with_capacity(grp.items.len());
        for (ix, c) in grp.items {
            let av = render_avatar(None, &c.author_name, px(30.), None, cx);
            let rel = c.timestamp().map(format_relative).unwrap_or_default();
            let author_sub = if rel.is_empty() { c.author_name.clone() } else { format!("{} authored {rel}", c.author_name) };
            let (full_sha, click_sha, browse_sha) = (c.id.clone(), c.id.clone(), c.id.clone());
            let (st_copy, st_browse, st_pipe) = (view.state.clone(), view.state.clone(), view.state.clone());
            let tags = view.sha_tags.get(&c.id).cloned().unwrap_or_default();
            let pipe = commit_pipeline(view, &c);

            cards.push(
                div().id(SharedString::from(format!("commit-row-{ix}")))
                    .w_full().px_3p5().py_2p5().rounded_md().bg(head_bg).border_1().border_color(border.opacity(0.85))
                    .cursor_pointer().hover(move |s| s.border_color(input_b))
                    .flex().items_center().justify_between().gap_3()
                    .on_click(cx.listener(move |v, _, _, cx| v.select_commit(click_sha.clone(), cx)))
                    .child(div().flex_1().min_w_0().flex().items_center().gap_3().child(av)
                        .child(div().flex_1().min_w_0().flex().flex_col().gap_0p5()
                            .child(div().id(SharedString::from(format!("commit-title-{ix}"))).w_full().text_sm().font_semibold().text_color(fg).truncate().child(c.title.clone()).test_support())
                            .child(div().id(SharedString::from(format!("commit-meta-{ix}"))).w_full().text_xs().text_color(muted)
                                .child(div().id(SharedString::from(format!("commit-author-{ix}"))).truncate().child(author_sub).test_support()).test_support())))
                    .child(div().flex().items_center().gap_2().flex_shrink_0()
                        .children(tags.into_iter().take(2).map(|t| {
                            div().px_2().py(px(1.)).rounded_full().bg(bg).border_1().border_color(border).flex().items_center().gap_1().text_xs().text_color(fg)
                                .child(Icon::empty().path("icons/glab-tag.svg").size_3().text_color(muted)).child(t)
                        }))
                        .children(pipe.map(|(pid, st)| {
                            div().id(SharedString::from(format!("commit-pipe-{ix}"))).cursor_pointer()
                                .on_click(move |_, _, cx| { cx.stop_propagation(); st_pipe.update(cx, |s, cx| s.open_pipeline(pid, cx)); })
                                .child(Icon::empty().path(status_icon_path(&st)).size(px(18.)).text_color(status_color(&st, dark))).test_support()
                        }))
                        .child(div().id(SharedString::from(format!("commit-sha-{ix}"))).font_family(mono.clone()).text_xs().text_color(link).child(c.display_short_id().to_string()).test_support())
                        .child(Button::new(SharedString::from(format!("commit-copy-{ix}"))).ghost().xsmall().icon(Icon::new(IconName::Copy).size_3p5())
                            .on_click(move |_, _, cx| { cx.stop_propagation(); cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone())); st_copy.update(cx, |s, cx| s.set_status("Copied commit SHA", cx)); }))
                        .child(Button::new(SharedString::from(format!("commit-tree-{ix}"))).ghost().xsmall().icon(Icon::new(IconName::Folder).size_3p5())
                            .on_click(move |_, _, cx| { cx.stop_propagation(); st_browse.update(cx, |s, cx| s.open_overview_ref(browse_sha.clone(), String::new(), cx)); })))
                    .test_support(),
            );
        }
        group_els.push(
            div().id(SharedString::from(format!("commit-date-group-{g_ix}"))).w_full().flex().flex_col().gap_2()
                .child(div().flex().items_center().gap_2().pt_1()
                    .child(div().size(px(8.)).rounded_full().border_1().border_color(muted))
                    .child(div().text_sm().font_semibold().text_color(fg).child(grp.label)))
                .child(div().ml(px(3.5)).pl_4().pb_3().border_l_1().border_color(border.opacity(0.65)).flex().flex_col().gap_2().children(cards))
                .test_support(),
        );
    }

    let body = if group_els.is_empty() {
        div().py_12().flex().items_center().justify_center().text_sm().text_color(muted)
            .child(if view.loading { "Loading commits…" } else { "No commits found" }).into_any_element()
    } else {
        div().w_full().flex().flex_col().children(group_els)
            .child(div().py_3().flex().justify_center().child(
                Button::new("commits-load-more").outline().small().label("Load more")
                    .on_click(cx.listener(move |v, _, _, cx| v.load_page(next_page, true, cx)))))
            .into_any_element()
    };

    div().id("commits-timeline-view").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
        .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
            .child(div().flex().items_center().gap_2p5().pb_2().child(ref_picker)
                .child(div().text_sm().font_semibold().text_color(fg).truncate().child(proj_name)).children(path_chip))
            .child(div().flex().items_center().justify_between().gap_3().pb_2p5()
                .child(div().text_2xl().font_bold().text_color(fg).child("Commits"))
                .child(div().flex().items_center().gap_1p5()
                    .child(Button::new("commits-split-toggle").outline().small().label("Split diff").on_click(cx.listener(|v, _, _, cx| v.toggle_split_mode(cx))))
                    .child(Button::new("commits-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5()).on_click(cx.listener(|v, _, _, cx| v.load_page(1, false, cx))))))
            .child(div().p_2().mb_3().rounded_md().bg(head_bg).border_1().border_color(border)
                .child(div().h(px(32.)).rounded_md().bg(bg).border_1().border_color(input_b).px_2().flex().items_center()
                    .child(div().flex_1().min_w_0().child(Input::new(&view.filter_input).appearance(false).cleanable(true).small()))))
            .child(div().id("commits-timeline-scroll").flex_1().min_h_0().overflow_y_scroll().pr_1().child(body)))
        .test_support()
}

pub fn render_commits_split_list(view: &CommitsView, cx: &mut Context<CommitsView>) -> impl IntoElement + use<> {
    let groups = group_commits_by_date(&filtered_commits(view, cx));
    let (sel, next_page) = (view.selected_sha.clone(), view.page + 1);
    let (ref_picker, path_chip) = (render_ref_picker(view, "commits-split-ref", cx), render_path_chip(view, cx));
    let theme = cx.theme();
    let (dark, border, primary, muted, fg, head_bg, active_bg, hover_bg, mono) = (
        theme.is_dark(), theme.border, theme.primary, theme.muted_foreground, theme.foreground,
        theme.list_head, theme.list_active, theme.list_hover, theme.mono_font_family.clone(),
    );

    let mut items_els = Vec::new();
    for (g_ix, grp) in groups.into_iter().enumerate() {
        items_els.push(
            div().id(SharedString::from(format!("commit-split-date-{g_ix}")))
                .w_full().px_3().pt_2p5().pb_1().flex().items_center().gap_1p5()
                .child(div().size(px(7.)).rounded_full().border_1().border_color(muted))
                .child(div().text_xs().font_semibold().text_color(muted).child(grp.label))
                .into_any_element(),
        );
        for (ix, c) in grp.items {
            let (is_sel, sha_click) = (sel.as_deref() == Some(c.id.as_str()), c.id.clone());
            let av = render_avatar(None, &c.author_name, px(24.), None, cx);
            let rel = c.timestamp().map(format_relative).unwrap_or_default();
            let meta_left = if rel.is_empty() { c.author_name.clone() } else { format!("{} · {rel}", c.author_name) };
            let pipe = commit_pipeline(view, &c);

            items_els.push(
                div().px_2().pb_1().w_full().child(
                    div().id(SharedString::from(format!("commit-row-{ix}")))
                        .w_full().px_2p5().py_2().rounded_md().cursor_pointer()
                        .border_1().border_color(if is_sel { primary.opacity(0.6) } else { border.opacity(0.6) })
                        .bg(if is_sel { active_bg } else { head_bg }).hover(move |s| s.bg(hover_bg))
                        .flex().items_center().gap_2()
                        .on_click(cx.listener(move |v, _, _, cx| v.select_commit(sha_click.clone(), cx)))
                        .child(av)
                        .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                            .child(div().id(SharedString::from(format!("commit-title-{ix}"))).w_full().flex_shrink_0()
                                .text_sm().font_medium().text_color(fg).truncate().child(c.title.clone()).test_support())
                            .child(div().id(SharedString::from(format!("commit-meta-{ix}"))).w_full().flex_shrink_0()
                                .flex().items_center().justify_between().gap_2().text_xs().text_color(muted)
                                .child(div().id(SharedString::from(format!("commit-author-{ix}"))).flex_1().min_w_0().truncate().child(meta_left).test_support())
                                .child(div().flex().items_center().gap_1().flex_shrink_0()
                                    .children(pipe.map(|(_, st)| Icon::empty().path(status_icon_path(&st)).size_3().text_color(status_color(&st, dark))))
                                    .child(div().id(SharedString::from(format!("commit-sha-{ix}"))).font_family(mono.clone()).child(c.display_short_id().to_string()).test_support()))
                                .test_support()))
                        .test_support(),
                ).into_any_element(),
            );
        }
    }

    let list_body = if items_els.is_empty() {
        div().flex_1().flex().items_center().justify_center().text_xs().text_color(muted)
            .child(if view.loading { "Loading commits…" } else { "No commits found" }).into_any_element()
    } else {
        div().id("commits-split-scroll").flex_1().min_h_0().w_full().overflow_y_scroll().flex().flex_col().children(items_els).into_any_element()
    };

    div().size_full().flex().flex_col().border_r_1().border_color(border)
        .child(div().p_2p5().border_b_1().border_color(border).flex().flex_col().gap_1p5()
            .child(div().w_full().flex().items_center().justify_between().gap_1()
                .child(div().flex().items_center().gap_1().min_w_0().child(ref_picker).children(path_chip))
                .child(Button::new("commits-back-timeline").ghost().xsmall().label("Timeline").on_click(cx.listener(|v, _, _, cx| v.close_diff_pane(cx)))))
            .child(Input::new(&view.filter_input).xsmall()))
        .child(div().flex_1().min_h_0().w_full().overflow_hidden().flex().flex_col().child(list_body))
        .child(div().p_2().border_t_1().border_color(border).flex().justify_center().child(
            Button::new("commits-load-more").ghost().xsmall().label("Load more").on_click(cx.listener(move |v, _, _, cx| v.load_page(next_page, true, cx)))))
}
