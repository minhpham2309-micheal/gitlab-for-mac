use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Selectable as _, Sizable as _,
    },
    div, px, uniform_list, ClipboardItem, Context, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::{mr_can_review, GitLab, MergeRequest, MrFilter};

use super::super::avatar::{avatar_size_sm, render_avatar};
use super::super::pajamas_theme::status_color;
use super::super::safe_url::safe_open_url;
use super::merge_requests::MergeRequestsView;

pub fn render_mr_list_panel(
    filter: MrFilter,
    mrs: Vec<MergeRequest>,
    selected_iid: Option<u64>,
    client: Option<GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let view_weak = cx.entity().downgrade();

    let filter_bar = div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_2()
        .border_b_1()
        .border_color(theme.border)
        .children(MrFilter::ALL.into_iter().map(|f| {
            Button::new(SharedString::from(format!("mr-f-{}", f.api_state())))
                .ghost()
                .xsmall()
                .selected(filter == f)
                .label(f.label())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter = f;
                    this.load_list(cx);
                }))
        }));

    let list = uniform_list("mr-uniform-list", mrs.len(), move |range, _, cx| {
        range
            .map(|ix| {
                let mr = mrs[ix].clone();
                let active = selected_iid == Some(mr.iid);
                let weak = view_weak.clone();
                let clicked = mr.clone();
                let author = mr.author.as_ref().map(|a| a.username.as_str()).unwrap_or("");
                let av = render_avatar(
                    mr.author.as_ref().and_then(|a| a.avatar_url.as_deref()),
                    author,
                    avatar_size_sm(),
                    client.as_ref(),
                    cx,
                );
                let theme = cx.theme();
                div()
                    .id(SharedString::from(format!("mr-row-{}", mr.iid)))
                    .h(px(64.))
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .cursor_pointer()
                    .bg(if active { theme.list_active } else { theme.colors.list })
                    .hover(|s| s.bg(theme.list_hover))
                    .flex()
                    .items_center()
                    .gap_2()
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |v, cx| v.select_mr(clicked.clone(), cx));
                    })
                    .child(av)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_sm().text_color(theme.foreground).truncate().child(format!("!{} {}", mr.iid, mr.title)))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(format!("{author} · {} → {} · 💬 {}", mr.source_branch, mr.target_branch, mr.user_notes_count)),
                            ),
                    )
            })
            .collect()
    })
    .flex_1()
    .size_full();

    div()
        .size_full()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(theme.border)
        .child(filter_bar)
        .child(list)
}

pub fn render_mr_header(mr: &MergeRequest, cx: &mut Context<MergeRequestsView>) -> impl IntoElement {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let can_review = mr_can_review(&mr.state);
    let branch_copy = mr.source_branch.clone();
    let web_url = mr.web_url.clone();

    div()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().text_base().text_color(theme.foreground).truncate().child(format!("!{} {}", mr.iid, mr.title)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .children(can_review.then(|| {
                            Button::new("mr-approve")
                                .outline()
                                .xsmall()
                                .label("Approve")
                                .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("approve", Some(w), cx)))
                        }))
                        .children(can_review.then(|| {
                            Button::new("mr-merge")
                                .primary()
                                .xsmall()
                                .label("Merge")
                                .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("merge", Some(w), cx)))
                        }))
                        .children(can_review.then(|| {
                            Button::new("mr-close")
                                .danger()
                                .xsmall()
                                .label("Close")
                                .on_click(cx.listener(|this, _, w, cx| this.perform_mr_mutation("close", Some(w), cx)))
                        }))
                        .child(
                            Button::new("mr-open-web")
                                .ghost()
                                .xsmall()
                                .label("Open web")
                                .on_click(move |_, _, _| {
                                    let _ = safe_open_url(&web_url);
                                }),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .px_2()
                        .py(px(1.))
                        .rounded_full()
                        .bg(status_color(&mr.state, dark).opacity(0.2))
                        .text_color(status_color(&mr.state, dark))
                        .child(mr.state.clone()),
                )
                .child(
                    Button::new("mr-copy-branch")
                        .ghost()
                        .xsmall()
                        .label(format!("{} → {}", mr.source_branch, mr.target_branch))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(branch_copy.clone()));
                        }),
                ),
        )
}
