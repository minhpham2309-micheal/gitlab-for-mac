use gpui_kit::{
    component::ActiveTheme as _, div, px, Context, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::{BridgeJob, Job};

use super::pipeline_graph_model::build_stage_columns;
use super::pipeline_job_card::render_job_card;
use super::pipelines::PipelinesView;

pub fn render_stage_graph(
    jobs: &[Job],
    bridges: &[BridgeJob],
    selected_job_id: Option<u64>,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let border = theme.border;
    let sidebar_bg = theme.sidebar;
    let fg = theme.foreground;
    let muted = theme.muted_foreground;
    let columns = build_stage_columns(jobs, bridges);

    if columns.is_empty() {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(muted)
            .child("Select a pipeline to inspect its stage graph")
            .into_any_element();
    }

    let col_count = columns.len();
    let mut row = div()
        .id("pipeline-stage-graph-scroll")
        .size_full()
        .overflow_x_scroll()
        .overflow_y_scroll()
        .p_3()
        .flex()
        .flex_row()
        .items_start()
        .gap_2();

    for (idx, col) in columns.into_iter().enumerate() {
        let mut col_el = div()
            .w(px(210.))
            .flex_shrink_0()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(sidebar_bg)
            .p_2()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(
                div()
                    .px_1()
                    .pb_1()
                    .border_b_1()
                    .border_color(border.opacity(0.6))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(fg).child(col.name))
                    .child(div().text_xs().text_color(muted).child(format!("{}", col.cards.len()))),
            );

        for card in col.cards {
            col_el = col_el.child(render_job_card(card, selected_job_id, dark, cx));
        }

        row = row.child(col_el);
        if idx + 1 < col_count {
            row = row.child(
                div()
                    .w(px(18.))
                    .pt(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .child(div().h(px(1.5)).w_full().bg(border)),
            );
        }
    }

    row.into_any_element()
}
