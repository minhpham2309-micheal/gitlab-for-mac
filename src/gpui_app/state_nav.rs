use gpui_kit::Context;

use crate::gitlab::Project;
use crate::nav::Screen;

use super::nav_history::{NavEntry, NavTarget};
use super::state::AppState;
use super::state_types::{AppEvent, ScreenCommand};

impl AppState {
    pub fn toggle_pinned(&mut self, cx: &mut Context<Self>) {
        self.pinned_open = !self.pinned_open;
        cx.notify();
    }

    pub fn set_active_ref(&mut self, git_ref: String) {
        if !git_ref.is_empty() {
            self.active_ref = Some(git_ref);
        }
    }

    pub fn can_go_back(&self) -> bool { self.history.can_go_back() }
    pub fn can_go_forward(&self) -> bool { self.history.can_go_forward() }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.history.go_back() {
            self.apply_nav_entry(entry, cx);
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.history.go_forward() {
            self.apply_nav_entry(entry, cx);
        }
    }

    fn current_ref_or_default(&self) -> String {
        self.active_ref
            .clone()
            .or_else(|| self.project.as_ref().and_then(|p| p.default_branch.clone()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main".into())
    }

    fn push_history(&mut self, entry: NavEntry, cx: &mut Context<Self>) {
        if self.history.push(entry) {
            cx.notify();
        }
    }

    fn switch_screen_no_history(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen != screen {
            self.screen = screen;
            self.reveal_nav();
            cx.emit(AppEvent::ScreenChanged { screen });
            cx.notify();
        }
    }

    fn open_with_entry(&mut self, entry: NavEntry, cmd: ScreenCommand, cx: &mut Context<Self>) {
        let screen = entry.screen;
        self.push_history(entry, cx);
        self.switch_screen_no_history(screen, cx);
        self.dispatch_command(cmd, cx);
    }

    pub fn select_project(&mut self, project: Project, cx: &mut Context<Self>) {
        self.select_project_on_screen(project, Screen::ProjectOverview, cx);
    }

    pub fn select_project_on_screen(&mut self, project: Project, screen: Screen, cx: &mut Context<Self>) {
        let default_ref = project.default_branch.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "main".into());
        let target = if screen == Screen::ProjectOverview {
            NavTarget::OverviewDir { git_ref: default_ref, path: String::new() }
        } else {
            NavTarget::Screen
        };
        self.history.push(NavEntry { project_id: Some(project.id), screen, target });
        self.activate_project_no_history(project, screen, cx);
    }

    pub fn set_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen != screen {
            let project_id = self.project.as_ref().map(|p| p.id);
            let target = if screen == Screen::ProjectOverview && project_id.is_some() {
                NavTarget::OverviewDir {
                    git_ref: self.current_ref_or_default(),
                    path: self.history.last_overview_path(project_id),
                }
            } else {
                NavTarget::Screen
            };
            self.history.push(NavEntry { project_id, screen, target });
            self.switch_screen_no_history(screen, cx);
        }
    }

    pub fn record_overview_dir(&mut self, git_ref: String, path: String, cx: &mut Context<Self>) {
        self.set_active_ref(git_ref.clone());
        let project_id = self.project.as_ref().map(|p| p.id);
        self.push_history(NavEntry {
            project_id,
            screen: Screen::ProjectOverview,
            target: NavTarget::OverviewDir { git_ref, path },
        }, cx);
    }

    pub fn record_repo_file(
        &mut self, git_ref: Option<String>, path: String, blob_sha: Option<String>, line: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        let Some(project_id) = self.project.as_ref().map(|p| p.id) else { return; };
        self.push_history(NavEntry {
            project_id: Some(project_id),
            screen: Screen::Repository,
            target: NavTarget::RepoFile { git_ref, path, blob_sha, line },
        }, cx);
    }

    pub fn open_repo_file(
        &mut self, project_id: u64, git_ref: Option<String>, path: String, blob_sha: Option<String>,
        line: Option<u32>, cx: &mut Context<Self>,
    ) {
        if let Some(r) = git_ref.as_ref().filter(|s| !s.is_empty()) { self.active_ref = Some(r.clone()); }
        let entry = NavEntry {
            project_id: Some(project_id),
            screen: Screen::Repository,
            target: NavTarget::RepoFile {
                git_ref: git_ref.clone(), path: path.clone(), blob_sha: blob_sha.clone(), line,
            },
        };
        let cmd = ScreenCommand::OpenRepoFile { project_id, git_ref, path, blob_sha, line };
        self.open_with_entry(entry, cmd, cx);
    }

    pub fn open_commit_history(&mut self, git_ref: Option<String>, path: Option<String>, cx: &mut Context<Self>) {
        let entry = NavEntry {
            project_id: self.project.as_ref().map(|p| p.id),
            screen: Screen::Commits,
            target: NavTarget::Commits { git_ref: git_ref.clone(), path: path.clone() },
        };
        self.open_with_entry(entry, ScreenCommand::OpenCommitHistory { git_ref, path }, cx);
    }

    pub fn open_overview_ref(&mut self, git_ref: String, path: String, cx: &mut Context<Self>) {
        self.set_active_ref(git_ref.clone());
        let entry = NavEntry {
            project_id: self.project.as_ref().map(|p| p.id),
            screen: Screen::ProjectOverview,
            target: NavTarget::OverviewDir { git_ref: git_ref.clone(), path: path.clone() },
        };
        self.open_with_entry(entry, ScreenCommand::OpenOverviewRef { git_ref, path }, cx);
    }

    pub fn open_pipeline(&mut self, pipeline_id: u64, cx: &mut Context<Self>) {
        let entry = NavEntry {
            project_id: self.project.as_ref().map(|p| p.id),
            screen: Screen::Pipelines,
            target: NavTarget::Pipeline { pipeline_id },
        };
        self.open_with_entry(entry, ScreenCommand::OpenPipeline { pipeline_id }, cx);
    }

    fn apply_nav_entry(&mut self, entry: NavEntry, cx: &mut Context<Self>) {
        if let Some(pid) = entry.project_id {
            if self.project.as_ref().map(|p| p.id) != Some(pid) {
                if let Some(proj) = self.projects.iter().find(|p| p.id == pid).cloned() {
                    self.activate_project_no_history(proj, entry.screen, cx);
                }
            }
        }
        self.switch_screen_no_history(entry.screen, cx);
        match entry.target {
            NavTarget::Screen => {}
            NavTarget::OverviewDir { git_ref, path } => {
                self.set_active_ref(git_ref.clone());
                self.dispatch_command(ScreenCommand::OpenOverviewRef { git_ref, path }, cx);
            }
            NavTarget::RepoFile { git_ref, path, blob_sha, line } => {
                if let Some(r) = git_ref.as_ref().filter(|s| !s.is_empty()) { self.active_ref = Some(r.clone()); }
                if let Some(project_id) = entry.project_id.or_else(|| self.project.as_ref().map(|p| p.id)) {
                    let cmd = ScreenCommand::OpenRepoFile { project_id, git_ref, path, blob_sha, line };
                    self.dispatch_command(cmd, cx);
                }
            }
            NavTarget::Commits { git_ref, path } => {
                self.dispatch_command(ScreenCommand::OpenCommitHistory { git_ref, path }, cx);
            }
            NavTarget::Pipeline { pipeline_id } => {
                self.dispatch_command(ScreenCommand::OpenPipeline { pipeline_id }, cx);
            }
        }
        cx.notify();
    }
}
