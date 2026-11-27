use gpui_kit::{
    component::{
        input::{InputEvent, InputState},
        resizable::{h_resizable, resizable_panel, v_resizable},
    },
    px, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    Subscription, Task, UniformListScrollHandle, Window,
};

use crate::gitlab::log_model::{LogModel, LogSearchHit, VisibleLogRow};
use crate::gitlab::{job_is_live, pipeline_is_live, BridgeJob, Job, Pipeline};
use crate::nav::Screen;

use super::super::state::{AppEvent, AppState, ScreenCommand};
use super::job_log::render_job_log;
use super::pipeline_graph::render_stage_graph;
use super::pipelines_list::render_pipelines_list;

pub struct PipelinesView {
    pub(crate) state: Entity<AppState>,
    pub(crate) visible: bool,
    pub(crate) pipelines: Vec<Pipeline>,
    pub(crate) selected_pipeline: Option<Pipeline>,
    pub(crate) pending_pipeline_id: Option<u64>,
    pub(crate) jobs: Vec<Job>,
    pub(crate) bridges: Vec<BridgeJob>,
    pub(crate) selected_job: Option<Job>,
    pub(crate) branches: Vec<String>,
    pub(crate) log_model: LogModel,
    pub(crate) visible_rows: Vec<VisibleLogRow>,
    pub(crate) log_offset: u64,
    pub(crate) auto_scroll_tail: bool,
    pub(crate) wrap_lines: bool,
    pub(crate) graph_zoom_pct: u16,
    pub(crate) log_scroll: UniformListScrollHandle,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) search_hits: Vec<LogSearchHit>,
    pub(crate) search_idx: usize,
    pub(crate) sel_anchor: Option<usize>,
    pub(crate) sel_end: Option<usize>,
    pub(crate) load_task: Option<Task<()>>,
    pub(crate) jobs_task: Option<Task<()>>,
    pub(crate) ci_poll_task: Option<Task<()>>,
    pub(crate) trace_poll_task: Option<Task<()>>,
    pub(crate) search_debounce_task: Option<Task<()>>,
    pub(crate) action_task: Option<Task<()>>,
    pub(crate) download_task: Option<Task<()>>,
    pub(crate) branches_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl PipelinesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Find in log… (⌘F)"));
        let sub_search = cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| {
            match ev {
                InputEvent::Change => this.schedule_search(cx),
                InputEvent::PressEnter { shift, .. } => this.jump_search(!*shift, cx),
                _ => {}
            }
        });
        let sub_state = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, w, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.reset_all(cx);
                this.visible = this.state.read(cx).screen == Screen::Pipelines;
                if this.visible { this.load_pipelines(cx); }
            }
            AppEvent::ScreenChanged { screen } => {
                let now_visible = *screen == Screen::Pipelines;
                this.visible = now_visible;
                if now_visible {
                    if this.pipelines.is_empty() { this.load_pipelines(cx); } else { this.ensure_ci_poll(cx); }
                    if this.selected_job.as_ref().is_some_and(|j| job_is_live(&j.status)) {
                        this.start_trace_stream(true, cx);
                    }
                    this.sync_summary(cx);
                } else {
                    // Pause polling without discarding the log buffer.
                    this.ci_poll_task = None;
                    this.trace_poll_task = None;
                }
            }
            AppEvent::PrefsChanged => {
                let prefs = &this.state.read(cx).prefs;
                this.wrap_lines = prefs.default_log_wrap;
                this.graph_zoom_pct = prefs.pipeline_graph_zoom();
                cx.notify();
            }
            AppEvent::PermissionsChanged => cx.notify(),
            AppEvent::Command(cmd) => this.handle_command(cmd.clone(), w, cx),
        });
        let (initial_wrap, initial_zoom) = {
            let prefs = &state.read(cx).prefs;
            (prefs.default_log_wrap, prefs.pipeline_graph_zoom())
        };
        Self {
            state, visible: false, pipelines: Vec::new(), selected_pipeline: None,
            pending_pipeline_id: None,
            jobs: Vec::new(), bridges: Vec::new(), selected_job: None, branches: Vec::new(),
            log_model: LogModel::new(), visible_rows: Vec::new(), log_offset: 0,
            auto_scroll_tail: true, wrap_lines: initial_wrap, graph_zoom_pct: initial_zoom,
            log_scroll: UniformListScrollHandle::new(),
            search_input, search_hits: Vec::new(), search_idx: 0, sel_anchor: None, sel_end: None,
            load_task: None, jobs_task: None, ci_poll_task: None, trace_poll_task: None,
            search_debounce_task: None, action_task: None, download_task: None,
            branches_task: None, _subs: vec![sub_search, sub_state],
        }
    }

    fn handle_command(&mut self, cmd: ScreenCommand, window: &mut Window, cx: &mut Context<Self>) {
        let can_ci = self.state.read(cx).project_permissions.can_run_pipelines();
        match cmd {
            ScreenCommand::Refresh if self.visible => self.load_pipelines(cx),
            ScreenCommand::ToggleFollowLog if self.visible => self.toggle_auto_scroll(cx),
            ScreenCommand::FocusLogSearch if self.visible => self.search_input.focus_handle(cx).focus(window, cx),
            ScreenCommand::ClearLogSearch if self.visible => self.clear_log_search(window, cx),
            ScreenCommand::LogFindNext if self.visible => self.jump_search(true, cx),
            ScreenCommand::LogFindPrev if self.visible => self.jump_search(false, cx),
            ScreenCommand::CopyFullLog if self.visible => self.copy_all_log(cx),
            ScreenCommand::RunPipeline if can_ci => self.open_run_pipeline_dialog(window, cx),
            ScreenCommand::RetryPipeline if can_ci => self.run_pipeline_action("retry", cx),
            ScreenCommand::CancelPipeline if can_ci => self.run_pipeline_action("cancel", cx),
            ScreenCommand::RetryJob if can_ci => {
                if let Some(j) = self.selected_job.as_ref() { self.run_job_action(j.id, "retry", cx); }
            }
            ScreenCommand::CancelJob if can_ci => {
                if let Some(j) = self.selected_job.as_ref().filter(|j| job_is_live(&j.status)) {
                    self.run_job_action(j.id, "cancel", cx);
                }
            }
            ScreenCommand::PlayJob if can_ci => {
                if let Some(j) = self.selected_job.as_ref().filter(|j| j.status == "manual") {
                    self.run_job_action(j.id, "play", cx);
                }
            }
            ScreenCommand::DownloadArtifacts => {
                if let Some(j) = self.selected_job.as_ref() { self.download_artifacts(j.id, window, cx); }
            }
            ScreenCommand::OpenPipeline { pipeline_id } => {
                if let Some(found) = self.pipelines.iter().find(|p| p.id == pipeline_id).cloned() {
                    self.pending_pipeline_id = None;
                    self.select_pipeline(found, cx);
                } else {
                    self.pending_pipeline_id = Some(pipeline_id);
                    self.load_pipelines(cx);
                }
            }
            _ => {}
        }
    }

    fn reset_all(&mut self, cx: &mut Context<Self>) {
        self.load_task = None;
        self.jobs_task = None;
        self.ci_poll_task = None;
        self.trace_poll_task = None;
        self.action_task = None;
        self.download_task = None;
        self.branches_task = None;
        self.pipelines.clear();
        self.selected_pipeline = None;
        self.pending_pipeline_id = None;
        self.jobs.clear();
        self.bridges.clear();
        self.selected_job = None;
        self.branches.clear();
        self.wipe_log();
        self.sync_summary(cx);
        cx.notify();
    }

    pub(crate) fn sync_summary(&self, cx: &mut Context<Self>) {
        let job = self.selected_job.clone();
        let pipe = self.selected_pipeline.clone();
        let following = self.auto_scroll_tail && job.as_ref().is_some_and(|j| job_is_live(&j.status));
        self.state.update(cx, |s, _| {
            s.summary.has_active_job = job.is_some();
            s.summary.active_job_id = job.as_ref().map(|j| j.id);
            s.summary.active_job_manual = job.as_ref().is_some_and(|j| j.status == "manual");
            s.summary.active_pipeline_id = pipe.as_ref().map(|p| p.id);
            s.summary.active_pipeline_live = pipe.as_ref().is_some_and(|p| pipeline_is_live(&p.status));
            s.summary.following_log = following;
            s.summary.active_job_label = job.as_ref().map(|j| format!("{} (#{})", j.name, j.id));
            s.summary.active_pipeline_label = pipe.as_ref().map(|p| format!("#{} ({})", p.id, p.git_ref));
            if let Some(j) = &job {
                s.summary.active_web_url = Some(j.web_url.clone());
            } else if let Some(p) = &pipe {
                s.summary.active_web_url = Some(p.web_url.clone());
            }
        });
    }
}

impl Render for PipelinesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_ci = self.state.read(cx).project_permissions.can_run_pipelines();
        let left = render_pipelines_list(self.pipelines.clone(), self.selected_pipeline.as_ref(), can_ci, cx);
        let graph = render_stage_graph(
            &self.jobs,
            &self.bridges,
            self.selected_job.as_ref().map(|j| j.id),
            self.graph_zoom_pct,
            can_ci,
            cx,
        );
        let log_view = render_job_log(self, cx);
        let right = v_resizable("pipelines-right-split")
            .child(resizable_panel().size(px(270.)).size_range(px(140.)..px(520.)).child(graph))
            .child(resizable_panel().child(log_view));
        h_resizable("pipelines-main-split")
            .child(resizable_panel().size(px(290.)).size_range(px(220.)..px(460.)).child(left))
            .child(resizable_panel().child(right))
    }
}
