use gpui_kit::{
    component::{
        diff::{DiffLinePosition, DiffMode, DiffState},
        input::InputState,
        resizable::{h_resizable, resizable_panel},
        tab::{Tab, TabBar},
        ActiveTheme as _,
    },
    div, px, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Task, Window,
};

use crate::gitlab::{mr_can_review, Discussion, MergeRequest, MrFilter, RawDiff};
use crate::nav::Screen;

use super::super::diff_hl::new_diff_state;
use super::super::markdown::MarkdownProjectContext;
use super::super::state::{AppEvent, AppState, ScreenCommand};
use super::mr_changes::render_mr_changes;
use super::mr_chrome::{render_mr_header, render_mr_list_panel};
use super::mr_overview::render_mr_overview;

pub struct MergeRequestsView {
    pub(crate) state: Entity<AppState>,
    pub(crate) filter: MrFilter,
    pub(crate) mrs: Vec<MergeRequest>,
    pub(crate) selected: Option<MergeRequest>,
    pub(crate) pending_iid: Option<u64>,
    pub(crate) active_tab: usize,
    pub(crate) raw_diffs: Vec<RawDiff>,
    pub(crate) fallback_notices: Vec<(String, String)>,
    pub(crate) has_truncated: bool,
    pub(crate) diff_state: Entity<DiffState>,
    pub(crate) discussions: Vec<Discussion>,
    pub(crate) discussions_error: Option<String>,
    pub(crate) inline_target: Option<DiffLinePosition>,
    pub(crate) replying_to: Option<String>,
    pub(crate) comment_input: Entity<InputState>,
    pub(crate) reply_input: Entity<InputState>,
    pub(crate) inline_input: Entity<InputState>,
    pub(crate) list_task: Option<Task<()>>,
    pub(crate) detail_task: Option<Task<()>>,
    pub(crate) mutation_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl MergeRequestsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_mode = if state.read(cx).prefs.default_diff_split {
            DiffMode::Split
        } else {
            DiffMode::Unified
        };
        let diff_state = cx.new(|cx| new_diff_state(Vec::new(), initial_mode, cx));
        let comment_input = cx.new(|cx| InputState::new(window, cx).placeholder("Write a comment…"));
        let reply_input = cx.new(|cx| InputState::new(window, cx).placeholder("Reply to thread…"));
        let inline_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add inline review comment…"));
        let sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _w, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.reset_state(cx);
                if this.state.read(cx).screen == Screen::MergeRequests {
                    this.load_list(cx);
                }
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::MergeRequests => {
                if this.mrs.is_empty() {
                    this.load_list(cx);
                }
                this.sync_summary(cx);
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::MergeRequests =>
            {
                this.load_list(cx);
            }
            AppEvent::Command(ScreenCommand::ApproveMr) => this.exec_mr_mutation("approve", cx),
            AppEvent::Command(ScreenCommand::MergeMr) => this.exec_mr_mutation("merge", cx),
            AppEvent::Command(ScreenCommand::CloseMr) => this.exec_mr_mutation("close", cx),
            AppEvent::Command(ScreenCommand::OpenMr { iid, .. }) => {
                this.pending_iid = Some(*iid);
                this.load_list(cx);
            }
            AppEvent::PrefsChanged => {
                let mode = if this.state.read(cx).prefs.default_diff_split {
                    DiffMode::Split
                } else {
                    DiffMode::Unified
                };
                this.diff_state.update(cx, |ds, cx| ds.set_mode(mode, cx));
            }
            _ => {}
        });
        Self {
            state, filter: MrFilter::Open, mrs: Vec::new(), selected: None, pending_iid: None,
            active_tab: 0, raw_diffs: Vec::new(), fallback_notices: Vec::new(),
            has_truncated: false, diff_state, discussions: Vec::new(), discussions_error: None,
            inline_target: None, replying_to: None, comment_input, reply_input, inline_input,
            list_task: None, detail_task: None, mutation_task: None, _subs: vec![sub],
        }
    }

    pub fn diff_mode(&self, cx: &gpui_kit::App) -> DiffMode {
        self.diff_state.read(cx).mode()
    }

    pub fn toggle_diff_mode(&mut self, cx: &mut Context<Self>) {
        let next = if self.diff_state.read(cx).mode() == DiffMode::Split {
            DiffMode::Unified
        } else {
            DiffMode::Split
        };
        self.diff_state.update(cx, |ds, cx| ds.set_mode(next, cx));
        cx.notify();
    }

    fn reset_state(&mut self, cx: &mut Context<Self>) {
        self.list_task = None;
        self.detail_task = None;
        self.mutation_task = None;
        self.mrs.clear();
        self.selected = None;
        self.raw_diffs.clear();
        self.fallback_notices.clear();
        self.has_truncated = false;
        self.discussions.clear();
        self.discussions_error = None;
        self.inline_target = None;
        self.replying_to = None;
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
        self.sync_summary(cx);
        cx.notify();
    }

    pub(crate) fn sync_summary(&self, cx: &mut Context<Self>) {
        let mr = self.selected.clone();
        self.state.update(cx, |s, _| {
            s.summary.can_review_mr = mr.as_ref().is_some_and(|m| mr_can_review(&m.state));
            s.summary.active_mr_iid = mr.as_ref().map(|m| m.iid);
            s.summary.active_mr_label = mr.as_ref().map(|m| format!("!{} {}", m.iid, m.title));
            if let Some(m) = &mr {
                s.summary.active_web_url = Some(m.web_url.clone());
            }
        });
    }
}

impl Render for MergeRequestsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let client = self.state.read(cx).gitlab.clone();
        let left = render_mr_list_panel(
            self.filter,
            self.mrs.clone(),
            self.selected.as_ref().map(|m| m.iid),
            client.clone(),
            cx,
        );

        let right = if let Some(mr) = self.selected.clone() {
            let md_ctx = MarkdownProjectContext::from_state(self.state.read(cx), cx);
            let header = render_mr_header(&mr, cx);
            let tabs = TabBar::new("mr-detail-tabs")
                .selected_index(self.active_tab)
                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                    this.active_tab = *ix;
                    cx.notify();
                }))
                .child(Tab::new().label(format!("Overview ({})", self.discussions.len())))
                .child(Tab::new().label(format!("Changes ({})", self.raw_diffs.len())));

            let tab_body = if self.active_tab == 0 {
                render_mr_overview(self, &mr, md_ctx, client.as_ref(), cx).into_any_element()
            } else {
                render_mr_changes(self, &mr, md_ctx, cx).into_any_element()
            };

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(header)
                .child(tabs)
                .child(div().flex_1().min_h_0().flex().flex_col().child(tab_body))
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Select a merge request to inspect overview, diff, and review threads")
        };

        h_resizable("mr-main-split")
            .child(resizable_panel().size(px(320.)).size_range(px(240.)..px(500.)).child(left))
            .child(resizable_panel().child(right))
    }
}
