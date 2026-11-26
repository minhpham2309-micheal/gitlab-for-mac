mod commit_banner;
pub mod file_icons;
mod file_table;
mod header;
mod info_panel;
mod loaders;
mod loaders_extra;
mod readme_card;
mod toolbar;

use gpui_kit::{
    component::ActiveTheme as _, div, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    Window,
};

use crate::gitlab::{
    Branch, LanguageSlice, ProjectDetail, ProjectOverviewCounts, RepoCommit, RepoTag,
    RepoTreeEntry, SpecialRepoFiles,
};
use crate::nav::Screen;

use super::super::markdown::MarkdownProjectContext;
use super::super::state::{AppEvent, AppState, ScreenCommand};
use self::commit_banner::render_commit_banner;
use self::file_table::render_file_table;
use self::header::{render_namespace_bar, render_project_header};
use self::info_panel::render_info_panel;
use self::readme_card::render_readme_card;
use self::toolbar::render_repo_toolbar;

pub struct ProjectOverviewView {
    pub(super) state: Entity<AppState>,
    pub(super) current_ref: String,
    pub(super) current_path: String,
    pub(super) ref_epoch: u64,
    pub(super) detail: Option<ProjectDetail>,
    pub(super) languages: Vec<LanguageSlice>,
    pub(super) counts: ProjectOverviewCounts,
    pub(super) branches: Vec<Branch>,
    pub(super) tags: Vec<RepoTag>,
    pub(super) entries: Vec<RepoTreeEntry>,
    pub(super) root_special: SpecialRepoFiles,
    pub(super) latest_commit: Option<RepoCommit>,
    pub(super) commit_loading: bool,
    pub(super) commit_expanded: bool,
    pub(super) readme_entry: Option<RepoTreeEntry>,
    pub(super) readme_text: Option<String>,
    pub(super) tree_loading: bool,
    pub(super) commits_loading: bool,
    pub(super) server_supports_commits: bool,
    pub(super) starred: bool,
    pub(super) meta_task: Option<Task<()>>,
    pub(super) tree_task: Option<Task<()>>,
    pub(super) hydrate_task: Option<Task<()>>,
    pub(super) commit_task: Option<Task<()>>,
    pub(super) readme_task: Option<Task<()>>,
    pub(super) star_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ProjectOverviewView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.reset_for_project(cx);
                if this.state.read(cx).screen == Screen::ProjectOverview { this.load_all(false, cx); }
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::ProjectOverview => {
                if this.entries.is_empty() && !this.tree_loading { this.load_all(false, cx); }
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::ProjectOverview => { this.load_all(true, cx); }
            AppEvent::Command(ScreenCommand::OpenOverviewRef { git_ref, path }) => {
                let changed = this.current_ref != *git_ref || this.current_path != *path;
                if changed || this.entries.is_empty() {
                    this.current_ref = git_ref.clone();
                    this.current_path = path.clone();
                    this.load_directory(false, cx);
                }
            }
            _ => {}
        });
        Self {
            state, current_ref: "main".into(), current_path: String::new(), ref_epoch: 1,
            detail: None, languages: Vec::new(), counts: ProjectOverviewCounts::default(),
            branches: Vec::new(), tags: Vec::new(), entries: Vec::new(),
            root_special: SpecialRepoFiles::default(), latest_commit: None, commit_loading: false,
            commit_expanded: false, readme_entry: None, readme_text: None, tree_loading: false,
            commits_loading: false, server_supports_commits: true, starred: false,
            meta_task: None, tree_task: None, hydrate_task: None, commit_task: None,
            readme_task: None, star_task: None, _subs: vec![sub],
        }
    }

    pub fn current_ref(&self) -> &str { &self.current_ref }
    pub fn current_path(&self) -> &str { &self.current_path }

    pub fn set_fixture_tree(&mut self, entries: Vec<RepoTreeEntry>, supports_commits: bool, cx: &mut Context<Self>) {
        self.entries = entries;
        self.tree_loading = false;
        self.commits_loading = false;
        self.server_supports_commits = supports_commits;
        self.refresh_special_files();
        cx.notify();
    }

    fn load_all(&mut self, force: bool, cx: &mut Context<Self>) {
        self.load_project_meta(force, cx);
        self.load_directory(force, cx);
    }

    pub(super) fn select_ref(&mut self, git_ref: String, cx: &mut Context<Self>) {
        if self.current_ref != git_ref {
            self.current_ref = git_ref.clone();
            self.current_path.clear();
            self.state.update(cx, |s, cx| s.record_overview_dir(git_ref, String::new(), cx));
            self.load_directory(false, cx);
        }
    }

    pub(super) fn navigate_dir(&mut self, path: String, cx: &mut Context<Self>) {
        if self.current_path != path {
            self.current_path = path.clone();
            let git_ref = self.current_ref.clone();
            self.state.update(cx, |s, cx| s.record_overview_dir(git_ref, path, cx));
            self.load_directory(false, cx);
        }
    }

    pub(super) fn toggle_commit_body(&mut self, cx: &mut Context<Self>) {
        self.commit_expanded = !self.commit_expanded;
        cx.notify();
    }

    pub(super) fn open_history(&mut self, cx: &mut Context<Self>) {
        let git_ref = Some(self.current_ref.clone());
        let path = (!self.current_path.is_empty()).then(|| self.current_path.clone());
        self.state.update(cx, |s, cx| s.open_commit_history(git_ref, path, cx));
    }
}

impl Render for ProjectOverviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (project, client, md_ctx) = {
            let st = self.state.read(cx);
            (st.project.clone(), st.gitlab.clone(), MarkdownProjectContext::from_state(st, cx))
        };
        let Some(project) = project else {
            return div()
                .size_full()
                .flex().items_center().justify_center()
                .text_sm().text_color(cx.theme().muted_foreground)
                .child("Select a project to view its overview and files")
                .into_any_element();
        };

        let ns_bar = render_namespace_bar(&project, &self.state, cx);
        let header = render_project_header(&project, self.detail.as_ref(), self.starred, client.as_ref(), cx);
        let toolbar = render_repo_toolbar(self, &project, self.detail.as_ref(), cx);
        let commit_bar = render_commit_banner(self.latest_commit.as_ref(), self.commit_loading, self.commit_expanded, &self.state, cx);
        let show_commits = self.commits_loading || self.server_supports_commits;
        let table = render_file_table(
            &self.entries, &self.current_path, self.tree_loading, self.commits_loading, show_commits, cx,
        );
        let readme = self.readme_entry.as_ref().zip(self.readme_text.as_deref())
            .map(|(entry, text)| render_readme_card(entry, text, md_ctx, cx));
        let info = render_info_panel(
            &project, self.detail.as_ref(), &self.languages, &self.counts,
            &self.root_special, &self.current_ref, &self.state, cx,
        );

        div()
            .size_full().flex().flex_col()
            .child(ns_bar)
            .child(
                div().id("overview-scroll").flex_1().min_h_0().overflow_y_scroll().px_6().py_4().child(
                    div().w_full().flex().flex_col().child(header).child(
                        div().w_full().flex().flex_row().items_start().gap_6()
                            .child(
                                div().flex_1().min_w_0().flex().flex_col().gap_3()
                                    .child(toolbar).child(commit_bar).child(table).children(readme),
                            )
                            .child(info),
                    ),
                ),
            )
            .into_any_element()
    }
}
