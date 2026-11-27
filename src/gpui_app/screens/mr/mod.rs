mod actions;
mod activity;
pub mod annotations;
mod breadcrumb;
mod cache;
mod changes;
mod changes_chrome;
mod comments;
mod commits_tab;
mod composer;
mod detail_header;
mod detail_loader;
mod discussion_card;
mod edit_dialog;
mod list_row;
mod list_view;
mod member_picker;
mod merge_widget;
mod mutations;
mod overview;
mod pipelines_tab;
mod sidebar;
mod test_summary_widget;
mod widgets;

use gpui_kit::{
    component::{
        diff::{DiffLinePosition, DiffMode, DiffState},
        input::{InputEvent, InputState, TextareaState},
        ActiveTheme as _,
    },
    div, px, AppContext as _, Context, Entity, IntoElement, ListAlignment, ListState,
    ParentElement as _, Render, Styled as _, Subscription, Task, Window,
};

use crate::gitlab::{
    AwardEmoji, BridgeJob, Discussion, Job, MergeRequest, MrActivityFilter, MrApprovals, MrFilter,
    MrListCounts, MrPermissions, MrSortField, Pipeline, PipelineTestSummary, RawDiff, RepoCommit,
    SortDir, User,
};
use crate::nav::Screen;

use super::super::diff_hl::new_diff_state;
use super::super::markdown::MarkdownProjectContext;
use super::super::state::{AppEvent, AppState, ScreenCommand};
use self::breadcrumb::render_mr_breadcrumb;
use self::cache::ActivityEntry;
use self::changes::render_mr_changes;
use self::commits_tab::render_mr_commits_tab;
use self::detail_header::render_mr_detail_header;
use self::list_view::render_mr_list_view;
use self::member_picker::MemberPickerKind;
use self::overview::render_mr_overview;
use self::pipelines_tab::render_mr_pipelines_tab;

pub struct MergeRequestsView {
    pub(crate) state: Entity<AppState>,
    pub(crate) filter: MrFilter,
    pub(crate) sort_field: MrSortField,
    pub(crate) sort_dir: SortDir,
    pub(crate) page: usize,
    pub(crate) per_page: usize,
    pub(crate) next_page: Option<usize>,
    pub(crate) counts: MrListCounts,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) search_history: Vec<String>,
    pub(crate) mrs: Vec<MergeRequest>,
    pub(crate) list_loading: bool,
    pub(crate) selected: Option<MergeRequest>,
    pub(crate) pending_iid: Option<u64>,
    pub(crate) active_tab: usize,
    pub(crate) approvals: Option<MrApprovals>,
    pub(crate) approvals_expanded: bool,
    pub(crate) merge_details_expanded: bool,
    pub(crate) test_summary: Option<PipelineTestSummary>,
    pub(crate) test_summary_expanded: bool,
    pub(crate) edit_commit_msg: bool,
    pub(crate) commit_msg_input: Entity<TextareaState>,
    pub(crate) remove_source_branch: bool,
    pub(crate) squash_commits: bool,
    pub(crate) commits: Vec<RepoCommit>,
    pub(crate) mr_pipelines: Vec<Pipeline>,
    pub(crate) head_pipeline_jobs: Vec<Job>,
    pub(crate) head_pipeline_bridges: Vec<BridgeJob>,
    pub(crate) cached_stages: Vec<&'static str>,
    pub(crate) cached_activity: Vec<ActivityEntry>,
    pub(crate) overview_list: ListState,
    pub(crate) open_thread_cursor: usize,
    pub(crate) participants: Vec<User>,
    pub(crate) project_members: Vec<User>,
    pub(crate) members_loading: bool,
    pub(crate) members_loaded: bool,
    pub(crate) member_picker: Option<MemberPickerKind>,
    pub(crate) member_filter_input: Entity<InputState>,
    pub(crate) award_emojis: Vec<AwardEmoji>,
    pub(crate) todo_added: bool,
    pub(crate) activity_filter: MrActivityFilter,
    pub(crate) activity_desc: bool,
    pub(crate) raw_diffs: Vec<RawDiff>,
    pub(crate) fallback_notices: Vec<(String, String)>,
    pub(crate) has_truncated: bool,
    pub(crate) diff_state: Entity<DiffState>,
    pub(crate) discussions: Vec<Discussion>,
    pub(crate) discussions_error: Option<String>,
    pub(crate) inline_target: Option<DiffLinePosition>,
    pub(crate) replying_to: Option<String>,
    pub(crate) comment_preview: bool,
    pub(crate) comment_input: Entity<TextareaState>,
    pub(crate) reply_input: Entity<InputState>,
    pub(crate) inline_input: Entity<InputState>,
    pub(crate) list_task: Option<Task<()>>,
    pub(crate) counts_task: Option<Task<()>>,
    pub(crate) detail_task: Option<Task<()>>,
    pub(crate) members_task: Option<Task<()>>,
    pub(crate) mutation_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl MergeRequestsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = if state.read(cx).prefs.default_diff_split { DiffMode::Split } else { DiffMode::Unified };
        let diff_state = cx.new(|cx| new_diff_state(Vec::new(), mode, cx));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search or filter results..."));
        let comment_input = cx.new(|cx| TextareaState::new(window, cx).placeholder("Write a comment or drag your files here…"));
        let commit_msg_input = cx.new(|cx| TextareaState::new(window, cx).placeholder("Custom merge/squash commit message…"));
        let reply_input = cx.new(|cx| InputState::new(window, cx).placeholder("Reply…"));
        let inline_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add inline review comment…"));
        let member_filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search users…"));
        let sub_member = cx.subscribe_in(&member_filter_input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_search = cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) { this.submit_search(cx); }
        });
        let sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.reset_state(cx);
                if this.state.read(cx).screen == Screen::MergeRequests { this.load_list(cx); }
            }
            AppEvent::PermissionsChanged => {
                this.sync_summary(cx);
                this.overview_list.remeasure();
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::MergeRequests => {
                if this.mrs.is_empty() && !this.list_loading { this.load_list(cx); }
                this.sync_summary(cx);
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::MergeRequests => {
                if let Some(mr) = this.selected.clone() { this.select_mr(mr, false, cx); } else { this.load_list(cx); }
            }
            AppEvent::Command(ScreenCommand::ShowMrList) => this.show_list_without_history(cx),
            AppEvent::Command(ScreenCommand::ApproveMr) => this.exec_mr_mutation("approve", cx),
            AppEvent::Command(ScreenCommand::MergeMr) => this.exec_mr_mutation("merge", cx),
            AppEvent::Command(ScreenCommand::CloseMr) => this.exec_mr_mutation("close", cx),
            AppEvent::Command(ScreenCommand::OpenMr { iid, .. }) => this.open_mr_by_iid(*iid, cx),
            AppEvent::PrefsChanged => {
                let m = if this.state.read(cx).prefs.default_diff_split { DiffMode::Split } else { DiffMode::Unified };
                this.diff_state.update(cx, |ds, cx| ds.set_mode(m, cx));
            }
            _ => {}
        });
        Self {
            state, filter: MrFilter::Open, sort_field: MrSortField::CreatedAt, sort_dir: SortDir::Desc,
            page: 1, per_page: 20, next_page: None, counts: MrListCounts::default(),
            search_input, search_history: Vec::new(), mrs: Vec::new(), list_loading: false,
            selected: None, pending_iid: None, active_tab: 0, approvals: None,
            approvals_expanded: false, merge_details_expanded: true, test_summary: None,
            test_summary_expanded: false, edit_commit_msg: false, commit_msg_input,
            remove_source_branch: false, squash_commits: false, commits: Vec::new(),
            mr_pipelines: Vec::new(), head_pipeline_jobs: Vec::new(), head_pipeline_bridges: Vec::new(),
            cached_stages: Vec::new(), cached_activity: Vec::new(),
            overview_list: ListState::new(4, ListAlignment::Top, px(240.)), open_thread_cursor: 0,
            participants: Vec::new(), project_members: Vec::new(), members_loading: false,
            members_loaded: false, member_picker: None, member_filter_input, award_emojis: Vec::new(),
            todo_added: false, activity_filter: MrActivityFilter::All, activity_desc: false,
            raw_diffs: Vec::new(), fallback_notices: Vec::new(), has_truncated: false,
            diff_state, discussions: Vec::new(), discussions_error: None, inline_target: None,
            replying_to: None, comment_preview: false, comment_input, reply_input, inline_input,
            list_task: None, counts_task: None, detail_task: None, members_task: None,
            mutation_task: None, _subs: vec![sub_search, sub_member, sub],
        }
    }

    pub fn diff_mode(&self, cx: &gpui_kit::App) -> DiffMode { self.diff_state.read(cx).mode() }
    pub fn selected_iid(&self) -> Option<u64> { self.selected.as_ref().map(|m| m.iid) }
    pub fn selected_mr(&self) -> Option<&MergeRequest> { self.selected.as_ref() }
    pub fn active_tab(&self) -> usize { self.active_tab }
    pub fn filter(&self) -> MrFilter { self.filter }

    pub(crate) fn mr_permissions(&self, mr: &MergeRequest, cx: &gpui_kit::App) -> MrPermissions {
        let st = self.state.read(cx);
        let me = st.current_user.as_ref().map(|u| u.id);
        st.project_permissions.mr_permissions(mr, self.approvals.as_ref(), me)
    }

    pub fn toggle_diff_mode(&mut self, cx: &mut Context<Self>) {
        let next = if self.diff_state.read(cx).mode() == DiffMode::Split { DiffMode::Unified } else { DiffMode::Split };
        self.diff_state.update(cx, |ds, cx| ds.set_mode(next, cx));
        cx.notify();
    }
}

impl Render for MergeRequestsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (project, client, md_ctx) = {
            let st = self.state.read(cx);
            (st.project.clone(), st.gitlab.clone(), MarkdownProjectContext::from_state(st, cx))
        };
        let Some(project) = project else {
            return div()
                .size_full().flex().items_center().justify_center()
                .text_sm().text_color(cx.theme().muted_foreground)
                .child("Select a project to view its merge requests")
                .into_any_element();
        };

        let breadcrumb = render_mr_breadcrumb(&project, self.selected.as_ref().map(|m| m.iid), &self.state, cx);
        let body = if let Some(mr) = self.selected.clone() {
            let header = render_mr_detail_header(self, &mr, cx);
            let tab_body = match self.active_tab {
                0 => render_mr_overview(self, &mr, &project, md_ctx, client.as_ref(), cx).into_any_element(),
                1 => render_mr_commits_tab(self, &mr, cx).into_any_element(),
                2 => render_mr_pipelines_tab(self, &mr, cx).into_any_element(),
                _ => render_mr_changes(self, &mr, md_ctx, cx).into_any_element(),
            };
            div().flex_1().min_h_0().flex().flex_col().child(header)
                .child(div().flex_1().min_h_0().flex().flex_col().child(tab_body))
                .into_any_element()
        } else {
            render_mr_list_view(self, &project, client.as_ref(), cx).into_any_element()
        };

        div().size_full().flex().flex_col().child(breadcrumb).child(body).into_any_element()
    }
}
