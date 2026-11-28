mod code_viewer;
pub mod file_finder;
pub mod fuzzy;
mod loaders;
mod tree_panel;
mod viewer_header;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::{
    component::{
        input::{EditorState, InputEvent, InputState},
        resizable::{h_resizable, resizable_panel},
    },
    div, px, AppContext as _, Context, Entity, Image, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, Task, Window,
};

use crate::gitlab::{RepoBlob, RepoBlobKind, RepoCommit, RepoTreeEntry};
use crate::nav::Screen;

use super::super::markdown::MarkdownProjectContext;
use super::super::state::{AppEvent, AppState, ScreenCommand};
use self::code_viewer::render_code_body;
use self::tree_panel::render_tree_panel;
use self::viewer_header::render_viewer_header;

pub struct RepositoryView {
    pub(super) state: Entity<AppState>,
    pub(super) editor_state: Entity<EditorState>,
    pub(super) tree_filter: Entity<InputState>,
    pub(super) active_ref: String,
    pub(super) active_path: Option<String>,
    pub(super) active_blob: Option<RepoBlob>,
    pub(super) file_last_commit: Option<RepoCommit>,
    pub(super) decoded_image: Option<Arc<Image>>,
    pub(super) blob_loading: bool,
    pub(super) blob_error: Option<String>,
    pub(super) blob_seq: u64,
    pub(super) dirs: HashMap<String, Vec<RepoTreeEntry>>,
    pub(super) expanded_dirs: HashSet<String>,
    pub(super) show_tree: bool,
    pub(super) markdown_preview: bool,
    pub(super) soft_wrap: bool,
    pub(super) blob_task: Option<Task<()>>,
    pub(super) commit_task: Option<Task<()>>,
    pub(super) dir_tasks: HashMap<String, Task<()>>,
    _subs: Vec<Subscription>,
}

impl RepositoryView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor_state = cx.new(|cx| EditorState::new(window, cx).line_number(true));
        let tree_filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter tree…"));
        let sub_f = cx.subscribe_in(&tree_filter, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_s = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, window, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.reset_for_project(window, cx);
                if this.state.read(cx).screen == Screen::Repository {
                    this.load_dir(String::new(), false, cx);
                }
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Repository => {
                if this.dirs.is_empty() { this.load_dir(String::new(), false, cx); }
            }
            AppEvent::Command(ScreenCommand::OpenRepoFile { project_id, git_ref, path, blob_sha, line }) => {
                if this.state.read(cx).project.as_ref().is_some_and(|p| p.id == *project_id) {
                    this.open_file_request(path.clone(), git_ref.clone(), blob_sha.clone(), *line, window, cx);
                }
            }
            AppEvent::Command(ScreenCommand::FocusLogSearch)
                if this.state.read(cx).screen == Screen::Repository => { this.open_in_file_search(cx); }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Repository =>
            {
                let to_reload: Vec<String> = std::iter::once(String::new())
                    .chain(this.expanded_dirs.iter().cloned())
                    .collect();
                this.dirs.clear();
                this.dir_tasks.clear();
                for dir in to_reload { this.load_dir(dir, true, cx); }
            }
            _ => {}
        });
        Self {
            state, editor_state, tree_filter, active_ref: "main".into(),
            active_path: None, active_blob: None, file_last_commit: None, decoded_image: None,
            blob_loading: false, blob_error: None, blob_seq: 1,
            dirs: HashMap::new(), expanded_dirs: HashSet::new(),
            show_tree: true, markdown_preview: true, soft_wrap: false,
            blob_task: None, commit_task: None, dir_tasks: HashMap::new(), _subs: vec![sub_f, sub_s],
        }
    }

    pub fn active_path(&self) -> Option<&str> { self.active_path.as_deref() }
    pub fn active_blob(&self) -> Option<&RepoBlob> { self.active_blob.as_ref() }
    pub fn is_markdown_preview(&self) -> bool { self.markdown_preview }

    pub fn toggle_markdown_preview(&mut self, cx: &mut Context<Self>) {
        self.markdown_preview = !self.markdown_preview;
        cx.notify();
    }

    pub fn set_fixture_blob(
        &mut self, path: String, blob: RepoBlob, line: Option<u32>,
        window: &mut Window, cx: &mut Context<Self>,
    ) {
        self.active_path = Some(path.clone());
        self.markdown_preview = viewer_header::is_markdown_path(&path);
        self.apply_blob(&path, blob, line, window, cx);
    }

    pub fn set_fixture_last_commit(&mut self, commit: Option<RepoCommit>, cx: &mut Context<Self>) {
        self.file_last_commit = commit;
        cx.notify();
    }

    fn reset_for_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        (self.blob_task, self.commit_task, self.file_last_commit) = (None, None, None);
        self.dir_tasks.clear();
        let st = self.state.read(cx);
        self.active_ref = st.active_ref.clone()
            .or_else(|| st.project.as_ref().and_then(|p| p.default_branch.clone()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main".into());
        self.active_path = None;
        self.active_blob = None;
        self.decoded_image = None;
        self.blob_loading = false;
        self.blob_error = None;
        self.dirs.clear();
        self.expanded_dirs.clear();
        self.tree_filter.update(cx, |i, cx| i.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn toggle_dir(&mut self, path: String, cx: &mut Context<Self>) {
        if self.expanded_dirs.contains(&path) {
            self.expanded_dirs.remove(&path);
        } else {
            self.expanded_dirs.insert(path.clone());
            if !self.dirs.contains_key(&path) { self.load_dir(path, false, cx); }
        }
        cx.notify();
    }

    pub(super) fn open_entry(&mut self, entry: &RepoTreeEntry, window: &mut Window, cx: &mut Context<Self>) {
        let git_ref = Some(self.active_ref.clone());
        let path = entry.path.clone();
        let sha = Some(entry.id.clone());
        self.state.update(cx, |s, cx| s.record_repo_file(git_ref, path.clone(), sha.clone(), None, cx));
        self.open_file_request(path, None, sha, None, window, cx);
    }

    pub(super) fn toggle_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.soft_wrap = !self.soft_wrap;
        let wrap = self.soft_wrap;
        self.editor_state.update(cx, |ed, cx| ed.set_soft_wrap(wrap, window, cx));
        cx.notify();
    }

    pub(super) fn open_in_file_search(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.active_blob.as_ref().map(|b| &b.kind), Some(RepoBlobKind::Text(_))) {
            return;
        }
        if self.markdown_preview {
            self.markdown_preview = false;
        }
        self.editor_state.update(cx, |ed, cx| ed.open_search(false, cx));
        cx.notify();
    }
}

impl Render for RepositoryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let md_ctx = MarkdownProjectContext::from_state(self.state.read(cx), cx);
        let header = render_viewer_header(self, cx);
        let body = render_code_body(self, md_ctx, cx);
        let right = div().size_full().flex().flex_col().child(header).child(div().flex_1().min_h_0().child(body));
        if !self.show_tree { return right.into_any_element(); }
        let tree = render_tree_panel(self, cx);
        h_resizable("repo-source-split")
            .child(resizable_panel().size(px(260.)).size_range(px(180.)..px(420.)).child(tree))
            .child(resizable_panel().child(right))
            .into_any_element()
    }
}
