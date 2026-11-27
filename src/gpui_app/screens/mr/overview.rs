use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, rgb, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{GitLab, MergeRequest, Project};

use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::activity::render_mr_activity;
use super::sidebar::render_mr_metadata_sidebar;
use super::widgets::render_mr_widgets;
use super::MergeRequestsView;

pub fn render_mr_overview(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    project: &Project,
    md_ctx: MarkdownProjectContext,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let desc = mr
        .description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("_No description provided._")
        .to_string();
    let desc_block = render_markdown_with_uploads(format!("mr-desc-{}", mr.iid), desc, md_ctx.clone(), cx);

    let me_id = view.state.read(cx).current_user.as_ref().map(|u| u.id);
    let award_count = |name: &str, fallback: u32| -> (usize, bool) {
        let count = view.award_emojis.iter().filter(|a| a.name == name).count();
        let mine = me_id.is_some_and(|uid| {
            view.award_emojis.iter().any(|a| a.name == name && a.user.as_ref().is_some_and(|u| u.id == uid))
        });
        let shown = if view.award_emojis.is_empty() { fallback as usize } else { count };
        (shown, mine)
    };
    let (up_n, up_mine) = award_count("thumbsup", mr.upvotes);
    let (down_n, down_mine) = award_count("thumbsdown", mr.downvotes);

    let mut extra_emojis: Vec<(String, usize)> = Vec::new();
    for a in &view.award_emojis {
        if a.name != "thumbsup" && a.name != "thumbsdown" {
            if let Some(slot) = extra_emojis.iter_mut().find(|(n, _)| n == &a.name) {
                slot.1 += 1;
            } else {
                extra_emojis.push((a.name.clone(), 1));
            }
        }
    }

    let widgets = render_mr_widgets(view, mr, cx);
    let activity = render_mr_activity(view, mr, project, &md_ctx, client, cx);
    let sidebar = render_mr_metadata_sidebar(view, mr, client, cx);
    let weak_emoji = cx.entity().downgrade();
    let theme = cx.theme();

    let thumb_color = rgb(if theme.is_dark() { 0xE9BE74 } else { 0xAB6100 });
    let reaction_btn = |id: &'static str, icon: IconName, count: usize, active: bool| {
        div()
            .id(id)
            .px_2p5()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .bg(if active { theme.list_active } else { theme.secondary })
            .border_1()
            .border_color(if active { theme.primary } else { theme.border })
            .hover(|s| s.bg(theme.secondary_hover))
            .flex()
            .items_center()
            .gap_1p5()
            .text_xs()
            .text_color(theme.foreground)
            .child(Icon::new(icon).size_3p5().text_color(thumb_color))
            .child(count.to_string())
    };

    let reactions_bar = div()
        .id("mr-reactions-bar")
        .flex()
        .items_center()
        .flex_wrap()
        .gap_2()
        .child(
            reaction_btn("mr-react-up", IconName::ThumbsUp, up_n, up_mine)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_award_emoji("thumbsup", cx)))
                .test_support(),
        )
        .child(
            reaction_btn("mr-react-down", IconName::ThumbsDown, down_n, down_mine)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_award_emoji("thumbsdown", cx)))
                .test_support(),
        )
        .children(extra_emojis.into_iter().map(|(name, cnt)| {
            let glyph = match name.as_str() {
                "rocket" => "🚀",
                "tada" => "🎉",
                "heart" => "❤️",
                "eyes" => "👀",
                _ => "✨",
            };
            div()
                .px_2p5()
                .py_1()
                .rounded_md()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .text_color(theme.foreground)
                .child(format!("{glyph} {cnt}"))
        }))
        .child(
            Button::new("mr-react-add")
                .outline()
                .xsmall()
                .icon(Icon::new(IconName::Plus).size_3())
                .dropdown_menu(move |menu, _, _| {
                    let w1 = weak_emoji.clone();
                    let w2 = weak_emoji.clone();
                    let w3 = weak_emoji.clone();
                    let w4 = weak_emoji.clone();
                    menu.item(PopupMenuItem::new("🚀 Rocket").on_click(move |_, _, cx| {
                        let _ = w1.update(cx, |v, cx| v.toggle_award_emoji("rocket", cx));
                    }))
                    .item(PopupMenuItem::new("🎉 Tada").on_click(move |_, _, cx| {
                        let _ = w2.update(cx, |v, cx| v.toggle_award_emoji("tada", cx));
                    }))
                    .item(PopupMenuItem::new("❤️ Heart").on_click(move |_, _, cx| {
                        let _ = w3.update(cx, |v, cx| v.toggle_award_emoji("heart", cx));
                    }))
                    .item(PopupMenuItem::new("👀 Eyes").on_click(move |_, _, cx| {
                        let _ = w4.update(cx, |v, cx| v.toggle_award_emoji("eyes", cx));
                    }))
                }),
        );

    let composer = div()
        .mt_2()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.list_head)
        .flex()
        .flex_col()
        .gap_2()
        .child(Input::new(&view.comment_input))
        .child(
            div().flex().items_center().justify_end().child(
                Button::new("mr-new-comment")
                    .primary()
                    .small()
                    .label("Comment")
                    .on_click(cx.listener(|this, _, window, cx| this.submit_mr_comment(window, cx))),
            ),
        );

    let main_col = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_4()
        .child(div().id("mr-desc-body").child(desc_block).test_support())
        .child(reactions_bar)
        .child(widgets)
        .child(activity)
        .child(composer);

    div()
        .id("mr-overview-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px_6()
        .py_4()
        .child(
            div()
                .w_full()
                .max_w(px(1120.))
                .mx_auto()
                .flex()
                .flex_row()
                .items_start()
                .gap_6()
                .child(main_col)
                .child(sidebar),
        )
}
