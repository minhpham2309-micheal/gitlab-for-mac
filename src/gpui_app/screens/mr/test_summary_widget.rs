use gpui_kit::{
    base::StyledExt as _,
    component::{ActiveTheme as _, Icon, IconName},
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::MergeRequest;

use super::super::super::pajamas_theme::status_color;
use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

pub fn render_test_summary_widget(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    cx: &mut Context<MergeRequestsView>,
) -> Option<impl IntoElement + use<>> {
    let summary = view.test_summary.as_ref()?;
    if summary.total.count == 0 {
        return None;
    }
    let theme = cx.theme();
    let dark = theme.is_dark();
    let totals = &summary.total;
    let has_failures = totals.failed > 0 || totals.error > 0;
    let check_fg: gpui_kit::Hsla = if dark { gpui_kit::rgb(0x18171D).into() } else { gpui_kit::rgb(0xFFFFFF).into() };
    let status_icon = if has_failures {
        Icon::new(IconName::CircleX).size(px(20.)).text_color(status_color("failed", dark)).into_any_element()
    } else {
        div()
            .size(px(20.)).rounded_full().bg(status_color("success", dark)).flex_shrink_0()
            .flex().items_center().justify_center()
            .child(Icon::new(IconName::Check).size_3().text_color(check_fg))
            .into_any_element()
    };

    let headline = if has_failures {
        format!(
            "Test summary: {} failed, {} errored, {} total tests",
            totals.failed, totals.error, totals.count
        )
    } else if totals.skipped > 0 {
        format!(
            "Test summary: no failed tests, {} skipped, {} total tests",
            totals.skipped, totals.count
        )
    } else {
        format!("Test summary: no changed test results, {} total tests", totals.count)
    };

    let report_url = format!("{}/test_reports", mr.web_url.trim_end_matches('/'));
    let mut card = div()
        .id("mr-widget-test-summary")
        .rounded_md().border_1().border_color(theme.border).bg(theme.list_head)
        .flex().flex_col()
        .child(
            div().px_4().py_2p5().flex().items_center().justify_between().gap_3()
                .child(
                    div().flex().items_center().gap_3().min_w_0()
                        .child(status_icon)
                        .child(div().text_xs().font_semibold().text_color(theme.foreground).truncate().child(headline)),
                )
                .child(
                    div().flex().items_center().gap_2().flex_shrink_0()
                        .child(
                            div().id("mr-test-full-report").cursor_pointer().text_xs().text_color(theme.link)
                                .hover(|s| s.text_color(theme.link_hover))
                                .on_click(move |_, _, _| { let _ = safe_open_url(&report_url); })
                                .child("Full report").test_support(),
                        )
                        .child(
                            div().id("mr-test-summary-expand").pl_2().border_l_1().border_color(theme.border)
                                .cursor_pointer().text_color(theme.muted_foreground)
                                .hover(|s| s.text_color(theme.foreground))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.test_summary_expanded = !this.test_summary_expanded;
                                    this.overview_list.remeasure_items(1..2);
                                    cx.notify();
                                }))
                                .child(Icon::new(if view.test_summary_expanded { IconName::ChevronUp } else { IconName::ChevronDown }).size_3p5()),
                        ),
                ),
        );

    if view.test_summary_expanded && !summary.test_suites.is_empty() {
        let mut list = div()
            .pl(px(48.)).pr_4().py_2p5().border_t_1().border_color(theme.border)
            .flex().flex_col().gap_1().text_xs();
        for suite in &summary.test_suites {
            let suite_ok = suite.failed_count == 0 && suite.error_count == 0;
            let tone = status_color(if suite_ok { "success" } else { "failed" }, dark);
            list = list.child(
                div().flex().items_center().justify_between().gap_2()
                    .child(div().text_color(theme.foreground).truncate().child(suite.name.clone()))
                    .child(
                        div().flex_shrink_0().text_color(tone).child(format!(
                            "{}/{} passed ({:.1}s)",
                            suite.success_count, suite.total_count, suite.total_time
                        )),
                    ),
            );
        }
        card = card.child(list);
    }
    Some(card.test_support())
}
