use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Icon, Sizable as _,
    },
    div, px, uniform_list, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::{pipeline_is_live, Pipeline};

use super::super::pajamas_theme::{status_color, status_icon_path};
use super::pipelines::PipelinesView;

pub fn render_pipelines_list(
    pipes: Vec<Pipeline>,
    selected_pipeline: Option<&Pipeline>,
    can_run_ci: bool,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let sel_pid = selected_pipeline.map(|p| p.id);
    let weak = cx.entity().downgrade();

    let top_controls = div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .child(div().text_sm().text_color(theme.foreground).child(format!("Pipelines ({})", pipes.len())))
        .child(
            div()
                .flex()
                .gap_1()
                .child(
                    Button::new("pipe-layout-toggle")
                        .ghost()
                        .xsmall()
                        .label("Web")
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_split_mode(cx))),
                )
                .children(can_run_ci.then(|| {
                    Button::new("pipe-run")
                        .primary()
                        .xsmall()
                        .label("Run")
                        .on_click(cx.listener(|this, _, w, cx| this.open_run_pipeline_dialog(w, cx)))
                }))
                .children(selected_pipeline.filter(|_| can_run_ci).map(|p| {
                    if pipeline_is_live(&p.status) {
                        Button::new("pipe-cancel")
                            .danger()
                            .xsmall()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, w, cx| this.confirm_cancel_pipeline(w, cx)))
                    } else {
                        Button::new("pipe-retry")
                            .outline()
                            .xsmall()
                            .label("Retry")
                            .on_click(cx.listener(|this, _, _, cx| this.run_pipeline_action("retry", cx)))
                    }
                })),
        );

    let pipe_list = uniform_list("pipelines-uniform-list", pipes.len(), move |range, _, cx| {
        let theme = cx.theme();
        range
            .map(|ix| {
                let p = pipes[ix].clone();
                let active = sel_pid == Some(p.id);
                let live = pipeline_is_live(&p.status);
                let color = status_color(&p.status, dark);
                let short_sha: String = p.sha.chars().take(8).collect();
                let w = weak.clone();
                let clicked = p.clone();
                let status_el = if live {
                    Spinner::new().xsmall().color(color).into_any_element()
                } else {
                    Icon::empty()
                        .path(status_icon_path(&p.status))
                        .size_3p5()
                        .text_color(color)
                        .into_any_element()
                };
                div()
                    .id(SharedString::from(format!("pipe-row-{}", p.id)))
                    .w_full()
                    .h(px(58.))
                    .px_3()
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .cursor_pointer()
                    .bg(if active { theme.list_active } else { theme.colors.list })
                    .hover(|s| s.bg(theme.list_hover))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_1()
                    .on_click(move |_, _, cx| {
                        let _ = w.update(cx, |v, cx| v.select_pipeline(clicked.clone(), cx));
                    })
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w_0()
                                    .child(div().flex_shrink_0().flex().items_center().child(status_el))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("pipe-name-{}", p.id)))
                                            .flex_1()
                                            .min_w_0()
                                            .text_sm()
                                            .text_color(theme.foreground)
                                            .truncate()
                                            .child(p.git_ref)
                                            .test_support(),
                                    ),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("pipe-id-{}", p.id)))
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_xs()
                                    .font_family(theme.mono_font_family.clone())
                                    .text_color(theme.muted_foreground)
                                    .child(format!("#{}", p.id))
                                    .test_support(),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{short_sha} · {}", p.source.unwrap_or_default())),
                    )
                    .test_support()
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
        .child(top_controls)
        .child(pipe_list)
}
