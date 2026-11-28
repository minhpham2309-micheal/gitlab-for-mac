use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::Button,
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, AppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::time_util::now_unix_secs;
use crate::gitlab::{split_active_stale_branches, Branch, BranchScope, MergeRequest, Pipeline};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};
use super::branches_rows::{render_branch_section, BranchSectionSpec};

const OVERVIEW_PREVIEW_LIMIT: usize = 5;

pub struct BranchesView {
    pub(super) state: Entity<AppState>,
    filter: Entity<InputState>,
    scope: BranchScope,
    branches: Vec<Branch>,
    pipelines: Vec<Pipeline>,
    mrs: Vec<MergeRequest>,
    loading: bool,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl BranchesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by branch name"));
        let sub_in = cx.subscribe_in(&filter, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.loading = false;
                this.scope = BranchScope::Overview;
                this.branches.clear();
                this.pipelines.clear();
                this.mrs.clear();
                if this.state.read(cx).screen == Screen::Branches { this.load(cx); }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Branches => {
                if this.branches.is_empty() && !this.loading { this.load(cx); }
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Branches => { this.load(cx); }
            _ => {}
        });
        Self {
            state, filter, scope: BranchScope::Overview,
            branches: Vec::new(), pipelines: Vec::new(), mrs: Vec::new(),
            loading: false, task: None, _subs: vec![sub_in, sub_st],
        }
    }

    pub fn scope(&self) -> BranchScope { self.scope }

    pub fn set_scope(&mut self, scope: BranchScope, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.scope = scope;
            cx.notify();
        }
    }

    pub fn set_fixture_data(
        &mut self, branches: Vec<Branch>, pipelines: Vec<Pipeline>, mrs: Vec<MergeRequest>, cx: &mut Context<Self>,
    ) {
        (self.branches, self.pipelines, self.mrs, self.loading) = (branches, pipelines, mrs, false);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.loading = true;
        cx.notify();
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (branches, pipes, mrs) = tokio::join!(
                    client.branches(project.id),
                    client.pipelines(project.id),
                    client.merge_requests(project.id, "all"),
                );
                branches.map(|b| (b, pipes.unwrap_or_default(), mrs.unwrap_or_default())).map_err(|e| e.to_string())
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok((branches, pipes, mrs))) => {
                        v.branches = branches;
                        v.pipelines = pipes;
                        v.mrs = mrs;
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for BranchesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.filter.read(cx).value().to_string();
        let (active, stale) = split_active_stale_branches(&self.branches, &query, now_unix_secs());
        let project_web = self.state.read(cx).project.as_ref().map(|p| p.web_url.clone()).filter(|u| !u.is_empty());
        let project_id = self.state.read(cx).project.as_ref().map(|p| p.id).unwrap_or(0);
        let (border, head_bg, bg, input_b, fg, muted, active_tab_border) = {
            let theme = cx.theme();
            (theme.border, theme.list_head, theme.background, theme.input, theme.foreground, theme.muted_foreground, theme.primary)
        };

        let tabs = div().flex().items_center().gap_1().children(BranchScope::ALL.into_iter().map(|sc| {
            let is_active = self.scope == sc;
            div()
                .id(SharedString::from(format!("branch-tab-{}", sc.label())))
                .px_3().py_2p5().cursor_pointer()
                .border_b_2().border_color(if is_active { active_tab_border } else { gpui_kit::transparent_black() })
                .text_sm()
                .text_color(if is_active { fg } else { muted })
                .hover(move |s| s.text_color(fg))
                .on_click(cx.listener(move |v, _, _, cx| v.set_scope(sc, cx)))
                .child(div().when(is_active, |d| d.font_semibold()).child(sc.label()))
                .test_support()
        }));

        let actions = div().flex().items_center().gap_1p5().pb_1p5()
            .child(Button::new("branches-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                .on_click(cx.listener(|v, _, _, cx| v.load(cx))))
            .children(project_web.map(|u| {
                let url = format!("{}/-/branches", u.trim_end_matches('/'));
                Button::new("branches-open-web").outline().small().icon(Icon::new(IconName::ExternalLink).size_3p5()).label("Web")
                    .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
            }));

        let filter_bar = div()
            .id("branches-filter-bar").mt_3().mb_4().p_2().rounded_md().bg(head_bg).border_1().border_color(border)
            .child(
                div().h(px(32.)).rounded_md().bg(bg).border_1().border_color(input_b).px_2().flex().items_center().gap_1p5()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.filter).appearance(false).cleanable(true).small()))
                    .child(Icon::new(IconName::Search).size_3p5().text_color(muted)),
            )
            .test_support();

        let body = if self.loading && self.branches.is_empty() {
            div().py_12().flex().items_center().justify_center().text_sm().text_color(muted)
                .child("Loading branches…").into_any_element()
        } else {
            let mut sections = Vec::new();
            match self.scope {
                BranchScope::Overview => {
                    let more_active = (active.len() > OVERVIEW_PREVIEW_LIMIT).then_some(("Show more active branches", BranchScope::Active));
                    sections.push(render_branch_section(
                        BranchSectionSpec { key: "active", title: "Active branches", total: active.len(), limit: Some(OVERVIEW_PREVIEW_LIMIT), show_more: more_active },
                        &active, &self.pipelines, &self.mrs, project_id, &self.state, cx,
                    ));
                    if !stale.is_empty() {
                        let more_stale = (stale.len() > OVERVIEW_PREVIEW_LIMIT).then_some(("Show more stale branches", BranchScope::Stale));
                        sections.push(render_branch_section(
                            BranchSectionSpec { key: "stale", title: "Stale branches", total: stale.len(), limit: Some(OVERVIEW_PREVIEW_LIMIT), show_more: more_stale },
                            &stale, &self.pipelines, &self.mrs, project_id, &self.state, cx,
                        ));
                    }
                }
                BranchScope::Active => sections.push(render_branch_section(
                    BranchSectionSpec { key: "active", title: "Active branches", total: active.len(), limit: None, show_more: None },
                    &active, &self.pipelines, &self.mrs, project_id, &self.state, cx,
                )),
                BranchScope::Stale => sections.push(render_branch_section(
                    BranchSectionSpec { key: "stale", title: "Stale branches", total: stale.len(), limit: None, show_more: None },
                    &stale, &self.pipelines, &self.mrs, project_id, &self.state, cx,
                )),
                BranchScope::All => {
                    let mut all = active;
                    all.extend(stale);
                    sections.push(render_branch_section(
                        BranchSectionSpec { key: "all", title: "All branches", total: all.len(), limit: None, show_more: None },
                        &all, &self.pipelines, &self.mrs, project_id, &self.state, cx,
                    ));
                }
            }
            div().w_full().flex().flex_col().gap_4().pb_6().children(sections).into_any_element()
        };

        div()
            .id("branches-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(
                div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                    .child(div().w_full().border_b_1().border_color(border).flex().items_end().justify_between().gap_4().child(tabs).child(actions))
                    .child(filter_bar)
                    .child(div().id("branches-scroll").flex_1().min_h_0().overflow_y_scroll().pr_1().child(body)),
            )
            .test_support()
    }
}
