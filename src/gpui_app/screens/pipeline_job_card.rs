use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Icon, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::job_is_live;

use super::super::pajamas_theme::{status_color, status_icon_path};
use super::pipeline_graph_model::StageJobCard;
use super::pipelines::PipelinesView;

fn format_duration(secs: Option<f64>) -> String {
    let Some(s) = secs else {
        return String::new();
    };
    let total = s.max(0.0).round() as u64;
    if total >= 60 {
        format!("{}m {:02}s", total / 60, total % 60)
    } else {
        format!("{total}s")
    }
}

pub fn render_job_card(
    card: StageJobCard,
    selected_job_id: Option<u64>,
    dark: bool,
    scale: f32,
    can_run_ci: bool,
    cx: &mut Context<PipelinesView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let job = card.job;
    let job_id = job.id;
    let is_selected = selected_job_id == Some(job_id);
    let live = job_is_live(&job.status);
    let color = status_color(&job.status, dark);
    let dur = format_duration(job.duration);
    let allow_fail = job.allow_failure.unwrap_or(false) && job.status == "failed";
    let click_job = job.clone();

    let pad_x = px((8.0 * scale).round().max(4.0));
    let pad_y = px((6.0 * scale).round().max(3.0));
    let gap = px((4.0 * scale).round().max(2.0));
    let icon_sz = px((14.0 * scale).round().clamp(10.0, 26.0));
    let title_sz = px((12.0 * scale).round().clamp(9.0, 22.0));
    let meta_sz = px((11.0 * scale).round().clamp(8.5, 20.0));
    let large_btn = scale >= 1.35;

    let status_el = if live {
        if scale >= 1.3 {
            Spinner::new().small().color(color).into_any_element()
        } else {
            Spinner::new().xsmall().color(color).into_any_element()
        }
    } else {
        Icon::empty()
            .path(status_icon_path(&job.status))
            .size(icon_sz)
            .text_color(color)
            .into_any_element()
    };

    let mut actions = div().flex().items_center().gap_1().flex_shrink_0();
    if can_run_ci && !card.is_bridge {
        let btn = if job.status == "manual" {
            let j_play = job.clone();
            Button::new(SharedString::from(format!("jg-play-{job_id}")))
                .ghost()
                .label("Play")
                .on_click(cx.listener(move |this, _, w, cx| {
                    cx.stop_propagation();
                    this.confirm_play_job(j_play.clone(), w, cx);
                }))
        } else if live {
            Button::new(SharedString::from(format!("jg-cancel-{job_id}")))
                .ghost()
                .label("Cancel")
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.run_job_action(job_id, "cancel", cx);
                }))
        } else {
            Button::new(SharedString::from(format!("jg-retry-{job_id}")))
                .ghost()
                .label("↻")
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.run_job_action(job_id, "retry", cx);
                }))
        };
        actions = actions.child(if large_btn { btn.small() } else { btn.xsmall() });
    }

    div()
        .id(SharedString::from(format!("job-card-{job_id}")))
        .px(pad_x)
        .py(pad_y)
        .rounded_md()
        .cursor_pointer()
        .border_1()
        .border_color(if is_selected {
            theme.primary
        } else {
            theme.border.opacity(0.7)
        })
        .bg(if is_selected {
            theme.list_active
        } else {
            theme.colors.list
        })
        .hover(|s| s.bg(theme.list_hover))
        .flex()
        .flex_col()
        .gap(gap)
        .on_click(cx.listener(move |this, _, _, cx| {
            if !card.is_bridge {
                this.select_job(click_job.clone(), cx);
            }
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(gap)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(gap)
                        .min_w_0()
                        .child(status_el)
                        .child(div().text_size(title_sz).text_color(theme.foreground).truncate().child(job.name)),
                )
                .child(actions),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(gap)
                .text_size(meta_sz)
                .text_color(theme.muted_foreground)
                .children((!dur.is_empty()).then(|| div().child(dur)))
                .children((card.retry_count > 0).then(|| {
                    div().text_color(theme.warning).child(format!("↻ {}", card.retry_count))
                }))
                .children(allow_fail.then(|| div().text_color(theme.warning).child("allowed")))
                .children(card.is_bridge.then(|| div().text_color(theme.info).child("downstream"))),
        )
        .test_support()
}
