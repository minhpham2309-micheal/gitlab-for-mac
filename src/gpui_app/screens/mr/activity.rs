use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::Button,
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{DiscussionNote, GitLab, MergeRequest, MrActivityFilter, ParsedSystemNote};

use super::super::super::markdown::MarkdownProjectContext;
use super::super::super::safe_url::safe_open_url;
use super::cache::ActivityEntry;
use super::discussion_card::render_discussion_card;
use super::MergeRequestsView;

pub fn render_activity_header(
    view: &MergeRequestsView,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let filter = view.activity_filter;
    let desc = view.activity_desc;
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
                                    let _ = w.update(cx, |v, cx| {
                                        v.activity_filter = f;
                                        v.rebuild_activity_cache();
                                        cx.notify();
                                    });
                                }));
                            }
                            menu
                        }),
                )
                .child(
                    Button::new("mr-activity-sort").outline().xsmall().icon(Icon::new(sort_icon).size_3p5())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.activity_desc = !this.activity_desc;
                            this.rebuild_activity_cache();
                            cx.notify();
                        })),
                ),
        );

    let err_banner = view.discussions_error.as_deref().map(|err| {
        div().p_2().rounded_md().border_1().border_color(theme.warning).bg(theme.warning.opacity(0.12))
            .text_xs().text_color(theme.foreground).child(format!("Discussions could not be loaded: {err}"))
    });
    let empty = (view.cached_activity.is_empty() && view.discussions_error.is_none()).then(|| {
        div().py_3().text_xs().text_color(theme.muted_foreground).child("No activity yet.")
    });

    div()
        .id("mr-activity-section")
        .pb_2().flex().flex_col().gap_2()
        .child(header)
        .children(err_banner)
        .children(empty)
        .test_support()
}

pub fn render_activity_entry(
    view: &MergeRequestsView,
    entry: &ActivityEntry,
    mr: &MergeRequest,
    web_origin: &str,
    md_ctx: &MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> AnyElement {
    match entry {
        ActivityEntry::System { discussion_ix, note_ix, parsed } => {
            let Some(note) = view.discussions.get(*discussion_ix).and_then(|d| d.notes.get(*note_ix)) else {
                return div().into_any_element();
            };
            render_system_timeline_item(note, parsed, web_origin, cx).into_any_element()
        }
        ActivityEntry::Thread { discussion_ix } => {
            let Some(disc) = view.discussions.get(*discussion_ix) else {
                return div().into_any_element();
            };
            let head_sha = mr.diff_refs.as_ref().and_then(|r| r.head_sha.as_deref());
            let is_replying = view.replying_to.as_deref() == Some(disc.id.as_str());
            render_discussion_card(disc, head_sha, is_replying, &view.reply_input, md_ctx, client, cx)
                .map(|c| div().pb_3().child(c).into_any_element())
                .unwrap_or_else(|| div().into_any_element())
        }
    }
}

fn render_system_timeline_item(
    note: &DiscussionNote,
    parsed: &ParsedSystemNote,
    web_origin: &str,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let author = note.author.as_ref().map(|a| a.username.clone()).unwrap_or_else(|| "system".into());
    let rel = note.created_at.as_deref().map(format_relative).unwrap_or_default();
    let note_id = note.id;

    let mut content = div()
        .flex_1().min_w_0().flex().flex_col().gap_1().pb_3()
        .child(
            div().flex().items_center().flex_wrap().gap_1().text_xs()
                .child(div().font_semibold().text_color(theme.foreground).child(author))
                .child(div().text_color(theme.muted_foreground).child(parsed.summary.clone()))
                .children((!rel.is_empty()).then(|| div().text_color(theme.muted_foreground).child(rel))),
        );

    if !parsed.commits.is_empty() {
        let mut list_el = div().pl_3().flex().flex_col().gap_0p5().text_xs();
        for (ix, c) in parsed.commits.iter().enumerate() {
            list_el = list_el.child(
                div().flex().items_center().flex_wrap().gap_1()
                    .child(div().text_color(theme.muted_foreground).child("•"))
                    .child(
                        div()
                            .id(SharedString::from(format!("sys-sha-{note_id}-{ix}")))
                            .cursor_pointer()
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.link)
                            .hover(|s| s.text_color(theme.link_hover))
                            .on_click(cx.listener(|this, _, _, cx| { this.active_tab = 1; cx.notify(); }))
                            .child(c.sha.clone()),
                    )
                    .child(div().text_color(theme.muted_foreground).child(format!("- {}", c.message))),
            );
        }
        content = content.child(list_el);
    }

    if let Some(cmp_path) = &parsed.compare_url {
        let full_url = if cmp_path.starts_with("http") {
            cmp_path.clone()
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
