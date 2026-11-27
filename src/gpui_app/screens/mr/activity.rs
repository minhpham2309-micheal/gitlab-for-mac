use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::Button,
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{
    parse_system_note, Discussion, DiscussionNote, GitLab, MergeRequest, MrActivityFilter, Project,
};

use super::super::super::markdown::MarkdownProjectContext;
use super::super::super::safe_url::safe_open_url;
use super::discussion_card::render_discussion_card;
use super::MergeRequestsView;

pub fn render_mr_activity(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    project: &Project,
    md_ctx: &MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let head_sha = mr.diff_refs.as_ref().and_then(|r| r.head_sha.as_deref());
    let filter = view.activity_filter;
    let desc = view.activity_desc;
    let web_origin = project.web_url.split("/-/").next().unwrap_or(&project.web_url).to_string();

    let mut ordered: Vec<&Discussion> = view.discussions.iter().collect();
    if desc {
        ordered.reverse();
    }

    let mut items: Vec<gpui_kit::AnyElement> = Vec::new();
    for disc in ordered {
        let is_system_only = !disc.notes.is_empty() && disc.notes.iter().all(|n| n.system);
        match filter {
            MrActivityFilter::CommentsOnly if is_system_only => continue,
            MrActivityFilter::HistoryOnly if !is_system_only => continue,
            _ => {}
        }
        if is_system_only {
            for note in &disc.notes {
                items.push(render_system_timeline_item(note, &web_origin, cx).into_any_element());
            }
        } else {
            let is_replying = view.replying_to.as_deref() == Some(disc.id.as_str());
            if let Some(card) = render_discussion_card(disc, head_sha, is_replying, &view.reply_input, md_ctx, client, cx) {
                items.push(card.into_any_element());
            }
        }
    }

    let weak_act = cx.entity().downgrade();
    let sort_icon = if desc { IconName::SortDescending } else { IconName::SortAscending };
    let theme = cx.theme();

    let header = div()
        .flex().items_center().justify_between().gap_2().pt_2()
        .child(div().text_lg().font_bold().text_color(theme.foreground).child("Activity"))
        .child(
            div().flex().items_center().gap_1p5()
                .child(
                    Button::new("mr-activity-filter").outline().xsmall()
                        .label(format!("{} ▾", filter.label()))
                        .dropdown_menu(move |mut menu, _, _| {
                            for f in MrActivityFilter::ALL {
                                let w = weak_act.clone();
                                menu = menu.item(PopupMenuItem::new(f.label()).checked(filter == f).on_click(move |_, _, cx| {
                                    let _ = w.update(cx, |v, cx| { v.activity_filter = f; cx.notify(); });
                                }));
                            }
                            menu
                        }),
                )
                .child(
                    Button::new("mr-activity-sort").outline().xsmall().icon(Icon::new(sort_icon).size_3p5())
                        .on_click(cx.listener(|this, _, _, cx| { this.activity_desc = !this.activity_desc; cx.notify(); })),
                ),
        );

    let err_banner = view.discussions_error.as_deref().map(|err| {
        div().p_2().rounded_md().border_1().border_color(theme.warning).bg(theme.warning.opacity(0.12))
            .text_xs().text_color(theme.foreground).child(format!("Discussions could not be loaded: {err}"))
    });

    let empty = items.is_empty().then(|| {
        div().py_4().text_xs().text_color(theme.muted_foreground).child("No activity yet.")
    });

    div()
        .id("mr-activity-section")
        .flex().flex_col().gap_3()
        .child(header)
        .children(err_banner)
        .children(items)
        .children(empty)
        .test_support()
}

fn render_system_timeline_item(
    note: &DiscussionNote,
    web_origin: &str,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let author = note.author.as_ref().map(|a| a.username.clone()).unwrap_or_else(|| "system".into());
    let rel = note.created_at.as_deref().map(format_relative).unwrap_or_default();
    let parsed = parse_system_note(&note.body);
    let note_id = note.id;

    let mut content = div()
        .flex_1().min_w_0().flex().flex_col().gap_1().pb_3()
        .child(
            div().flex().items_center().flex_wrap().gap_1().text_xs()
                .child(div().font_semibold().text_color(theme.foreground).child(author))
                .child(div().text_color(theme.muted_foreground).child(parsed.summary))
                .children((!rel.is_empty()).then(|| div().text_color(theme.muted_foreground).child(rel))),
        );

    if !parsed.commits.is_empty() {
        let mut list = div().pl_3().flex().flex_col().gap_0p5().text_xs();
        for (ix, c) in parsed.commits.into_iter().enumerate() {
            let sha_copy = c.sha.clone();
            list = list.child(
                div().flex().items_center().flex_wrap().gap_1()
                    .child(div().text_color(theme.muted_foreground).child("•"))
                    .child(
                        div()
                            .id(SharedString::from(format!("sys-sha-{note_id}-{ix}")))
                            .cursor_pointer()
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.link)
                            .hover(|s| s.text_color(theme.link_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let _ = &sha_copy;
                                this.active_tab = 1;
                                cx.notify();
                            }))
                            .child(c.sha),
                    )
                    .child(div().text_color(theme.muted_foreground).child(format!("- {}", c.message))),
            );
        }
        content = content.child(list);
    }

    if let Some(cmp_path) = parsed.compare_url {
        let full_url = if cmp_path.starts_with("http") {
            cmp_path
        } else {
            let host_root = web_origin.split('/').take(3).collect::<Vec<_>>().join("/");
            format!("{host_root}{cmp_path}")
        };
        content = content.child(
            div()
                .id(SharedString::from(format!("sys-cmp-{note_id}")))
                .pl_3().cursor_pointer().text_xs().text_color(theme.link)
                .hover(|s| s.text_color(theme.link_hover))
                .on_click(move |_, _, _| { let _ = safe_open_url(&full_url); })
                .child("Compare with previous version"),
        );
    }

    div()
        .id(SharedString::from(format!("mr-sys-note-{note_id}")))
        .flex().flex_row().items_stretch().gap_3().pl_2()
        .child(
            div().flex().flex_col().items_center().pt_1().flex_shrink_0()
                .child(div().size(px(6.)).rounded_full().bg(theme.muted_foreground))
                .child(div().flex_1().w(px(1.)).mt_1().bg(theme.border.opacity(0.6))),
        )
        .child(content)
}
