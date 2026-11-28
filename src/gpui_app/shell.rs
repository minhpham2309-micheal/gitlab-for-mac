use gpui_kit::{
    component::{command::CommandState, ActiveTheme as _},
    div, AnyElement, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    TestSupportExt as _, Window,
};

use crate::nav::Screen;

use super::breadcrumb::render_panel_breadcrumb;
use super::pajamas_theme::{apply_theme_from_prefs, studio_panel_border, studio_shell_bg};
use super::palette::render_palette_overlay;
use super::screens::{
    code_refs::{BranchesView, CommitsView, TagsView},
    groups::GroupsView,
    issues::IssuesView,
    login::LoginView,
    merge_requests::MergeRequestsView,
    ops::{
        console::ConsoleView, releases::ReleasesView, runners::RunnersView,
        schedules::SchedulesView, search::SearchView,
    },
    overview::ProjectOverviewView,
    pipelines::PipelinesView,
    preferences::PreferencesView,
    projects::ProjectsView,
    repository::{file_finder::FindFileModal, RepositoryView},
    todos::TodosView,
};
use super::shell_actions::attach_shell_actions;
use super::sidebar::render_sidebar;
use super::state::AppState;
use super::statusbar::render_status_bar;
use super::topbar::render_top_bar;
use super::update_modal::render_update_modal;

pub struct AppShell {
    state: Entity<AppState>,
    command_state: Entity<CommandState>,
    projects_view: Entity<ProjectsView>,
    groups_view: Entity<GroupsView>,
    overview_view: Entity<ProjectOverviewView>,
    repo_view: Entity<RepositoryView>,
    pipelines_view: Entity<PipelinesView>,
    mrs_view: Entity<MergeRequestsView>,
    branches_view: Entity<BranchesView>,
    tags_view: Entity<TagsView>,
    commits_view: Entity<CommitsView>,
    issues_view: Entity<IssuesView>,
    todos_view: Entity<TodosView>,
    releases_view: Entity<ReleasesView>,
    schedules_view: Entity<SchedulesView>,
    runners_view: Entity<RunnersView>,
    search_view: Entity<SearchView>,
    console_view: Entity<ConsoleView>,
    preferences_view: Entity<PreferencesView>,
    login_view: Entity<LoginView>,
    find_file_modal: Entity<FindFileModal>,
    palette_open: bool,
    focus_handle: FocusHandle,
    _sub: Subscription,
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppShell {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_prefs = state.read(cx).prefs.clone();
        apply_theme_from_prefs(&initial_prefs, cx);
        let s = &state;
        let _sub = cx.observe(s, |_, _, cx| cx.notify());
        Self {
            command_state: cx.new(|cx| CommandState::new(window, cx)),
            projects_view: cx.new(|cx| ProjectsView::new(s.clone(), window, cx)),
            groups_view: cx.new(|cx| GroupsView::new(s.clone(), window, cx)),
            overview_view: cx.new(|cx| ProjectOverviewView::new(s.clone(), window, cx)),
            repo_view: cx.new(|cx| RepositoryView::new(s.clone(), window, cx)),
            pipelines_view: cx.new(|cx| PipelinesView::new(s.clone(), window, cx)),
            mrs_view: cx.new(|cx| MergeRequestsView::new(s.clone(), window, cx)),
            branches_view: cx.new(|cx| BranchesView::new(s.clone(), window, cx)),
            tags_view: cx.new(|cx| TagsView::new(s.clone(), window, cx)),
            commits_view: cx.new(|cx| CommitsView::new(s.clone(), window, cx)),
            issues_view: cx.new(|cx| IssuesView::new(s.clone(), window, cx)),
            todos_view: cx.new(|cx| TodosView::new(s.clone(), window, cx)),
            releases_view: cx.new(|cx| ReleasesView::new(s.clone(), window, cx)),
            schedules_view: cx.new(|cx| SchedulesView::new(s.clone(), window, cx)),
            runners_view: cx.new(|cx| RunnersView::new(s.clone(), window, cx)),
            search_view: cx.new(|cx| SearchView::new(s.clone(), window, cx)),
            console_view: cx.new(|cx| ConsoleView::new(s.clone(), window, cx)),
            preferences_view: cx.new(|cx| PreferencesView::new(s.clone(), window, cx)),
            login_view: cx.new(|cx| LoginView::new(s.clone(), window, cx)),
            find_file_modal: cx.new(|cx| FindFileModal::new(s.clone(), window, cx)),
            palette_open: false,
            focus_handle: cx.focus_handle(),
            state,
            _sub,
        }
    }

    pub fn is_palette_open(&self) -> bool { self.palette_open }
    pub fn is_find_file_open(&self, cx: &gpui_kit::App) -> bool { self.find_file_modal.read(cx).is_open() }
    pub fn has_overlay_open(&self, cx: &gpui_kit::App) -> bool {
        self.palette_open || self.is_find_file_open(cx) || self.state.read(cx).update.modal_open
    }
    pub fn state(&self) -> &Entity<AppState> { &self.state }
    pub fn mrs_view(&self) -> &Entity<MergeRequestsView> { &self.mrs_view }
    pub fn pipelines_view(&self) -> &Entity<PipelinesView> { &self.pipelines_view }
    pub fn repo_view(&self) -> &Entity<RepositoryView> { &self.repo_view }
    pub fn overview_view(&self) -> &Entity<ProjectOverviewView> { &self.overview_view }
    pub fn groups_view(&self) -> &Entity<GroupsView> { &self.groups_view }
    pub fn commits_view(&self) -> &Entity<CommitsView> { &self.commits_view }
    pub fn branches_view(&self) -> &Entity<BranchesView> { &self.branches_view }
    pub fn tags_view(&self) -> &Entity<TagsView> { &self.tags_view }
    pub fn releases_view(&self) -> &Entity<ReleasesView> { &self.releases_view }
    pub fn todos_view(&self) -> &Entity<TodosView> { &self.todos_view }
    pub fn issues_view(&self) -> &Entity<IssuesView> { &self.issues_view }
    pub fn projects_view(&self) -> &Entity<ProjectsView> { &self.projects_view }
    pub fn schedules_view(&self) -> &Entity<SchedulesView> { &self.schedules_view }
    pub fn runners_view(&self) -> &Entity<RunnersView> { &self.runners_view }
    pub fn search_view(&self) -> &Entity<SearchView> { &self.search_view }
    pub fn console_view(&self) -> &Entity<ConsoleView> { &self.console_view }

    pub(crate) fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = !self.palette_open;
        if self.palette_open {
            let _ = self.find_file_modal.update(cx, |m, cx| m.close(cx));
            self.command_state.update(cx, |cmd, cx| cmd.set_query("", window, cx));
            self.command_state.focus_handle(cx).focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn close_palette_only(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.palette_open { return false; }
        self.palette_open = false;
        if !self.is_find_file_open(cx) { self.focus_handle.focus(window, cx); }
        cx.notify();
        true
    }

    pub(crate) fn dismiss_top_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.close_palette_only(window, cx) { return true; }
        if self.state.update(cx, |s, cx| s.close_update_modal(cx)) { self.focus_handle.focus(window, cx); return true; }
        if self.find_file_modal.update(cx, |m, cx| m.close(cx)) { self.focus_handle.focus(window, cx); return true; }
        if self.state.read(cx).auth.adding_account {
            self.state.update(cx, |s, cx| s.cancel_add_account(cx));
            self.focus_handle.focus(window, cx);
            return true;
        }
        false
    }

    pub(crate) fn open_find_file(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open { self.palette_open = false; cx.notify(); }
        self.state.update(cx, |s, cx| s.dispatch_command(super::state::ScreenCommand::OpenFindFile, cx));
    }

    fn render_screen_body(&self, cx: &Context<Self>) -> AnyElement {
        match self.state.read(cx).screen {
            Screen::Projects => self.projects_view.clone().into_any_element(),
            Screen::Groups => self.groups_view.clone().into_any_element(),
            Screen::ProjectOverview => self.overview_view.clone().into_any_element(),
            Screen::Repository => self.repo_view.clone().into_any_element(),
            Screen::Pipelines => self.pipelines_view.clone().into_any_element(),
            Screen::MergeRequests => self.mrs_view.clone().into_any_element(),
            Screen::Branches => self.branches_view.clone().into_any_element(),
            Screen::Tags => self.tags_view.clone().into_any_element(),
            Screen::Commits => self.commits_view.clone().into_any_element(),
            Screen::Issues => self.issues_view.clone().into_any_element(),
            Screen::Todos => self.todos_view.clone().into_any_element(),
            Screen::Releases => self.releases_view.clone().into_any_element(),
            Screen::Schedules => self.schedules_view.clone().into_any_element(),
            Screen::Runners => self.runners_view.clone().into_any_element(),
            Screen::Search => self.search_view.clone().into_any_element(),
            Screen::Console => self.console_view.clone().into_any_element(),
            Screen::Preferences => self.preferences_view.clone().into_any_element(),
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = cx.theme().is_dark();
        let panel_bg = cx.theme().background;
        let fg = cx.theme().foreground;
        let update_modal = self.state.read(cx).update.modal_open.then(|| {
            render_update_modal(&self.state, cx)
        });
        let root = div().id("glab-app-shell").track_focus(&self.focus_handle).relative()
            .size_full().flex().flex_col().bg(studio_shell_bg(dark)).text_color(fg);

        if !self.state.read(cx).is_authenticated() {
            return attach_shell_actions(root.bg(panel_bg), cx)
                .child(self.login_view.clone())
                .children(update_modal);
        }

        let sidebar = render_sidebar(&self.state, cx);
        let top_bar = render_top_bar(&self.state, window, cx);
        let panel_breadcrumb = render_panel_breadcrumb(&self.state, cx);
        let status_bar = render_status_bar(self.state.read(cx), cx);
        let shell_weak = cx.entity().downgrade();
        let palette = self.palette_open.then(|| {
            let weak = shell_weak.clone();
            render_palette_overlay(
                &self.command_state,
                &self.state,
                move |window, cx| {
                    let _ = weak.update(cx, |this, cx| this.close_palette_only(window, cx));
                },
                cx,
            )
        });

        let main_panel = div()
            .flex_1().min_w_0().min_h_0().pr_2().pb_1p5().flex().flex_col()
            .child(
                div()
                    .id("studio-main-panel")
                    .flex_1().min_w_0().min_h_0()
                    .rounded_lg().border_1().border_color(studio_panel_border(dark))
                    .bg(panel_bg).overflow_hidden().flex().flex_col()
                    .children(panel_breadcrumb)
                    .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().child(self.render_screen_body(cx)))
                    .test_support(),
            );

        attach_shell_actions(root, cx)
            .child(top_bar)
            .child(div().flex_1().min_h_0().flex().flex_row().child(sidebar).child(main_panel))
            .child(status_bar)
            .child(self.find_file_modal.clone())
            .children(update_modal)
            .children(palette)
    }
}
