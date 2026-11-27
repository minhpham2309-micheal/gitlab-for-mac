use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::MergeRequest;

use super::super::super::pajamas_theme::{status_color, status_icon_path};
use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

pub fn render_mr_pipelines_tab(
    view: &MergeRequestsView,
    _mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();

    let rows: Vec<_> = view
        .mr_pipelines
        .iter()
        .enumerate()
        .map(|(ix, p)| {
            let color = status_color(&p.status, dark);
            let short_sha = p.sha.get(..8).unwrap_or(&p.sha).to_string();
            let rel = p.updated_at.as_deref().or(p.created_at.as_deref()).map(format_relative).unwrap_or_default();
            let pid = p.id;
            let st_open = view.state.clone();
            let web = p.web_url.clone();

            div()
                .id(SharedString::from(format!("mr-pipe-row-{ix}")))
                .px_4().py_3().border_b_1().border_color(theme.border.opacity(0.6))
                .cursor_pointer().hover(|s| s.bg(theme.list_hover.opacity(0.4)))
                .flex().items_center().justify_between().gap_4()
                .on_click(move |_, _, cx| st_open.update(cx, |s, cx| s.open_pipeline(pid, cx)))
                .child(
                    div().flex().items_center().gap_3().min_w_0()
                        .child(
                            div().px_2().py_0p5().rounded_full().bg(color.opacity(0.16)).border_1().border_color(color.opacity(0.5))
                                .flex().items_center().gap_1().text_xs().text_color(color).flex_shrink_0()
                                .child(Icon::empty().path(status_icon_path(&p.status)).size_3().text_color(color))
                                .child(p.status.clone()),
                        )
                        .child(
                            div().flex().flex_col().gap_0p5().min_w_0()
                                .child(
                                    div().flex().items_center().gap_2().text_xs()
                                        .child(div().font_semibold().text_color(theme.link).child(format!("#{pid}")))
                                        .child(div().px_1p5().rounded_sm().bg(theme.secondary).font_family(theme.mono_font_family.clone()).text_color(theme.foreground).truncate().child(p.git_ref.clone()))
                                        .child(div().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(short_sha)),
                                )
                                .children(p.source.clone().map(|src| {
                                    div().text_xs().text_color(theme.muted_foreground).child(format!("Source: {src}"))
                                })),
                        ),
                )
                .child(
                    div().flex().items_center().gap_2().flex_shrink_0()
                        .children((!rel.is_empty()).then(|| div().text_xs().text_color(theme.muted_foreground).child(rel)))
                        .child(
                            Button::new(SharedString::from(format!("mr-pipe-web-{ix}")))
                                .ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3p5())
                                .on_click(move |_, _, _| { let _ = safe_open_url(&web); }),
                        ),
                )
        })
        .collect();

    let body = if rows.is_empty() {
        div().py_12().flex().justify_center().text_sm().text_color(theme.muted_foreground)
            .child("No pipelines found for this merge request.").into_any_element()
    } else {
        div().rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
            .overflow_hidden().flex().flex_col().children(rows).into_any_element()
    };

    div()
        .id("mr-pipelines-scroll").flex_1().min_h_0().overflow_y_scroll().px_6().py_4()
        .child(div().w_full().max_w(px(1120.)).mx_auto().child(body))
        .test_support()
}
