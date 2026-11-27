use gpui_kit::{Context, EventEmitter, Task};

use crate::config::HostAuth;
use crate::gitlab::{GitLab, Project, ProjectPermissions, User};
use crate::nav::{NavSection, Screen};
use crate::prefs::{self, Prefs};

use super::nav_history::{NavEntry, NavHistory};
pub use super::state_types::{AppEvent, ScreenCommand, ScreenContextSummary, SidebarCounts};

pub struct AppState {
    pub hosts: Vec<HostAuth>,
    pub active_host: Option<HostAuth>,
    pub gitlab: Option<GitLab>,
    pub current_user: Option<User>,
    pub projects: Vec<Project>,
    pub project: Option<Project>,
    pub project_permissions: ProjectPermissions,
    pub active_ref: Option<String>,
    pub screen: Screen,
    pub history: NavHistory,
    pub nav_open: u8,
    pub pinned_open: bool,
    pub sidebar_collapsed: bool,
    pub theme_dark: bool,
    pub prefs: Prefs,
    pub persist_prefs: bool,
    pub offline_mode: bool,
    pub status: String,
    pub busy: bool,
    pub host_epoch: u64,
    pub project_epoch: u64,
    pub counts: SidebarCounts,
    pub summary: ScreenContextSummary,
    pub(super) host_load_task: Option<Task<()>>,
    pub(super) counts_task: Option<Task<()>>,
}

impl EventEmitter<AppEvent> for AppState {}

impl AppState {
    pub fn from_parts(
        hosts: Vec<HostAuth>,
        active_host: Option<HostAuth>,
        projects: Vec<Project>,
        prefs: Prefs,
        status: impl Into<String>,
    ) -> Self {
        let gitlab = active_host.as_ref().and_then(|h| GitLab::new(h.clone()).ok());
        let theme_dark = prefs.theme_mode.as_deref() != Some("light");
        let sidebar_collapsed = prefs.sidebar_collapsed;
        let nav_open = NavSection::ALL.iter().fold(0u8, |acc, s| acc | s.bit());
        Self {
            hosts,
            active_host,
            gitlab,
            current_user: None,
            projects,
            project: None,
            project_permissions: ProjectPermissions::unknown(),
            active_ref: None,
            screen: Screen::Projects,
            history: NavHistory::new(NavEntry::screen(None, Screen::Projects)),
            nav_open,
            pinned_open: true,
            sidebar_collapsed,
            theme_dark,
            prefs,
            persist_prefs: false,
            offline_mode: true,
            status: status.into(),
            busy: false,
            host_epoch: 1,
            project_epoch: 1,
            counts: SidebarCounts::default(),
            summary: ScreenContextSummary::default(),
            host_load_task: None,
            counts_task: None,
        }
    }

    pub fn select_host(&mut self, hostname: &str, cx: &mut Context<Self>) {
        let Some(host) = self.hosts.iter().find(|h| h.hostname == hostname).cloned() else {
            self.status = format!("Unknown host {hostname}");
            cx.notify();
            return;
        };
        let client = match GitLab::new(host.clone()) {
            Ok(client) => client,
            Err(err) => {
                self.status = err.to_string();
                cx.notify();
                return;
            }
        };
        self.host_load_task = None;
        self.counts_task = None;
        self.host_epoch = self.host_epoch.wrapping_add(1);
        self.project_epoch = self.project_epoch.wrapping_add(1);
        self.active_host = Some(host);
        self.gitlab = Some(client);
        self.current_user = None;
        self.projects.clear();
        self.project = None;
        self.project_permissions = ProjectPermissions::unknown();
        self.active_ref = None;
        self.counts = SidebarCounts::default();
        self.summary = ScreenContextSummary::default();
        self.screen = Screen::Projects;
        self.history.reset(NavEntry::screen(None, Screen::Projects));
        self.prefs.host = Some(hostname.to_string());
        self.prefs.project = None;
        self.save_prefs();
        if let Some(cache) = cx.try_global::<super::image_cache::ImageCache>() {
            cache.clear();
        }
        if let Some(cache) = cx.try_global::<super::repo_cache::RepoCache>() {
            cache.clear();
        }
        self.status = format!("Switched to {hostname}");
        cx.emit(AppEvent::HostChanged { epoch: self.host_epoch });
        self.refresh_host_data(cx);
        cx.notify();
    }

    pub(super) fn activate_project_no_history(
        &mut self,
        project: Project,
        screen: Screen,
        cx: &mut Context<Self>,
    ) {
        self.project_epoch = self.project_epoch.wrapping_add(1);
        self.prefs.project = Some(project.path_with_namespace.clone());
        self.summary = ScreenContextSummary {
            active_web_url: Some(project.web_url.clone()),
            ..ScreenContextSummary::default()
        };
        self.counts.open_issues = None;
        self.counts.open_mrs = None;
        self.counts.live_pipelines = 0;
        self.active_ref = project.default_branch.clone().filter(|s| !s.is_empty());
        self.project = Some(project);
        self.project_permissions = ProjectPermissions::unknown();
        self.screen = screen;
        self.reveal_nav();
        self.save_prefs();
        cx.emit(AppEvent::ProjectChanged { epoch: self.project_epoch });
        cx.emit(AppEvent::ScreenChanged { screen });
        self.refresh_sidebar_counts(cx);
        cx.notify();
    }

    pub fn dispatch_command(&mut self, cmd: ScreenCommand, cx: &mut Context<Self>) {
        cx.emit(AppEvent::Command(cmd));
    }

    pub fn section_open(&self, section: NavSection) -> bool {
        self.nav_open & section.bit() != 0
    }

    pub fn reveal_nav(&mut self) {
        if let Some(section) = NavSection::of(self.screen) {
            self.nav_open |= section.bit();
        }
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.prefs.sidebar_collapsed = self.sidebar_collapsed;
        self.save_prefs();
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) -> Prefs {
        self.theme_dark = !self.theme_dark;
        self.prefs.theme_mode = Some(if self.theme_dark { "dark" } else { "light" }.into());
        self.save_prefs();
        cx.emit(AppEvent::PrefsChanged);
        cx.notify();
        self.prefs.clone()
    }

    pub fn set_project_permissions(&mut self, perms: ProjectPermissions, cx: &mut Context<Self>) {
        if self.project_permissions != perms {
            self.project_permissions = perms;
            cx.emit(AppEvent::PermissionsChanged);
            cx.notify();
        }
    }

    pub fn set_status(&mut self, status: impl Into<String>, cx: &mut Context<Self>) {
        self.status = status.into();
        cx.notify();
    }

    pub fn save_prefs(&self) {
        if self.persist_prefs {
            prefs::save(&self.prefs);
        }
    }
}
