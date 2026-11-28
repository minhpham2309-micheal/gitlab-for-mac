use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::Button,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, uniform_list, AppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::time_util::parse_iso_utc_secs;
use crate::gitlab::{Project, SortDir};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::state::{AppEvent, AppState, ScreenCommand};
use super::projects_row::render_project_row;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectTab { #[default] All, Starred }

impl ProjectTab {
    const ALL: [Self; 2] = [Self::All, Self::Starred];
    fn label(self) -> &'static str { match self { Self::All => "All", Self::Starred => "Starred" } }
    fn key(self) -> &'static str { match self { Self::All => "all", Self::Starred => "starred" } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSort { #[default] Updated, Name, Stars }

impl ProjectSort {
    const ALL: [Self; 3] = [Self::Updated, Self::Name, Self::Stars];
    fn label(self) -> &'static str {
        match self { Self::Updated => "Updated", Self::Name => "Name", Self::Stars => "Stars" }
    }
}

pub struct ProjectsView {
    state: Entity<AppState>,
    search_input: Entity<InputState>,
    tab: ProjectTab,
    sort: ProjectSort,
    sort_dir: SortDir,
    search_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ProjectsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Filter projects locally, or press Enter to search GitLab…")
        });
        let sub_input = cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| match ev {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => this.remote_search(cx),
            _ => {}
        });
        let sub_state = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| match ev {
            AppEvent::HostChanged { .. } => {
                (this.search_task, this.tab) = (None, ProjectTab::All);
                this.search_input.update(cx, |input, cx| input.set_value("", window, cx));
                cx.notify();
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Projects => {
                this.state.update(cx, |s, cx| s.refresh_host_data(cx));
            }
            _ => {}
        });
        let obs_state = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state, search_input, tab: ProjectTab::All, sort: ProjectSort::Updated, sort_dir: SortDir::Desc,
            search_task: None, _subs: vec![sub_input, sub_state, obs_state],
        }
    }

    pub fn tab(&self) -> ProjectTab { self.tab }
    pub fn set_tab(&mut self, tab: ProjectTab, cx: &mut Context<Self>) { if self.tab != tab { self.tab = tab; cx.notify(); } }
    pub fn set_sort(&mut self, sort: ProjectSort, cx: &mut Context<Self>) { if self.sort != sort { self.sort = sort; cx.notify(); } }
    pub fn toggle_sort_dir(&mut self, cx: &mut Context<Self>) { self.sort_dir = self.sort_dir.toggle(); cx.notify(); }

    fn remote_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().trim().to_string();
        if query.len() < 2 {
            self.state.update(cx, |s, cx| s.refresh_host_data(cx));
            return;
        }
        let st = self.state.read(cx);
        let Some(client) = st.gitlab.clone() else { return; };
        let epoch = st.host_epoch;
        self.state.update(cx, |s, cx| { s.busy = true; s.set_status(format!("Searching GitLab for '{query}'…"), cx); });
        self.search_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.search_projects(&query).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |view, cx| {
                view.state.update(cx, |s, cx| {
                    if s.host_epoch != epoch { return; }
                    s.busy = false;
                    match res {
                        Ok(Ok(projects)) => { let n = projects.len(); s.projects = projects; s.set_status(format!("Found {n} projects"), cx); }
                        Ok(Err(err)) | Err(err) => s.set_status(err, cx),
                    }
                });
            });
        }));
    }

    fn filtered(&self, cx: &Context<Self>) -> Vec<Project> {
        let q = self.search_input.read(cx).value().trim().to_ascii_lowercase();
        let mut out: Vec<Project> = self.state.read(cx).projects.iter().filter(|p| {
            (self.tab == ProjectTab::All || p.star_count > 0)
                && (q.is_empty() || p.path_with_namespace.to_ascii_lowercase().contains(&q) || p.name.to_ascii_lowercase().contains(&q))
        }).cloned().collect();
        out.sort_by(|a, b| {
            let ord = match self.sort {
                ProjectSort::Updated => {
                    let ta = a.last_activity_at.as_deref().and_then(parse_iso_utc_secs).unwrap_or(i64::MIN);
                    let tb = b.last_activity_at.as_deref().and_then(parse_iso_utc_secs).unwrap_or(i64::MIN);
                    ta.cmp(&tb).then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
                }
                ProjectSort::Name => a.path_with_namespace.to_ascii_lowercase().cmp(&b.path_with_namespace.to_ascii_lowercase()),
                ProjectSort::Stars => a.star_count.cmp(&b.star_count).then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase())),
            };
            match self.sort_dir { SortDir::Asc => ord, SortDir::Desc => ord.reverse() }
        });
        out
    }
}

impl Render for ProjectsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let projects = self.filtered(cx);
        let (active_id, all_count, starred_count, client_for_av) = {
            let st = self.state.read(cx);
            (st.project.as_ref().map(|p| p.id), st.projects.len(), st.projects.iter().filter(|p| p.star_count > 0).count(), st.gitlab.clone())
        };
        let (total, cur_tab, cur_sort) = (projects.len(), self.tab, self.sort);
        let sort_icon = match self.sort_dir { SortDir::Desc => IconName::SortDescending, SortDir::Asc => IconName::SortAscending };
        let (state_reload, state_click, state_groups, weak) = (self.state.clone(), self.state.clone(), self.state.clone(), cx.entity().downgrade());
        let theme = cx.theme();
        let dark = theme.is_dark();
        let (inact_bg, inact_fg): (gpui_kit::Hsla, gpui_kit::Hsla) = if dark { (rgb(0x383839).into(), rgb(0xBFBFBF).into()) } else { (theme.secondary, theme.muted_foreground) };

        let tabs = div().flex().items_center().gap_1().children(ProjectTab::ALL.into_iter().map(|t| {
            let active = cur_tab == t;
            let cnt = match t { ProjectTab::All => all_count, ProjectTab::Starred => starred_count };
            let (bbg, bfg) = if active { (theme.primary, theme.primary_foreground) } else { (inact_bg, inact_fg) };
            div().id(SharedString::from(format!("projects-tab-{}", t.key()))).px_3().py_2p5().cursor_pointer()
                .border_b_2().border_color(if active { theme.primary } else { gpui_kit::transparent_black() })
                .flex().items_center().gap_1p5().text_sm()
                .text_color(if active { theme.foreground } else { theme.muted_foreground }).hover(|s| s.text_color(theme.foreground))
                .on_click(cx.listener(move |v, _, _, cx| v.set_tab(t, cx)))
                .child(div().when(active, |d| d.font_semibold()).child(t.label()))
                .child(div().px_1p5().py(px(1.)).rounded_full().bg(bbg).text_color(bfg).text_xs().font_semibold().child(cnt.to_string()))
                .test_support()
        }));

        let list = if total == 0 {
            div().id("projects-empty").flex_1().py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child("No projects match your current filter.").test_support().into_any_element()
        } else {
            uniform_list("projects-uniform-list", total, move |range, _, cx| {
                range.map(|ix| {
                    let project = projects[ix].clone();
                    let is_active = active_id == Some(project.id);
                    render_project_row(project, is_active, client_for_av.as_ref(), state_click.clone(), cx)
                }).collect()
            }).flex_1().size_full().into_any_element()
        };

        div().id("projects-screen").size_full().flex().flex_col().overflow_hidden()
            .child(
                div().w_full().px_6().pt_3().flex_shrink_0().flex().flex_col().items_center().child(
                    div().w_full().max_w(px(1200.)).flex().flex_col()
                        .child(div().pb_2().flex().items_center().justify_between().gap_4()
                            .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Projects"))
                            .child(div().flex().items_center().gap_1p5()
                                .child(Button::new("projects-open-groups").outline().small().label("Explore groups")
                                    .on_click(move |_, _, cx| state_groups.update(cx, |s, cx| s.open_group(None, cx))))
                                .child(Button::new("projects-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5()).label("Reload")
                                    .on_click(move |_, _, cx| state_reload.update(cx, |s, cx| s.refresh_host_data(cx))))))
                        .child(div().w_full().border_b_1().border_color(theme.border).flex().items_end().child(tabs))
                        .child(div().mt_3().mb_2().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border).flex().items_center().gap_2()
                            .child(div().flex_1().min_w_0().h(px(32.)).rounded_md().bg(theme.background).border_1().border_color(theme.input).px_2().flex().items_center().gap_1p5()
                                .child(div().flex_1().min_w_0().child(Input::new(&self.search_input).appearance(false).cleanable(true).small()))
                                .child(Icon::new(IconName::Search).size_3p5().text_color(theme.muted_foreground)))
                            .child(div().flex().items_center().flex_shrink_0()
                                .child(Button::new("projects-sort-field").outline().small().label(format!("{} ▾", cur_sort.label()))
                                    .dropdown_menu(move |mut menu, _, _| {
                                        for f in ProjectSort::ALL {
                                            let w = weak.clone();
                                            menu = menu.item(PopupMenuItem::new(f.label()).checked(cur_sort == f).on_click(move |_, _, cx| {
                                                let _ = w.update(cx, |v, cx| v.set_sort(f, cx));
                                            }));
                                        }
                                        menu
                                    }))
                                .child(Button::new("projects-sort-dir").outline().small().icon(Icon::new(sort_icon).size_3p5())
                                    .on_click(cx.listener(|v, _, _, cx| v.toggle_sort_dir(cx)))))),
                ),
            )
            .child(div().flex_1().min_h_0().w_full().flex().flex_col().child(list))
            .test_support()
    }
}
