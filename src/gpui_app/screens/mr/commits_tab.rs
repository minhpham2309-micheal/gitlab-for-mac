use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::MergeRequest;

use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

pub fn render_mr_commits_tab(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let src_branch = mr.source_branch.clone();

    let rows: Vec<_> = view
        .commits
        .iter()
        .enumerate()
        .map(|(ix, c)| {
            let short = c.display_short_id().to_string();
            let full_sha = c.id.clone();
            let rel = c.timestamp().map(format_relative).unwrap_or_default();
            let web = c.web_url.clone();
            let st_copy = view.state.clone();
            let st_hist = view.state.clone();
            let branch = src_branch.clone();
            let pipe_badge = c.last_pipeline.as_ref().map(|p| {
                let col = status_color(&p.status, dark);
                Icon::empty().path(status_icon_path(&p.status)).size(px(20.)).text_color(col)
            });

            div()
                .id(SharedString::from(format!("mr-commit-row-{ix}")))
                .px_4().py_3().border_b_1().border_color(theme.border.opacity(0.6))
                .hover(|s| s.bg(theme.list_hover.opacity(0.4)))
                .flex().items_center().justify_between().gap_4()
                .child(
                    div().flex_1().min_w_0().flex().flex_col().gap_1()
                        .child(
                            div().id(SharedString::from(format!("mr-commit-open-{ix}")))
                                .cursor_pointer().text_sm().font_semibold().text_color(theme.foreground)
                                .hover(|s| s.text_color(theme.link)).truncate()
                                .on_click(move |_, _, cx| st_hist.update(cx, |s, cx| s.open_commit_history(Some(branch.clone()), None, cx)))
                                .child(c.title.clone()),
                        )
                        .child(
                            div().text_xs().text_color(theme.muted_foreground)
                                .child(if rel.is_empty() { c.author_name.clone() } else { format!("{} authored {rel}", c.author_name) }),
                        ),
                )
                .child(
                    div().flex().items_center().gap_2().flex_shrink_0()
                        .children(pipe_badge)
                        .child(
                            Button::new(SharedString::from(format!("mr-commit-sha-{ix}")))
                                .outline().xsmall().icon(Icon::new(IconName::Copy).size_3()).label(short)
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(full_sha.clone()));
                                    st_copy.update(cx, |s, cx| s.set_status("Copied commit SHA", cx));
                                }),
                        )
                        .children(web.map(|u| {
                            Button::new(SharedString::from(format!("mr-commit-web-{ix}")))
                                .ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3p5())
                                .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                        })),
                )
        })
        .collect();

    let body = if rows.is_empty() {
        div().py_12().flex().justify_center().text_sm().text_color(theme.muted_foreground)
            .child("No commits found for this merge request.").into_any_element()
    } else {
        div().rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
            .overflow_hidden().flex().flex_col().children(rows).into_any_element()
    };

    div()
        .id("mr-commits-scroll").flex_1().min_h_0().overflow_y_scroll().px_6().py_4()
        .child(div().w_full().max_w(px(1120.)).mx_auto().child(body))
        .test_support()
}
