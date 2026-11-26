use gpui_kit::{
    component::WindowExt as _, Action, Context, InteractiveElement as _, Stateful, Window,
};

use crate::nav::Screen;

use super::actions::*;
use super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::pajamas_theme::apply_theme_from_prefs;
use super::safe_url::safe_open_url;
use super::shell::AppShell;
use super::state::ScreenCommand;

fn on_go<A: Action>(
    el: Stateful<gpui_kit::Div>,
    screen: Screen,
    cx: &mut Context<AppShell>,
) -> Stateful<gpui_kit::Div> {
    el.on_action(cx.listener(move |this, _: &A, _, cx| {
        this.state().update(cx, |s, cx| s.set_screen(screen, cx));
    }))
}

fn on_cmd<A: Action>(
    el: Stateful<gpui_kit::Div>,
    cmd: ScreenCommand,
    cx: &mut Context<AppShell>,
) -> Stateful<gpui_kit::Div> {
    el.on_action(cx.listener(move |this, _: &A, _, cx| {
        this.state().update(cx, |s, cx| s.dispatch_command(cmd.clone(), cx));
    }))
}

#[allow(clippy::too_many_arguments)]
fn confirm_cmd(
    shell: &AppShell,
    title: &str,
    action_label: &str,
    target: String,
    detail: &str,
    cmd: ScreenCommand,
    window: &mut Window,
    cx: &mut Context<AppShell>,
) {
    let st = shell.state().read(cx);
    let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
    let (mr_iid, pipe_id, job_id) = (
        st.summary.active_mr_iid,
        st.summary.active_pipeline_id,
        st.summary.active_job_id,
    );
    let host = st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_else(|| "unknown".into());
    let project = st.project.as_ref().map(|p| p.path_with_namespace.clone()).unwrap_or_else(|| "none".into());
    let state = shell.state().clone();
    open_destructive_confirm(
        DestructiveConfirm {
            title: title.into(),
            action_label: action_label.into(),
            host,
            project,
            target,
            detail: detail.into(),
        },
        move |_, cx| {
            state.update(cx, |s, cx| {
                if s.host_epoch != h_ep || s.project_epoch != p_ep { return; }
                let ok = match cmd {
                    ScreenCommand::MergeMr | ScreenCommand::CloseMr => s.summary.active_mr_iid == mr_iid,
                    ScreenCommand::CancelPipeline => s.summary.active_pipeline_id == pipe_id,
                    ScreenCommand::PlayJob => s.summary.active_job_id == job_id,
                    _ => true,
                };
                if ok { s.dispatch_command(cmd.clone(), cx); }
            });
        },
        window,
        cx,
    );
}

pub fn attach_shell_actions(
    mut root: Stateful<gpui_kit::Div>,
    cx: &mut Context<AppShell>,
) -> Stateful<gpui_kit::Div> {
    root = root
        .on_action(cx.listener(|this, _: &TogglePalette, w, cx| this.toggle_palette(w, cx)))
        .on_action(cx.listener(|this, _: &DismissOverlay, w, cx| {
            if !this.dismiss_top_overlay(w, cx) {
                this.state().update(cx, |s, cx| s.dispatch_command(ScreenCommand::ClearLogSearch, cx));
            }
        }))
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
            this.state().update(cx, |s, cx| s.toggle_sidebar(cx));
        }))
        .on_action(cx.listener(|this, _: &ToggleTheme, _, cx| {
            let prefs = this.state().update(cx, |s, cx| s.toggle_theme(cx));
            apply_theme_from_prefs(&prefs, cx);
        }))
        .on_action(cx.listener(|this, _: &ZoomIn, _, cx| {
            let prefs = this.state().update(cx, |s, cx| s.step_ui_font_size(1, cx));
            apply_theme_from_prefs(&prefs, cx);
        }))
        .on_action(cx.listener(|this, _: &ZoomOut, _, cx| {
            let prefs = this.state().update(cx, |s, cx| s.step_ui_font_size(-1, cx));
            apply_theme_from_prefs(&prefs, cx);
        }))
        .on_action(cx.listener(|this, _: &ZoomReset, _, cx| {
            let prefs = this.state().update(cx, |s, cx| s.reset_ui_font_size(cx));
            apply_theme_from_prefs(&prefs, cx);
        }))
        .on_action(cx.listener(|this, _: &OpenInBrowser, window, cx| {
            let url = this.state().read(cx).summary.active_web_url.clone();
            if let Some(url) = url {
                match safe_open_url(&url) {
                    Ok(msg) => this.state().update(cx, |s, cx| s.set_status(msg, cx)),
                    Err(err) => {
                        window.push_notification(err.clone(), cx);
                        this.state().update(cx, |s, cx| s.set_status(err, cx));
                    }
                }
            }
        }));

    root = on_go::<GoProjects>(root, Screen::Projects, cx);
    root = on_go::<GoOverview>(root, Screen::ProjectOverview, cx);
    root = on_go::<GoRepository>(root, Screen::Repository, cx);
    root = on_go::<GoPipelines>(root, Screen::Pipelines, cx);
    root = on_go::<GoMergeRequests>(root, Screen::MergeRequests, cx);
    root = on_go::<GoBranches>(root, Screen::Branches, cx);
    root = on_go::<GoTags>(root, Screen::Tags, cx);
    root = on_go::<GoCommits>(root, Screen::Commits, cx);
    root = on_go::<GoIssues>(root, Screen::Issues, cx);
    root = on_go::<GoTodos>(root, Screen::Todos, cx);
    root = on_go::<GoReleases>(root, Screen::Releases, cx);
    root = on_go::<GoSchedules>(root, Screen::Schedules, cx);
    root = on_go::<GoRunners>(root, Screen::Runners, cx);
    root = on_go::<GoSearch>(root, Screen::Search, cx);
    root = on_go::<GoConsole>(root, Screen::Console, cx);
    root = on_go::<GoPreferences>(root, Screen::Preferences, cx);

    root = root
        .on_action(cx.listener(|this, _: &OpenFindFile, w, cx| this.open_find_file(w, cx)))
        .on_action(cx.listener(|this, _: &NavBack, w, cx| {
            let _ = this.dismiss_top_overlay(w, cx);
            this.state().update(cx, |s, cx| s.go_back(cx));
        }))
        .on_action(cx.listener(|this, _: &NavForward, w, cx| {
            let _ = this.dismiss_top_overlay(w, cx);
            this.state().update(cx, |s, cx| s.go_forward(cx));
        }))
        .on_action(cx.listener(|this, _: &FocusLogSearch, _, cx| {
            if !this.has_overlay_open(cx) {
                this.state().update(cx, |s, cx| s.dispatch_command(ScreenCommand::FocusLogSearch, cx));
            }
        }));
    root = on_cmd::<RefreshScreen>(root, ScreenCommand::Refresh, cx);
    root = on_cmd::<ToggleFollowLog>(root, ScreenCommand::ToggleFollowLog, cx);
    root = on_cmd::<LogFindNext>(root, ScreenCommand::LogFindNext, cx);
    root = on_cmd::<LogFindPrev>(root, ScreenCommand::LogFindPrev, cx);
    root = on_cmd::<CopyFullLog>(root, ScreenCommand::CopyFullLog, cx);
    root = on_cmd::<RunPipelineAction>(root, ScreenCommand::RunPipeline, cx);
    root = on_cmd::<RetryPipelineAction>(root, ScreenCommand::RetryPipeline, cx);
    root = on_cmd::<RetryJobAction>(root, ScreenCommand::RetryJob, cx);
    root = on_cmd::<CancelJobAction>(root, ScreenCommand::CancelJob, cx);
    root = on_cmd::<DownloadArtifactsAction>(root, ScreenCommand::DownloadArtifacts, cx);
    root = on_cmd::<ApproveMrAction>(root, ScreenCommand::ApproveMr, cx);

    root.on_action(cx.listener(|this, _: &CancelPipelineAction, w, cx| {
        let sum = &this.state().read(cx).summary;
        if !sum.active_pipeline_live { return; }
        let target = sum.active_pipeline_label.clone().unwrap_or_else(|| "selected pipeline".into());
        confirm_cmd(this, "Cancel pipeline", "Cancel pipeline", target, "Cancel the running pipeline and stop pending jobs.", ScreenCommand::CancelPipeline, w, cx);
    }))
    .on_action(cx.listener(|this, _: &PlayJobAction, w, cx| {
        let sum = &this.state().read(cx).summary;
        if !sum.active_job_manual { return; }
        let target = sum.active_job_label.clone().unwrap_or_else(|| "selected manual job".into());
        confirm_cmd(this, "Play manual job", "Play job", target, "Trigger execution of this manual CI job.", ScreenCommand::PlayJob, w, cx);
    }))
    .on_action(cx.listener(|this, _: &MergeMrAction, w, cx| {
        let sum = &this.state().read(cx).summary;
        if !sum.can_review_mr { return; }
        let target = sum.active_mr_label.clone().unwrap_or_else(|| "selected MR".into());
        confirm_cmd(this, "Merge merge request", "Merge", target, "Merge the active merge request into its target branch.", ScreenCommand::MergeMr, w, cx);
    }))
    .on_action(cx.listener(|this, _: &CloseMrAction, w, cx| {
        let sum = &this.state().read(cx).summary;
        if !sum.can_review_mr { return; }
        let target = sum.active_mr_label.clone().unwrap_or_else(|| "selected MR".into());
        confirm_cmd(this, "Close merge request", "Close MR", target, "Close this merge request without merging.", ScreenCommand::CloseMr, w, cx);
    }))
}
