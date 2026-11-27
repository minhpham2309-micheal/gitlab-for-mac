use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _}, diff::{DiffMode, DiffState},
        input::{Input, InputEvent, InputState}, resizable::{h_resizable, resizable_panel},
        ActiveTheme as _, Icon, Sizable as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{RawDiff, RepoCommit};
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
    pub(super) diff_error: Option<String>,
    pub(super) fallback_notices: Vec<(String, String)>,
    pub(super) has_truncated: bool,
    pub(super) raw_diff_count: usize,
    list_task: Option<Task<()>>,
    diff_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl CommitsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = if state.read(cx).prefs.default_diff_split { DiffMode::Split } else { DiffMode::Unified };
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter commits…"));
        let diff_state = cx.new(|cx| new_diff_state(Vec::new(), mode, cx));
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
                this.clear_diff(cx);
                this.load_page(1, false, cx);
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Commits => { this.load_page(1, false, cx); }
            AppEvent::PrefsChanged => {
                let m = if this.state.read(cx).prefs.default_diff_split { DiffMode::Split } else { DiffMode::Unified };
                this.diff_state.update(cx, |ds, cx| ds.set_mode(m, cx));
            }
            _ => {}
        });
        Self {
            state, filter_input, diff_state, git_ref: "main".into(), path_filter: None,
            commits: Vec::new(), selected_sha: None, page: 1, loading: false, diff_loading: false,
            diff_error: None, fallback_notices: Vec::new(), has_truncated: false, raw_diff_count: 0,
            list_task: None, diff_task: None, _subs: vec![sub_in, sub_st],
        }
    }

    pub(super) fn selected_commit(&self) -> Option<&RepoCommit> {
        self.selected_sha.as_deref().and_then(|sha| self.commits.iter().find(|c| c.id == sha))
    }

    pub fn diff_file_count(&self, cx: &gpui_kit::App) -> usize { self.diff_state.read(cx).files().len() }

    pub fn set_fixture_data(
        &mut self, commits: Vec<RepoCommit>, selected_sha: Option<String>, raw_diffs: Vec<RawDiff>,
        diff_error: Option<String>, cx: &mut Context<Self>,
    ) {
        (self.commits, self.selected_sha, self.loading, self.diff_loading, self.diff_error) =
            (commits, selected_sha, false, false, diff_error);
        self.apply_raw_diffs(&raw_diffs, cx);
        cx.notify();
    }

    fn clear_diff(&mut self, cx: &mut Context<Self>) {
        (self.diff_task, self.diff_loading, self.diff_error, self.has_truncated, self.raw_diff_count) =
            (None, false, None, false, 0);
        self.fallback_notices.clear();
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
    }

    fn apply_raw_diffs(&mut self, raw: &[RawDiff], cx: &mut Context<Self>) {
        let parsed = parse_raw_diffs_individually(raw);
        (self.raw_diff_count, self.fallback_notices, self.has_truncated) =
            (raw.len(), parsed.fallback_notices, parsed.has_truncated);
        self.diff_state.update(cx, |ds, cx| ds.set_files(parsed.files, cx));
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        (self.list_task, self.path_filter, self.selected_sha, self.page, self.loading) = (None, None, None, 1, false);
        self.git_ref = self.state.read(cx).project.as_ref()
            .and_then(|p| p.default_branch.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| "main".into());
        self.commits.clear();
        self.clear_diff(cx);
        cx.notify();
    }

    fn load_page(&mut self, page: usize, append: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let ((h_ep, p_ep), (git_ref, path)) = ((st.host_epoch, st.project_epoch), (self.git_ref.clone(), self.path_filter.clone()));
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
                        if v.selected_sha.as_deref().is_none_or(|sha| !v.commits.iter().any(|c| c.id == sha)) {
                            if let Some(first) = v.commits.first().map(|c| c.id.clone()) { v.select_commit(first, cx); }
                            else { v.selected_sha = None; v.clear_diff(cx); }
                        }
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn select_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        self.selected_sha = Some(sha.clone());
        self.clear_diff(cx);
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
                    Ok(Ok(raw)) => v.apply_raw_diffs(&raw, cx),
                    Ok(Err(err)) | Err(err) => {
                        v.diff_error = Some(err.clone());
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                    }
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
        let row_h = px((f32::from(theme.font_size) * 3.75).ceil().max(60.));
        let list_body = if rows.is_empty() {
            div().flex_1().flex().items_center().justify_center().text_xs().text_color(theme.muted_foreground)
                .child(if self.loading { "Loading commits…" } else { "No commits found" }).into_any_element()
        } else {
            uniform_list("commits-uniform-list", rows.len(), cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
                range.map(|ix| {
                    let c = &rows[ix];
                    let (is_sel, sha_click, theme) = (sel.as_deref() == Some(c.id.as_str()), c.id.clone(), cx.theme());
                    let rel = c.timestamp().map(format_relative).unwrap_or_default();
                    let meta_left = if rel.is_empty() { c.author_name.clone() } else { format!("{} · {rel}", c.author_name) };
                    div().id(SharedString::from(format!("commit-row-{ix}")))
                        .w_full().h(row_h).px_3().cursor_pointer().border_b_1().border_color(theme.border.opacity(0.45))
                        .bg(if is_sel { theme.list_active } else { gpui_kit::transparent_black() }).hover(|s| s.bg(theme.list_hover))
                        .flex().flex_col().justify_center().gap_1()
                        .on_click(cx.listener(move |v, _, _, cx| v.select_commit(sha_click.clone(), cx)))
                        .child(div().id(SharedString::from(format!("commit-title-{ix}"))).w_full().flex_shrink_0()
                            .text_xs().font_medium().text_color(theme.foreground).truncate().child(c.title.clone()).test_support())
                        .child(div().id(SharedString::from(format!("commit-meta-{ix}"))).w_full().flex_shrink_0()
                            .flex().items_center().justify_between().gap_2().text_xs().text_color(theme.muted_foreground)
                            .child(div().id(SharedString::from(format!("commit-author-{ix}"))).flex_1().min_w_0().truncate().child(meta_left).test_support())
                            .child(div().id(SharedString::from(format!("commit-sha-{ix}"))).flex_shrink_0()
                                .font_family(theme.mono_font_family.clone()).child(c.display_short_id().to_string()).test_support())
                            .test_support())
                        .test_support().into_any_element()
                }).collect()
            })).flex_1().size_full().into_any_element()
        };

        let next_page = self.page + 1;
        let left = div().size_full().flex().flex_col().border_r_1().border_color(theme.border)
            .child(div().p_2p5().border_b_1().border_color(theme.border).flex().flex_col().gap_1p5()
                .child(div().w_full().flex().items_center().justify_between().gap_1()
                    .child(div().flex_1().min_w_0().flex().items_center().gap_1().text_xs().text_color(theme.foreground)
                        .child(Icon::empty().path("icons/glab-branch.svg").size_3p5().text_color(theme.muted_foreground))
                        .child(div().min_w_0().truncate().child(self.git_ref.clone())))
                    .children(self.path_filter.clone().map(|p| {
                        Button::new("commits-clear-path").outline().xsmall().label(format!("{p} ×"))
                            .on_click(cx.listener(|v, _, _, cx| { (v.path_filter, v.selected_sha) = (None, None); v.clear_diff(cx); v.load_page(1, false, cx); }))
                    })))
                .child(Input::new(&self.filter_input).xsmall()))
            .child(div().flex_1().min_h_0().w_full().overflow_hidden().flex().flex_col().child(list_body))
            .child(div().p_2().border_t_1().border_color(theme.border).flex().justify_center().child(
                Button::new("commits-load-more").ghost().xsmall().label("Load more")
                    .on_click(cx.listener(move |v, _, _, cx| v.load_page(next_page, true, cx))),
            ));

        h_resizable("commits-split")
            .child(resizable_panel().size(px(360.)).size_range(px(240.)..px(540.)).child(left))
            .child(resizable_panel().child(render_commit_diff_pane(self, cx)))
    }
}
