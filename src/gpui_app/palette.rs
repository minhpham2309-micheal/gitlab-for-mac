use gpui_kit::{
    component::{
        command::{Command, CommandGroup, CommandItem, CommandState},
        ActiveTheme as _, Disableable as _, Icon, IconName,
    },
    div, px, Action, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window,
};

use super::actions::*;
use super::state::AppState;

fn nav_item(label: &str, icon: Icon, kw: &[&str], disabled: bool, act: impl Action) -> CommandItem {
    CommandItem::new()
        .label(label.to_string())
        .icon(icon)
        .keywords(kw.iter().copied())
        .disabled(disabled)
        .action(Box::new(act))
}

fn act_item(label: &str, disabled: bool, act: impl Action) -> CommandItem {
    CommandItem::new()
        .label(label.to_string())
        .disabled(disabled)
        .action(Box::new(act))
}

pub fn render_palette_overlay<F: Fn(&mut Window, &mut App) + Clone + 'static>(
    cmd_state: &Entity<CommandState>,
    state: &Entity<AppState>,
    on_close: F,
    cx: &App,
) -> impl IntoElement + use<F> {
    let st = state.read(cx);
    let theme = cx.theme();
    let no_proj = st.project.is_none();
    let can_ci = !no_proj && st.project_permissions.can_run_pipelines();
    let no_pipe_live = !st.summary.active_pipeline_live;
    let no_job = !st.summary.has_active_job;
    let no_job_manual = !st.summary.active_job_manual;
    let can_approve_mr = st.summary.can_approve_mr;
    let can_merge_mr = st.summary.can_merge_mr;
    let can_close_mr = st.summary.can_close_mr;
    let no_url = st.summary.active_web_url.is_none();
    let no_back = !st.can_go_back();
    let no_fwd = !st.can_go_forward();

    let nav_group = CommandGroup::new()
        .label("Navigation")
        .item(nav_item("Go back (⌘[)", Icon::new(IconName::ArrowLeft), &["back", "previous", "history"], no_back, NavBack))
        .item(nav_item("Go forward (⌘])", Icon::new(IconName::ArrowRight), &["forward", "next", "history"], no_fwd, NavForward))
        .item(nav_item("Projects", Icon::new(IconName::Folder), &["repos", "switch"], false, GoProjects))
        .item(nav_item("Project overview", Icon::new(IconName::LayoutDashboard), &["home", "files", "readme"], no_proj, GoOverview))
        .item(nav_item("Repository / Source", Icon::new(IconName::FolderClosed), &["code", "source", "files"], no_proj, GoRepository))
        .item(nav_item("Find file (⌘T)", Icon::new(IconName::Search), &["quick open", "fuzzy", "tree"], no_proj, OpenFindFile))
        .item(nav_item("To-Do List", Icon::empty().path("icons/glab-todo.svg"), &["inbox"], false, GoTodos))
        .item(nav_item("Pipelines", Icon::empty().path("icons/glab-rocket.svg"), &["ci", "jobs"], no_proj, GoPipelines))
        .item(nav_item("Merge requests", Icon::empty().path("icons/glab-mr.svg"), &["mr", "diff"], no_proj, GoMergeRequests))
        .item(nav_item("Branches", Icon::empty().path("icons/glab-branch.svg"), &["git", "refs"], no_proj, GoBranches))
        .item(nav_item("Tags", Icon::empty().path("icons/glab-tag.svg"), &["git", "refs"], no_proj, GoTags))
        .item(nav_item("Commits", Icon::new(IconName::FileText), &["history", "log"], no_proj, GoCommits))
        .item(nav_item("Issues", Icon::empty().path("icons/glab-issue.svg"), &["plan"], no_proj, GoIssues))
        .item(nav_item("Releases", Icon::empty().path("icons/glab-tag.svg"), &["tags"], no_proj, GoReleases))
        .item(nav_item("Pipeline schedules", Icon::new(IconName::Calendar), &["cron"], no_proj, GoSchedules))
        .item(nav_item("Runners", Icon::new(IconName::Cpu), &[], no_proj, GoRunners))
        .item(nav_item("Code search", Icon::empty().path("icons/glab-search.svg"), &[], no_proj, GoSearch))
        .item(nav_item("glab console", Icon::empty().path("icons/glab-terminal.svg"), &["cli"], false, GoConsole))
        .item(nav_item("Preferences", Icon::empty().path("icons/glab-settings.svg"), &["settings", "font", "theme", "ui"], false, GoPreferences));

    let mut ci_group = CommandGroup::new().label("Pipeline & Job Actions");
    if can_ci {
        ci_group = ci_group
            .item(act_item("Run pipeline", no_proj, RunPipelineAction))
            .item(act_item("Retry pipeline", no_proj, RetryPipelineAction))
            .item(act_item("Cancel pipeline", no_pipe_live, CancelPipelineAction))
            .item(act_item("Retry job", no_job, RetryJobAction))
            .item(act_item("Cancel job", no_job, CancelJobAction))
            .item(act_item("Play manual job", no_job_manual, PlayJobAction));
    }
    ci_group = ci_group
        .item(act_item("Toggle follow log", no_job, ToggleFollowLog))
        .item(act_item("Copy full log", no_job, CopyFullLog))
        .item(act_item("Download job artifacts", no_job, DownloadArtifactsAction));

    let has_mr_actions = can_approve_mr || can_merge_mr || can_close_mr;
    let mut mr_group = CommandGroup::new().label("Merge Request Actions");
    if can_approve_mr {
        mr_group = mr_group.item(act_item("Approve merge request", false, ApproveMrAction));
    }
    if can_merge_mr {
        mr_group = mr_group.item(act_item("Merge merge request", false, MergeMrAction));
    }
    if can_close_mr {
        mr_group = mr_group.item(act_item("Close merge request", false, CloseMrAction));
    }

    let tools_group = CommandGroup::new()
        .label("Tools")
        .item(act_item("Refresh current screen", false, RefreshScreen))
        .item(act_item("Open active item in browser", no_url, OpenInBrowser))
        .item(act_item("Toggle sidebar", false, ToggleSidebar))
        .item(act_item("Toggle dark / light theme", false, ToggleTheme))
        .item(act_item("Increase UI font size (⌘+)", false, ZoomIn))
        .item(act_item("Decrease UI font size (⌘-)", false, ZoomOut))
        .item(act_item("Reset UI font size (⌘0)", false, ZoomReset))
        .item(act_item("Check for updates", false, CheckForUpdates));

    let close_cancel = on_close.clone();
    let close_confirm = on_close.clone();
    let close_backdrop = on_close;

    let mut command = Command::new(cmd_state)
        .placeholder("Type a command or jump to screen…")
        .highlight_matches(true)
        .max_h(px(380.))
        .group(nav_group)
        .group(ci_group);
    if has_mr_actions {
        command = command.group(mr_group);
    }
    command = command
        .group(tools_group)
        .on_confirm(move |_, window, cx| close_confirm(window, cx))
        .on_cancel(move |window, cx| close_cancel(window, cx));

    div()
        .id("command-palette-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(px(88.))
        .bg(theme.overlay)
        .on_click(move |_, window, cx| close_backdrop(window, cx))
        .child(
            div()
                .id("command-palette-card")
                .w(px(560.))
                .rounded_lg()
                .shadow_lg()
                .bg(theme.popover)
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(command),
        )
}
