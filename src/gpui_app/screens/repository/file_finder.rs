use gpui_kit::{
    component::{
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window,
};

use crate::gitlab::RepoTreeEntry;
use crate::runtime::spawn_tokio;

use super::super::overview::file_icons::render_entry_icon;
use super::super::super::repo_cache::RepoCache;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};
use super::fuzzy::fuzzy_score;

const MAX_FIND_FILES: usize = 2_000;

pub struct FindFileModal {
    state: Entity<AppState>,
    input: Entity<InputState>,
    open: bool,
    loading: bool,
    truncated: bool,
    git_ref: String,
    files: Vec<RepoTreeEntry>,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl FindFileModal {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find file by name or path…"));
        let sub_in = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, _, cx| match ev {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => this.open_top_match(cx),
            _ => {}
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                (this.open, this.loading, this.truncated, this.task) = (false, false, false, None);
                this.files.clear();
                cx.notify();
            }
            AppEvent::Command(ScreenCommand::OpenFindFile) => this.show(window, cx),
            _ => {}
        });
        Self {
            state, input, open: false, loading: false, truncated: false,
            git_ref: "main".into(), files: Vec::new(), task: None, _subs: vec![sub_in, sub_st],
        }
    }

    pub fn is_open(&self) -> bool { self.open }

    pub fn close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.open { self.open = false; cx.notify(); true } else { false }
    }

    fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let Some(project) = st.project.clone() else { return; };
        let git_ref = st.active_ref.clone()
            .or_else(|| project.default_branch.clone())
            .filter(|s| !s.is_empty()).unwrap_or_else(|| "main".into());
        let (client, offline, h_ep, p_ep) = (st.gitlab.clone(), st.offline_mode, st.host_epoch, st.project_epoch);
        (self.open, self.git_ref, self.task) = (true, git_ref.clone(), None);
        self.input.update(cx, |i, cx| i.set_value("", window, cx));
        self.input.focus_handle(cx).focus(window, cx);

        let cache = cx.try_global::<RepoCache>().cloned();
        let cache_gen = cache.as_ref().map(RepoCache::generation).unwrap_or(0);
        let Some(client) = client else { cx.notify(); return; };
        let origin = client.origin().ascii_serialization();
        if let Some((cached, trunc)) = cache.as_ref().and_then(|c| c.get_index(&origin, project.id, &git_ref)) {
            self.files = cached.into_iter().filter(|e| e.is_blob()).collect();
            (self.truncated, self.loading) = (trunc, false);
            cx.notify();
            return;
        }
        if offline { cx.notify(); return; }
        self.files.clear();
        (self.truncated, self.loading) = (false, true);
        cx.notify();
        self.task = Some(cx.spawn(async move |this, cx| {
            let ref_req = git_ref.clone();
            let res = spawn_tokio(async move {
                client.repo_tree_recursive_capped(project.id, &ref_req, MAX_FIND_FILES).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.git_ref != git_ref { return; }
                v.loading = false;
                match res {
                    Ok(Ok((items, trunc))) => {
                        if let Some(c) = cache.as_ref() {
                            c.insert_index(cache_gen, &origin, project.id, &git_ref, items.clone(), trunc);
                        }
                        v.files = items.into_iter().filter(|e| e.is_blob()).collect();
                        v.truncated = trunc;
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    fn filtered(&self, cx: &Context<Self>) -> Vec<RepoTreeEntry> {
        let q = self.input.read(cx).value().to_string();
        let mut scored: Vec<(i32, &RepoTreeEntry)> = self
            .files
            .iter()
            .filter_map(|e| fuzzy_score(&e.path, &q).map(|s| (s, e)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
        scored.into_iter().take(120).map(|(_, e)| e.clone()).collect()
    }

    fn open_entry(&mut self, entry: RepoTreeEntry, cx: &mut Context<Self>) {
        self.open = false;
        if let Some(pid) = self.state.read(cx).project.as_ref().map(|p| p.id) {
            let git_ref = Some(self.git_ref.clone());
            self.state.update(cx, |s, cx| {
                s.open_repo_file(pid, git_ref, entry.path, Some(entry.id), None, cx);
            });
        }
        cx.notify();
    }

    fn open_top_match(&mut self, cx: &mut Context<Self>) {
        if let Some(first) = self.filtered(cx).into_iter().next() { self.open_entry(first, cx); }
    }
}

impl Render for FindFileModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open { return div().into_any_element(); }
        let theme = cx.theme();
        let hits = self.filtered(cx);
        let count = hits.len();
        let list = uniform_list(
            "find-file-results",
            count,
            cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|ix| {
                        let entry = hits[ix].clone();
                        let icon = render_entry_icon(&entry.name, false, cx);
                        let theme = cx.theme();
                        div()
                            .id(SharedString::from(format!("ff-hit-{ix}")))
                            .h(px(30.)).px_3().cursor_pointer().hover(|s| s.bg(theme.list_hover))
                            .flex().items_center().gap_2()
                            .on_click(cx.listener(move |this, _, _, cx| this.open_entry(entry.clone(), cx)))
                            .child(icon)
                            .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.foreground).truncate().child(hits[ix].path.clone()))
                            .into_any_element()
                    })
                    .collect()
            }),
        )
        .h(px(320.))
        .w_full();

        div()
            .id("find-file-backdrop")
            .absolute().inset_0().flex().items_start().justify_center().pt(px(88.)).bg(theme.overlay)
            .on_click(cx.listener(|this, _, _, cx| { this.close(cx); }))
            .child(
                div()
                    .id("find-file-card")
                    .w(px(580.)).rounded_lg().shadow_lg().border_1().border_color(theme.border)
                    .bg(theme.popover).overflow_hidden().flex().flex_col()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        div().px_3().py_2().border_b_1().border_color(theme.border).flex().items_center().gap_2()
                            .child(Icon::new(IconName::Search).size_4().text_color(theme.muted_foreground))
                            .child(div().flex_1().child(Input::new(&self.input).small())),
                    )
                    .children(self.truncated.then(|| {
                        div().px_3().py_1().bg(theme.secondary).text_xs().text_color(theme.muted_foreground)
                            .child("Showing first 2,000 files — use Code search for larger repositories")
                    }))
                    .child(if self.loading && count == 0 {
                        div().p_4().text_xs().text_color(theme.muted_foreground).child("Indexing repository files…").into_any_element()
                    } else if count == 0 {
                        div().p_4().text_xs().text_color(theme.muted_foreground).child("No matching files").into_any_element()
                    } else {
                        list.into_any_element()
                    }),
            )
            .into_any_element()
    }
}
