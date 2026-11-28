mod header;
mod loaders;
mod rows;

use std::collections::{HashMap, HashSet};

use gpui_kit::{
    component::{
        input::{InputEvent, InputState},
        ActiveTheme as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::{
    build_group_tree_rows, Group, GroupChildren, GroupSortField, GroupTab, Project, SortDir,
};
use crate::nav::Screen;

use super::super::state::{AppEvent, AppState, ScreenCommand};
use self::header::{render_groups_tabs_and_toolbar, render_groups_top_header};
use self::rows::render_group_tree_row;

pub struct GroupsView {
    pub(super) state: Entity<AppState>,
    pub(super) search_input: Entity<InputState>,
    pub(super) active_group: Option<Group>,
    pub(super) pending_path: Option<String>,
    pub(super) tab: GroupTab,
    pub(super) sort_field: GroupSortField,
    pub(super) sort_dir: SortDir,
    pub(super) subgroups: Vec<Group>,
    pub(super) projects: Vec<Project>,
    pub(super) children_cache: HashMap<u64, GroupChildren>,
    pub(super) expanded: HashSet<u64>,
    pub(super) loading_children: HashSet<u64>,
    pub(super) loading: bool,
    pub(super) error: Option<String>,
    pub(super) list_task: Option<Task<()>>,
    pub(super) child_tasks: HashMap<u64, Task<()>>,
    _subs: Vec<Subscription>,
}

impl GroupsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Filter subgroups and projects, or press Enter to search GitLab…")
        });
        let sub_in = cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| {
            match ev {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.submit_search(cx),
                _ => {}
            }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| match ev {
            AppEvent::HostChanged { .. } => {
                this.reset_all(window, cx);
                if this.state.read(cx).screen == Screen::Groups {
                    this.load_root_groups(None, cx);
                }
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Groups => {
                if let Some(path) = this.pending_path.take() {
                    this.open_group_path(Some(path), cx);
                } else if this.active_group.is_none() && this.subgroups.is_empty() && !this.loading {
                    let target = this.state.read(cx).active_group_path.clone();
                    this.open_group_path(target, cx);
                }
            }
            AppEvent::Command(ScreenCommand::OpenGroup { full_path }) => {
                this.open_group_path(full_path.clone(), cx);
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Groups =>
            {
                this.reload(cx);
            }
            _ => {}
        });
        Self {
            state,
            search_input,
            active_group: None,
            pending_path: None,
            tab: GroupTab::SubgroupsAndProjects,
            sort_field: GroupSortField::Name,
            sort_dir: SortDir::Asc,
            subgroups: Vec::new(),
            projects: Vec::new(),
            children_cache: HashMap::new(),
            expanded: HashSet::new(),
            loading_children: HashSet::new(),
            loading: false,
            error: None,
            list_task: None,
            child_tasks: HashMap::new(),
            _subs: vec![sub_in, sub_st],
        }
    }

    pub fn active_group(&self) -> Option<&Group> { self.active_group.as_ref() }
    pub fn tab(&self) -> GroupTab { self.tab }
    pub fn is_expanded(&self, group_id: u64) -> bool { self.expanded.contains(&group_id) }

    pub fn set_fixture_data(
        &mut self,
        active_group: Option<Group>,
        subgroups: Vec<Group>,
        projects: Vec<Project>,
        children_cache: HashMap<u64, GroupChildren>,
        cx: &mut Context<Self>,
    ) {
        self.active_group = active_group.clone();
        self.subgroups = subgroups;
        self.projects = projects;
        self.children_cache = children_cache;
        self.loading = false;
        self.error = None;
        self.state.update(cx, |s, _| {
            s.active_group_path = active_group.as_ref().map(|g| g.full_path.clone());
            if let Some(g) = &active_group && !g.web_url.is_empty() {
                s.summary.active_web_url = Some(g.web_url.clone());
            }
        });
        cx.notify();
    }

    pub(super) fn set_tab(&mut self, tab: GroupTab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            self.expanded.clear();
            self.load_active_group_contents(None, cx);
        }
    }

    pub(super) fn set_sort_field(&mut self, field: GroupSortField, cx: &mut Context<Self>) {
        if self.sort_field != field {
            self.sort_field = field;
            cx.notify();
        }
    }

    pub(super) fn toggle_sort_dir(&mut self, cx: &mut Context<Self>) {
        self.sort_dir = self.sort_dir.toggle();
        cx.notify();
    }

    pub(super) fn submit_search(&mut self, cx: &mut Context<Self>) {
        let q = self.search_input.read(cx).value().trim().to_string();
        let query = (!q.is_empty()).then_some(q);
        if self.active_group.is_some() {
            self.load_active_group_contents(query, cx);
        } else {
            self.load_root_groups(query, cx);
        }
    }

    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        self.children_cache.clear();
        self.loading_children.clear();
        if self.active_group.is_some() {
            self.load_active_group_contents(None, cx);
        } else {
            self.load_root_groups(None, cx);
        }
    }
}

impl Render for GroupsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let client = self.state.read(cx).gitlab.clone();
        let query = self.search_input.read(cx).value().to_string();
        let rows = build_group_tree_rows(
            &self.subgroups, &self.projects, &self.children_cache, &self.expanded,
            &self.loading_children, &query, self.sort_field, self.sort_dir,
        );

        let top_header = render_groups_top_header(self, client.as_ref(), cx);
        let tabs_and_toolbar = render_groups_tabs_and_toolbar(self, cx);
        let theme = cx.theme();

        let view_weak = cx.entity().downgrade();
        let app_state = self.state.clone();
        let body = if let Some(err) = self.error.clone() {
            div().py_12().flex().items_center().justify_center().text_sm().text_color(theme.danger).child(err).into_any_element()
        } else if self.loading && rows.is_empty() {
            div().py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child("Loading groups and projects…").into_any_element()
        } else if rows.is_empty() {
            div().id("groups-empty-state").py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child("No subgroups or projects match your current filter.").test_support().into_any_element()
        } else {
            uniform_list("groups-uniform-list", rows.len(), move |range, _, cx| {
                range
                    .map(|ix| render_group_tree_row(&rows[ix], client.as_ref(), view_weak.clone(), app_state.clone(), cx))
                    .collect()
            })
            .flex_1()
            .size_full()
            .into_any_element()
        };

        div()
            .id("groups-screen")
            .size_full().flex().flex_col()
            .child(
                div().w_full().px_6().pt_5().pb_2().flex_shrink_0().flex().flex_col().items_center()
                    .child(div().w_full().max_w(px(1120.)).flex().flex_col().child(top_header).child(tabs_and_toolbar)),
            )
            .child(div().flex_1().min_h_0().w_full().flex().flex_col().child(body))
            .test_support()
    }
}
