use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{BridgeJob, Job, Pipeline};
use crate::prefs::{
    DEFAULT_PIPELINE_GRAPH_ZOOM, MAX_PIPELINE_GRAPH_ZOOM, MIN_PIPELINE_GRAPH_ZOOM,
    PIPELINE_GRAPH_ZOOM_STEP,
};

use super::pipeline_graph_model::build_stage_columns;
use super::pipeline_job_card::render_job_card;
use super::pipelines::PipelinesView;

impl PipelinesView {
    pub fn graph_zoom_pct(&self) -> u16 {
        self.graph_zoom_pct
    }

    pub fn step_graph_zoom(&mut self, delta: i16, cx: &mut Context<Self>) {
        let next = (self.graph_zoom_pct as i16 + delta)
            .clamp(MIN_PIPELINE_GRAPH_ZOOM as i16, MAX_PIPELINE_GRAPH_ZOOM as i16)
            as u16;
        if next != self.graph_zoom_pct {
            self.graph_zoom_pct = next;
            self.state.update(cx, |s, _| {
                s.prefs.pipeline_graph_zoom = Some(next);
                s.save_prefs();
            });
            cx.notify();
        }
    }

    pub fn reset_graph_zoom(&mut self, cx: &mut Context<Self>) {
        if self.graph_zoom_pct != DEFAULT_PIPELINE_GRAPH_ZOOM {
            self.graph_zoom_pct = DEFAULT_PIPELINE_GRAPH_ZOOM;
            self.state.update(cx, |s, _| {
                s.prefs.pipeline_graph_zoom = Some(DEFAULT_PIPELINE_GRAPH_ZOOM);
                s.save_prefs();
            });
            cx.notify();
        }
    }

    pub fn set_fixture_stage_graph(
        &mut self,
        pipelines: Vec<Pipeline>,
        jobs: Vec<Job>,
        bridges: Vec<BridgeJob>,
        cx: &mut Context<Self>,
    ) {
        self.selected_pipeline = pipelines.first().cloned();
        self.pipelines = pipelines;
        self.jobs = jobs;
        self.bridges = bridges;
        self.sync_summary(cx);
        cx.notify();
    }
}

pub fn render_stage_graph(
    jobs: &[Job],
    bridges: &[BridgeJob],
    selected_job_id: Option<u64>,
    zoom_pct: u16,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let border = theme.border;
    let sidebar_bg = theme.sidebar;
    let popover_bg = theme.popover;
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

    let scale = (zoom_pct.clamp(MIN_PIPELINE_GRAPH_ZOOM, MAX_PIPELINE_GRAPH_ZOOM) as f32) / 100.0;
    let col_w = px((210.0 * scale).round());
    let col_pad = px((8.0 * scale).round().max(4.0));
    let col_gap = px((6.0 * scale).round().max(3.0));
    let row_gap = px((8.0 * scale).round().max(4.0));
    let row_pad = px((12.0 * scale).round().max(6.0));
    let hdr_font = px((12.0 * scale).round().clamp(9.0, 22.0));
    let conn_w = px((18.0 * scale).round().max(8.0));
    let conn_pt = px((28.0 * scale).round().max(14.0));
    let conn_h = px((1.5 * scale).max(1.0));

    let col_count = columns.len();
    let mut row = div()
        .id("pipeline-stage-graph-scroll")
        .size_full()
        .overflow_x_scroll()
        .overflow_y_scroll()
        .pt(row_pad)
        .pl(row_pad)
        .pr(px(48.))
        .pb(px(48.))
        .flex()
        .flex_row()
        .items_start()
        .gap(row_gap);

    for (idx, col) in columns.into_iter().enumerate() {
        let mut col_el = div()
            .id(SharedString::from(format!("stage-col-{idx}")))
            .w(col_w)
            .flex_shrink_0()
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(sidebar_bg)
            .p(col_pad)
            .flex()
            .flex_col()
            .gap(col_gap)
            .child(
                div()
                    .px_1()
                    .pb_1()
                    .border_b_1()
                    .border_color(border.opacity(0.6))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(hdr_font)
                    .child(div().text_color(fg).truncate().child(col.name))
                    .child(div().text_color(muted).child(format!("{}", col.cards.len()))),
            )
            .test_support();

        for card in col.cards {
            col_el = col_el.child(render_job_card(card, selected_job_id, dark, scale, cx));
        }

        row = row.child(col_el);
        if idx + 1 < col_count {
            row = row.child(
                div()
                    .w(conn_w)
                    .pt(conn_pt)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .child(div().h(conn_h).w_full().bg(border)),
            );
        }
    }

    let zoom_bar = div()
        .id("pipeline-graph-zoom-bar")
        .absolute()
        .right_3()
        .bottom_3()
        .px_1()
        .py_0p5()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(popover_bg.opacity(0.95))
        .shadow_md()
        .flex()
        .items_center()
        .gap_0p5()
        .child(
            Button::new("pipe-zoom-out")
                .ghost()
                .xsmall()
                .disabled(zoom_pct <= MIN_PIPELINE_GRAPH_ZOOM)
                .icon(Icon::new(IconName::Minus).size_3p5())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.step_graph_zoom(-PIPELINE_GRAPH_ZOOM_STEP, cx);
                })),
        )
        .child(
            Button::new("pipe-zoom-reset")
                .ghost()
                .xsmall()
                .label(format!("{zoom_pct}%"))
                .on_click(cx.listener(|this, _, _, cx| this.reset_graph_zoom(cx))),
        )
        .child(
            Button::new("pipe-zoom-in")
                .ghost()
                .xsmall()
                .disabled(zoom_pct >= MAX_PIPELINE_GRAPH_ZOOM)
                .icon(Icon::new(IconName::Plus).size_3p5())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.step_graph_zoom(PIPELINE_GRAPH_ZOOM_STEP, cx);
                })),
        )
        .test_support();

    div()
        .relative()
        .size_full()
        .child(row)
        .child(zoom_bar)
        .into_any_element()
}
