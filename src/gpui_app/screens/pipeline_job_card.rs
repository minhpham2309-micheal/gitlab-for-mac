use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Icon, Sizable as _,
    },
    div, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _,
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

    let status_el = if live {
        Spinner::new().xsmall().color(color).into_any_element()
    } else {
        Icon::empty()
            .path(status_icon_path(&job.status))
            .size_3p5()
            .text_color(color)
            .into_any_element()
    };

    let mut actions = div().flex().items_center().gap_1().flex_shrink_0();
    if can_run_ci && !card.is_bridge {
        if job.status == "manual" {
            let j_play = job.clone();
            actions = actions.child(
                Button::new(SharedString::from(format!("jg-play-{job_id}")))
                    .ghost()
                    .xsmall()
                    .label("Play")
                    .on_click(cx.listener(move |this, _, w, cx| {
                        cx.stop_propagation();
                        this.confirm_play_job(j_play.clone(), w, cx);
                    })),
            );
        } else if live {
            actions = actions.child(
                Button::new(SharedString::from(format!("jg-cancel-{job_id}")))
                    .ghost()
                    .xsmall()
                    .label("Cancel")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.run_job_action(job_id, "cancel", cx);
                    })),
            );
        } else {
            actions = actions.child(
                Button::new(SharedString::from(format!("jg-retry-{job_id}")))
                    .ghost()
                    .xsmall()
                    .label("↻")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.run_job_action(job_id, "retry", cx);
                    })),
            );
        }
    }

    div()
        .id(SharedString::from(format!("job-card-{job_id}")))
        .px_2()
        .py_1p5()
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
        .gap_1()
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
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .min_w_0()
                        .child(status_el)
                        .child(div().text_xs().text_color(theme.foreground).truncate().child(job.name)),
                )
                .child(actions),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .text_xs()
                .text_color(theme.muted_foreground)
                .children((!dur.is_empty()).then(|| div().child(dur)))
                .children((card.retry_count > 0).then(|| {
                    div().text_color(theme.warning).child(format!("↻ {}", card.retry_count))
                }))
                .children(allow_fail.then(|| div().text_color(theme.warning).child("allowed")))
                .children(card.is_bridge.then(|| div().text_color(theme.info).child("downstream"))),
        )
}
