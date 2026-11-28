use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::Input,
        spinner::Spinner,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, uniform_list, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{
    filter_pipelines, format_pipeline_ref, latest_pipeline_ids_by_ref, pipeline_is_live,
    pipeline_kind_tag, pipeline_status_label, PipelineScope,
};

use super::super::pajamas_theme::{status_badge_palette, status_color, status_icon_path};
use super::super::safe_url::safe_open_url;
use super::pipelines::PipelinesView;

pub fn render_pipelines_table_view(
    view: &PipelinesView,
    can_run_ci: bool,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let query = view.pipe_filter_input.read(cx).value().to_string();
    let filtered = filter_pipelines(&view.pipelines, view.scope, &query);
    let latest_ids = latest_pipeline_ids_by_ref(&view.pipelines);
    let total_all = view.pipelines.len();
    let stages_cache = view.stages_cache.clone();

    let active_tab_border = theme.primary;
    let active_badge_bg = theme.primary;
    let active_badge_fg = theme.primary_foreground;
    let inactive_badge_bg: gpui_kit::Hsla = if dark { rgb(0x383839).into() } else { theme.secondary };
    let inactive_badge_fg: gpui_kit::Hsla = if dark { rgb(0xBFBFBF).into() } else { theme.muted_foreground };

    let tabs = div().flex().items_center().gap_1().children(PipelineScope::ALL.into_iter().map(|sc| {
        let active = view.scope == sc;
        let (bbg, bfg) = if active { (active_badge_bg, active_badge_fg) } else { (inactive_badge_bg, inactive_badge_fg) };
        div()
            .id(SharedString::from(format!("pipe-scope-{}", sc.label())))
            .px_3().py_2p5().cursor_pointer()
            .border_b_2().border_color(if active { active_tab_border } else { gpui_kit::transparent_black() })
            .flex().items_center().gap_1p5().text_sm()
            .text_color(if active { theme.foreground } else { theme.muted_foreground })
            .hover(|s| s.text_color(theme.foreground))
            .on_click(cx.listener(move |this, _, _, cx| { this.scope = sc; cx.notify(); }))
            .child(div().when(active, |d| d.font_semibold()).child(sc.label()))
            .children((sc == PipelineScope::All).then(|| {
                div().px_1p5().py(px(1.)).rounded_full().bg(bbg).text_color(bfg).text_xs().font_semibold().child(total_all.to_string())
            }))
            .test_support()
    }));

    let actions = div()
        .flex().items_center().gap_2().pb_1p5()
        .child(
            Button::new("pipe-layout-toggle").outline().small().label("3-pane")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_split_mode(cx))),
        )
        .children(can_run_ci.then(|| {
            Button::new("pipe-run").primary().small().label("New pipeline")
                .on_click(cx.listener(|this, _, w, cx| this.open_run_pipeline_dialog(w, cx)))
        }));

    let filter_bar = div()
        .id("pipe-filter-bar").mt_3().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border)
        .flex().items_center().gap_2()
        .child(
            div().flex_1().min_w_0().h(px(32.)).rounded_md().bg(theme.background)
                .border_1().border_color(theme.input).flex().items_center().px_2()
                .child(div().flex_1().min_w_0().child(Input::new(&view.pipe_filter_input).appearance(false).cleanable(true).small())),
        )
        .child(
            Button::new("pipe-table-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                .on_click(cx.listener(|this, _, _, cx| this.load_pipelines(cx))),
        )
        .test_support();

    let table_head = div()
        .w_full().max_w(px(1200.)).px_3().py_2p5().border_b_1().border_color(theme.border)
        .flex().items_center().gap_4().text_xs().font_semibold().text_color(theme.foreground)
        .child(div().w(px(142.)).flex_shrink_0().child("Status"))
        .child(div().flex_1().min_w_0().child("Pipeline"))
        .child(div().w(px(136.)).flex_shrink_0().child("Trigger"))
        .child(div().w(px(136.)).flex_shrink_0().child("Stages"))
        .child(div().w(px(96.)).flex_shrink_0().text_right().child("Actions"));

    let weak = cx.entity().downgrade();
    let list_body = if filtered.is_empty() {
        div().id("pipe-table-empty").py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
            .child("No pipelines match your current filter.").test_support().into_any_element()
    } else {
        uniform_list("pipelines-table-list", filtered.len(), move |range, _, cx| {
            let theme = cx.theme();
            let dark = theme.is_dark();
            range.map(|ix| {
                let p = filtered[ix].clone();
                let pid = p.id;
                let live = pipeline_is_live(&p.status);
                let badge = status_badge_palette(&p.status, dark);
                let rel = p.updated_at.as_deref().or(p.created_at.as_deref()).map(format_relative).unwrap_or_default();
                let ref_label = format_pipeline_ref(&p.git_ref);
                let short_sha: String = p.sha.chars().take(8).collect();
                let kind = pipeline_kind_tag(&p);
                let is_latest = latest_ids.contains(&pid);
                let src = p.source.clone().unwrap_or_else(|| "push".into()).replace('_', " ");
                let (latest_bg, latest_fg) = {
                    let s = status_badge_palette("success", dark);
                    (s.bg, s.text)
                };
                let (kind_bg, kind_fg) = {
                    let s = status_badge_palette("running", dark);
                    (s.bg, s.text)
                };

                let status_icon = if live {
                    Spinner::new().xsmall().color(badge.icon).into_any_element()
                } else {
                    Icon::empty().path(status_icon_path(&p.status)).size_3p5().text_color(badge.icon).into_any_element()
                };

                let cached_stages = stages_cache.get(&pid);
                let mut stages_el = div().w(px(136.)).flex_shrink_0().flex().items_center();
                if let Some(st_list) = cached_stages.filter(|v| !v.is_empty()) {
                    for (si, &st_name) in st_list.iter().take(5).enumerate() {
                        if si > 0 { stages_el = stages_el.child(div().w(px(6.)).h(px(1.5)).bg(theme.border)); }
                        let sc = status_color(st_name, dark);
                        stages_el = stages_el.child(Icon::empty().path(status_icon_path(st_name)).size(px(20.)).text_color(sc));
                    }
                } else {
                    let sc = status_color(&p.status, dark);
                    stages_el = stages_el.child(Icon::empty().path(status_icon_path(&p.status)).size(px(20.)).text_color(sc));
                }

                let (w_sel, w_act) = (weak.clone(), weak.clone());
                let (p_click, p_act, web_url) = (p.clone(), p.clone(), p.web_url.clone());

                div()
                    .w_full().h(px(74.)).px_6().flex().flex_col().items_center()
                    .child(
                        div()
                            .id(SharedString::from(format!("pipe-table-row-{pid}")))
                            .w_full().max_w(px(1200.)).h_full().px_3()
                            .border_b_1().border_color(theme.border.opacity(0.6))
                            .cursor_pointer().hover(|s| s.bg(theme.list_hover.opacity(0.55)))
                            .flex().items_center().gap_4()
                            .on_click(move |_, _, cx| {
                                let _ = w_sel.update(cx, |v, cx| v.select_pipeline(p_click.clone(), cx));
                            })
                            .child(
                                div().w(px(142.)).flex_shrink_0().flex().flex_col().gap_1()
                                    .child(
                                        div().flex().items_center().child(
                                            div().px_2().py(px(2.)).rounded_full().bg(badge.bg).text_color(badge.text)
                                                .flex().items_center().gap_1().text_xs().font_semibold()
                                                .child(status_icon).child(pipeline_status_label(&p.status)),
                                        ),
                                    )
                                    .children((!rel.is_empty()).then(|| div().text_xs().text_color(theme.muted_foreground).child(rel))),
                            )
                            .child(
                                div().flex_1().min_w_0().flex().flex_col().gap_1()
                                    .child(
                                        div().flex().items_center().gap_2().min_w_0()
                                            .child(div().text_sm().font_semibold().text_color(theme.foreground).child(format!("#{pid}")))
                                            .child(div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.foreground).max_w(px(240.)).truncate().child(ref_label))
                                            .child(div().font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.muted_foreground).child(short_sha)),
                                    )
                                    .child(
                                        div().flex().items_center().gap_1p5()
                                            .children(is_latest.then(|| {
                                                div().px_1p5().py(px(1.)).rounded_full().bg(latest_bg).text_color(latest_fg).text_xs().font_semibold().child("latest")
                                            }))
                                            .child(div().px_1p5().py(px(1.)).rounded_full().bg(kind_bg).text_color(kind_fg).text_xs().font_semibold().child(kind)),
                                    ),
                            )
                            .child(div().w(px(136.)).flex_shrink_0().text_xs().text_color(theme.muted_foreground).truncate().child(src))
                            .child(stages_el)
                            .child(
                                div().w(px(96.)).flex_shrink_0().flex().items_center().justify_end().gap_1()
                                    .children(can_run_ci.then(|| {
                                        let kind_str = if live { "cancel" } else { "retry" };
                                        let icon = if live { IconName::Close } else { IconName::RefreshCw };
                                        Button::new(SharedString::from(format!("pipe-row-act-{pid}"))).outline().xsmall().icon(Icon::new(icon).size_3())
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                let _ = w_act.update(cx, |v, cx| {
                                                    v.selected_pipeline = Some(p_act.clone());
                                                    v.run_pipeline_action(kind_str, cx);
                                                });
                                            })
                                    }))
                                    .child(
                                        Button::new(SharedString::from(format!("pipe-row-web-{pid}"))).ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3())
                                            .on_click(move |_, _, cx| { cx.stop_propagation(); let _ = safe_open_url(&web_url); }),
                                    ),
                            )
                            .test_support(),
                    )
            }).collect()
        })
        .flex_1()
        .size_full()
        .into_any_element()
    };

    div()
        .id("pipelines-table-view")
        .size_full().flex().flex_col()
        .child(
            div().w_full().px_6().pt_3().flex_shrink_0().flex().flex_col().items_center()
                .child(
                    div().w_full().max_w(px(1200.)).flex().flex_col()
                        .child(div().w_full().border_b_1().border_color(theme.border).flex().items_end().justify_between().gap_4().child(tabs).child(actions))
                        .child(filter_bar),
                )
                .child(table_head),
        )
        .child(div().flex_1().min_h_0().w_full().flex().flex_col().child(list_body))
        .test_support()
}
