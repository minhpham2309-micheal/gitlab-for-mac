use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _}, diff::{DiffMode, DiffState},
        input::{Input, InputEvent, InputState}, resizable::{h_resizable, resizable_panel},
        ActiveTheme as _, Icon, Sizable as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, Window,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::RepoCommit;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::diff_hl::{new_diff_state, parse_raw_diffs_individually};
use super::super::super::state::{AppEvent, AppState, ScreenCommand};
use super::commit_diff::render_commit_diff_pane;

pub struct CommitsView {
    pub(super) state: Entity<AppState>,
    filter_input: Entity<InputState>,
    pub(super) diff_state: Entity<DiffState>,
    git_ref: String,
    path_filter: Option<String>,
    commits: Vec<RepoCommit>,
    selected_sha: Option<String>,
    page: usize,
    loading: bool,
    pub(super) diff_loading: bool,
    list_task: Option<Task<()>>,
    diff_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl CommitsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter commits…"));
        let diff_state = cx.new(|cx| new_diff_state(Vec::new(), DiffMode::Unified, cx));
        let sub_in = cx.subscribe_in(&filter_input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.reset(cx);
                if this.state.read(cx).screen == Screen::Commits { this.load_page(1, false, cx); }
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Commits => {
                if this.commits.is_empty() && !this.loading { this.load_page(1, false, cx); }
            }
            AppEvent::Command(ScreenCommand::OpenCommitHistory { git_ref, path }) => {
                if let Some(r) = git_ref.clone().filter(|s| !s.is_empty()) { this.git_ref = r; }
                this.path_filter = path.clone().filter(|s| !s.is_empty());
                this.selected_sha = None;
                this.load_page(1, false, cx);
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Commits => { this.load_page(1, false, cx); }
            _ => {}
        });
        Self {
            state, filter_input, diff_state, git_ref: "main".into(), path_filter: None,
            commits: Vec::new(), selected_sha: None, page: 1, loading: false, diff_loading: false,
            list_task: None, diff_task: None, _subs: vec![sub_in, sub_st],
        }
    }

    pub(super) fn selected_commit(&self) -> Option<&RepoCommit> {
        self.selected_sha.as_deref().and_then(|sha| self.commits.iter().find(|c| c.id == sha))
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        (self.list_task, self.diff_task, self.path_filter, self.selected_sha) = (None, None, None, None);
        self.git_ref = self.state.read(cx).project.as_ref()
            .and_then(|p| p.default_branch.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| "main".into());
        self.commits.clear();
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
        (self.page, self.loading, self.diff_loading) = (1, false, false);
        cx.notify();
    }

    fn load_page(&mut self, page: usize, append: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let (git_ref, path) = (self.git_ref.clone(), self.path_filter.clone());
        self.loading = true;
        cx.notify();
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.repo_commits(project.id, &git_ref, path.as_deref(), page, 40).await.map_err(|e| e.to_string())
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(items)) => {
                        if !items.is_empty() || !append { v.page = page; }
                        if append { v.commits.extend(items); } else { v.commits = items; }
                        if v.selected_sha.is_none() {
                            if let Some(first) = v.commits.first().map(|c| c.id.clone()) { v.select_commit(first, cx); }
                        }
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    fn select_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        self.selected_sha = Some(sha.clone());
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
        let st = self.state.read(cx);
        if st.offline_mode { cx.notify(); return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.diff_loading = true;
        cx.notify();
        self.diff_task = Some(cx.spawn(async move |this, cx| {
            let sha_req = sha.clone();
            let res = spawn_tokio(async move { client.repo_commit_diff(project.id, &sha_req).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.selected_sha.as_deref() != Some(sha.as_str()) { return; }
                v.diff_loading = false;
                match res {
                    Ok(Ok(raw)) => v.diff_state.update(cx, |ds, cx| ds.set_files(parse_raw_diffs_individually(&raw).files, cx)),
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for CommitsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let q = self.filter_input.read(cx).value().trim().to_ascii_lowercase();
        let rows: Vec<RepoCommit> = self.commits.iter().filter(|c| {
            q.is_empty() || c.title.to_ascii_lowercase().contains(&q)
                || c.author_name.to_ascii_lowercase().contains(&q) || c.id.starts_with(&q)
        }).cloned().collect();
        let sel = self.selected_sha.clone();
        let list = uniform_list("commits-uniform-list", rows.len(), cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
            range.map(|ix| {
                let c = &rows[ix];
                let (is_sel, sha_click, rel, theme) = (sel.as_deref() == Some(c.id.as_str()), c.id.clone(), c.timestamp().map(format_relative).unwrap_or_default(), cx.theme());
                div().id(SharedString::from(format!("commit-row-{ix}")))
                    .h(px(56.)).px_3().py_2().cursor_pointer().border_b_1().border_color(theme.border.opacity(0.45))
                    .bg(if is_sel { theme.list_active } else { gpui_kit::transparent_black() }).hover(|s| s.bg(theme.list_hover))
                    .flex().flex_col().justify_center().gap_0p5()
                    .on_click(cx.listener(move |v, _, _, cx| v.select_commit(sha_click.clone(), cx)))
                    .child(div().text_xs().text_color(theme.foreground).truncate().child(c.title.clone()))
                    .child(div().flex().items_center().justify_between().gap_2().text_xs().text_color(theme.muted_foreground)
                        .child(div().truncate().child(format!("{} · {rel}", c.author_name)))
                        .child(div().font_family(theme.mono_font_family.clone()).child(c.display_short_id().to_string())))
                    .into_any_element()
            }).collect()
        })).flex_1().size_full();

        let next_page = self.page + 1;
        let left = div().size_full().flex().flex_col().border_r_1().border_color(theme.border)
            .child(div().p_2p5().border_b_1().border_color(theme.border).flex().flex_col().gap_1p5()
                .child(div().flex().items_center().justify_between().gap_1()
                    .child(div().flex().items_center().gap_1().text_xs().text_color(theme.foreground)
                        .child(Icon::empty().path("icons/glab-branch.svg").size_3p5().text_color(theme.muted_foreground))
                        .child(self.git_ref.clone()))
                    .children(self.path_filter.clone().map(|p| {
                        Button::new("commits-clear-path").outline().xsmall().label(format!("{p} ×"))
                            .on_click(cx.listener(|v, _, _, cx| { (v.path_filter, v.selected_sha) = (None, None); v.load_page(1, false, cx); }))
                    })))
                .child(Input::new(&self.filter_input).xsmall()))
            .child(div().flex_1().min_h_0().child(list))
            .child(div().p_2().border_t_1().border_color(theme.border).flex().justify_center().child(
                Button::new("commits-load-more").ghost().xsmall().label("Load more")
                    .on_click(cx.listener(move |v, _, _, cx| v.load_page(next_page, true, cx))),
            ));

        h_resizable("commits-split")
            .child(resizable_panel().size(px(360.)).size_range(px(240.)..px(540.)).child(left))
            .child(resizable_panel().child(render_commit_diff_pane(self, cx)))
    }
}
