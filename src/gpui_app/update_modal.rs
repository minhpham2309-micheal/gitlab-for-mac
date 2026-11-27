use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    },
    div, px, AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::updater::manifest::format_bytes;
use crate::updater::{UpdateInfo, CURRENT_VERSION};

use super::pajamas_theme::status_color;
use super::state::AppState;
use super::update_modal_body::render_phase_body_and_footer;

pub fn render_update_modal(state: &Entity<AppState>, cx: &App) -> impl IntoElement + use<> {
    let st = state.read(cx);
    let theme = cx.theme();
    let dark = theme.is_dark();
    let phase = st.update.phase.clone();
    let target = st.update.install_target.clone();
    let can_dismiss = st.update.can_dismiss_modal();
    let channel_label = st.prefs.update_channel.label();

    let close_bg = state.clone();
    let close_btn_st = state.clone();
    let header = div()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::empty()
                        .path("icons/glab-rocket.svg")
                        .size_4()
                        .text_color(status_color("success", dark)),
                )
                .child(div().text_sm().text_color(theme.foreground).child("GitLab for Mac Update"))
                .child(
                    div()
                        .px_2()
                        .py(px(1.))
                        .rounded_full()
                        .bg(theme.secondary)
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(channel_label),
                ),
        )
        .child(
            Button::new("update-modal-close")
                .ghost()
                .xsmall()
                .disabled(!can_dismiss)
                .label("×")
                .on_click(move |_, _, cx| {
                    close_btn_st.update(cx, |s, cx| {
                        s.close_update_modal(cx);
                    });
                }),
        );

    let (body, footer) = render_phase_body_and_footer(&phase, &target, state, cx);

    div()
        .id("update-modal-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(px(96.))
        .bg(theme.overlay)
        .on_click(move |_, _, cx| {
            if can_dismiss {
                close_bg.update(cx, |s, cx| {
                    s.close_update_modal(cx);
                });
            }
        })
        .child(
            div()
                .id("update-modal-card")
                .w(px(540.))
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .bg(theme.popover)
                .flex()
                .flex_col()
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(header)
                .child(div().p_4().flex().flex_col().gap_3().child(body))
                .child(
                    div()
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(footer),
                ),
        )
}

pub(super) fn render_release_summary(info: &UpdateInfo, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let dark = theme.is_dark();
    let size_text = if info.asset_size > 0 {
        format!(" · {}", format_bytes(info.asset_size))
    } else {
        String::new()
    };
    let date_text = info
        .published_at
        .as_deref()
        .and_then(|s| s.get(..10))
        .map(|d| format!(" · {d}"))
        .unwrap_or_default();
    let notes = if info.notes.is_empty() {
        "No release notes provided for this version.".to_string()
    } else {
        info.notes.clone()
    };

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().flex_1().min_w_0().text_sm().text_color(theme.foreground).child(info.name.clone()))
                .child(
                    div()
                        .flex_shrink_0()
                        .px_2()
                        .py(px(1.))
                        .rounded_full()
                        .bg(status_color("success", dark).opacity(0.18))
                        .text_color(status_color("success", dark))
                        .text_xs()
                        .font_family(theme.mono_font_family.clone())
                        .child(format!("v{CURRENT_VERSION} → v{}", info.version)),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("{}{size_text}{date_text}", info.asset_name)),
        )
        .child(
            div()
                .id("update-release-notes")
                .max_h(px(210.))
                .overflow_y_scroll()
                .p_3()
                .rounded_md()
                .bg(theme.muted)
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .text_color(theme.foreground)
                .child(notes),
        )
        .into_any_element()
}
