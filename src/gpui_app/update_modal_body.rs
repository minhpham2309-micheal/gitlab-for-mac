use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        spinner::Spinner,
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    },
    div, px, relative, AnyElement, App, Entity, IntoElement, ParentElement as _, Styled as _,
};

use crate::updater::manifest::format_bytes;
use crate::updater::{current_macos_arch, InstallTarget, UpdateInfo, CURRENT_VERSION};

use super::pajamas_theme::status_color;
use super::safe_url::safe_open_url;
use super::state::{AppState, UpdatePhase};
use super::update_modal::render_release_summary;

pub(super) fn render_phase_body_and_footer(
    phase: &UpdatePhase,
    target: &InstallTarget,
    state: &Entity<AppState>,
    cx: &App,
) -> (AnyElement, AnyElement) {
    let theme = cx.theme();
    match phase {
        UpdatePhase::Checking => (
            div().py_4().flex().items_center().gap_2()
                .child(Spinner::new().small())
                .child(div().text_sm().text_color(theme.foreground).child("Checking GitHub Releases…"))
                .into_any_element(),
            close_footer(state, false),
        ),
        UpdatePhase::Idle => render_up_to_date(None, state, cx),
        UpdatePhase::UpToDate { note, .. } => render_up_to_date(note.as_deref(), state, cx),
        UpdatePhase::Available(info) => render_available(info, target, state, cx),
        UpdatePhase::Downloading { info, downloaded, total } => {
            render_downloading(info, *downloaded, *total, state, cx)
        }
        UpdatePhase::Verifying { info } | UpdatePhase::Installing { info } => {
            let is_installing = matches!(phase, UpdatePhase::Installing { .. });
            let msg = if is_installing {
                "Replacing app bundle and restarting GitLab for Mac…"
            } else {
                "Verifying SHA-256 checksum and macOS code signature…"
            };
            let body = div().flex().flex_col().gap_2p5()
                .child(render_release_summary(info, cx))
                .child(div().flex().items_center().gap_2().child(Spinner::new().small()).child(div().text_xs().text_color(theme.foreground).child(msg)))
                .into_any_element();
            (body, close_footer(state, is_installing))
        }
        UpdatePhase::Error { message, info } => render_error(message, info.as_deref(), state, cx),
    }
}

fn render_up_to_date(
    note: Option<&str>,
    state: &Entity<AppState>,
    cx: &App,
) -> (AnyElement, AnyElement) {
    let theme = cx.theme();
    let (st_check, st_close) = (state.clone(), state.clone());
    let detail = note.map(str::to_string).unwrap_or_else(|| {
        format!(
            "GitLab for Mac v{CURRENT_VERSION} ({}) is the latest release on this channel.",
            current_macos_arch()
        )
    });
    let body = div().flex().flex_col().gap_1p5()
        .child(div().text_sm().text_color(status_color("success", theme.is_dark())).child("You’re up to date"))
        .child(div().text_xs().text_color(theme.muted_foreground).child(detail))
        .into_any_element();
    let footer = div().w_full().flex().items_center().justify_end().gap_2()
        .child(Button::new("update-recheck-btn").outline().small().label("Check again").on_click(move |_, _, cx| {
            st_check.update(cx, |s, cx| s.check_for_updates(true, cx));
        }))
        .child(Button::new("update-done-btn").primary().small().label("Close").on_click(move |_, _, cx| {
            st_close.update(cx, |s, cx| { s.close_update_modal(cx); });
        }))
        .into_any_element();
    (body, footer)
}

fn render_available(
    info: &UpdateInfo,
    target: &InstallTarget,
    state: &Entity<AppState>,
    cx: &App,
) -> (AnyElement, AnyElement) {
    let theme = cx.theme();
    let note_box = target.info_note().map(|note| {
        div().p_2p5().rounded_md().bg(theme.muted).border_1().border_color(theme.border)
            .text_xs().text_color(theme.muted_foreground).child(note)
    });
    let body = div().flex().flex_col().gap_2p5()
        .child(render_release_summary(info, cx))
        .children(note_box)
        .into_any_element();

    let open_url = info.dmg_download_url.clone().unwrap_or_else(|| info.html_url.clone());
    let release_url = info.html_url.clone();
    let (st_skip, st_later, st_install) = (state.clone(), state.clone(), state.clone());
    let primary_btn = if target.can_install_in_app() {
        Button::new("update-install-btn")
            .primary().small().label(target.action_label())
            .on_click(move |_, _, cx| st_install.update(cx, |s, cx| s.start_update_install(cx)))
    } else {
        Button::new("update-open-release-btn")
            .primary().small().icon(Icon::new(IconName::ExternalLink).size_3p5()).label(target.action_label())
            .on_click(move |_, _, _| { let _ = safe_open_url(&open_url); })
    };
    let footer = div().w_full().flex().items_center().justify_between().gap_2()
        .child(Button::new("update-github-link").ghost().xsmall().icon(Icon::new(IconName::ExternalLink).size_3p5()).label("View on GitHub").on_click(move |_, _, _| {
            let _ = safe_open_url(&release_url);
        }))
        .child(
            div().flex().items_center().gap_1p5()
                .child(Button::new("update-skip-btn").ghost().small().label("Skip version").on_click(move |_, _, cx| {
                    st_skip.update(cx, |s, cx| s.dismiss_available_update(cx));
                }))
                .child(Button::new("update-later-btn").outline().small().label("Later").on_click(move |_, _, cx| {
                    st_later.update(cx, |s, cx| { s.close_update_modal(cx); });
                }))
                .child(primary_btn),
        )
        .into_any_element();
    (body, footer)
}

fn render_downloading(
    info: &UpdateInfo,
    downloaded: u64,
    total: Option<u64>,
    state: &Entity<AppState>,
    cx: &App,
) -> (AnyElement, AnyElement) {
    let theme = cx.theme();
    let ratio = total.filter(|&t| t > 0).map(|t| (downloaded as f32 / t as f32).clamp(0.0, 1.0)).unwrap_or(0.15);
    let pct_label = total
        .filter(|&t| t > 0)
        .map(|t| format!("{} / {} ({}%)", format_bytes(downloaded), format_bytes(t), ((downloaded.saturating_mul(100)) / t).min(100)))
        .unwrap_or_else(|| format!("Downloaded {}", format_bytes(downloaded)));
    let body = div().flex().flex_col().gap_2p5()
        .child(render_release_summary(info, cx))
        .child(
            div().flex().flex_col().gap_1()
                .child(div().text_xs().text_color(theme.foreground).child(format!("Downloading… {pct_label}")))
                .child(
                    div().w_full().h(px(6.)).rounded_full().bg(theme.secondary).overflow_hidden()
                        .child(div().h_full().w(relative(ratio)).bg(status_color("running", theme.is_dark()))),
                ),
        )
        .into_any_element();
    let st_cancel = state.clone();
    let footer = div().w_full().flex().justify_end()
        .child(Button::new("update-cancel-dl").outline().small().label("Cancel").on_click(move |_, _, cx| {
            st_cancel.update(cx, |s, cx| s.cancel_update_download(cx));
        }))
        .into_any_element();
    (body, footer)
}

fn render_error(
    message: &str,
    info: Option<&UpdateInfo>,
    state: &Entity<AppState>,
    cx: &App,
) -> (AnyElement, AnyElement) {
    let theme = cx.theme();
    let failed = status_color("failed", theme.is_dark());
    let body = div().p_3().rounded_md().bg(failed.opacity(0.12)).border_1().border_color(failed.opacity(0.4))
        .text_xs().text_color(theme.foreground).child(message.to_string()).into_any_element();
    let (st_retry, st_close) = (state.clone(), state.clone());
    let release_url = info.map(|i| i.html_url.clone());
    let footer = div().w_full().flex().items_center().justify_end().gap_2()
        .children(release_url.map(|url| {
            Button::new("update-err-open").ghost().small().icon(Icon::new(IconName::ExternalLink).size_3p5()).label("Open Releases")
                .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
        }))
        .child(Button::new("update-err-retry").outline().small().label("Retry").on_click(move |_, _, cx| {
            st_retry.update(cx, |s, cx| s.check_for_updates(true, cx));
        }))
        .child(Button::new("update-err-close").primary().small().label("Close").on_click(move |_, _, cx| {
            st_close.update(cx, |s, cx| { s.close_update_modal(cx); });
        }))
        .into_any_element();
    (body, footer)
}

fn close_footer(state: &Entity<AppState>, disabled: bool) -> AnyElement {
    let st_close = state.clone();
    div().w_full().flex().justify_end()
        .child(Button::new("update-modal-close-footer").outline().small().disabled(disabled).label("Close").on_click(move |_, _, cx| {
            st_close.update(cx, |s, cx| { s.close_update_modal(cx); });
        }))
        .into_any_element()
}
