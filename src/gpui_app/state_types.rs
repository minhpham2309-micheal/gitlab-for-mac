use crate::nav::Screen;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenCommand {
    Refresh,
    ToggleFollowLog,
    FocusLogSearch,
    ClearLogSearch,
    LogFindNext,
    LogFindPrev,
    CopyFullLog,
    RunPipeline,
    RetryPipeline,
    CancelPipeline,
    RetryJob,
    CancelJob,
    PlayJob,
    DownloadArtifacts,
    ApproveMr,
    MergeMr,
    CloseMr,
    OpenMr { project_id: u64, iid: u64 },
    OpenIssue { project_id: u64, iid: u64 },
    OpenRepoFile {
        project_id: u64,
        git_ref: Option<String>,
        path: String,
        blob_sha: Option<String>,
        line: Option<u32>,
    },
    OpenCommitHistory {
        git_ref: Option<String>,
        path: Option<String>,
    },
    OpenOverviewRef {
        git_ref: String,
        path: String,
    },
    OpenFindFile,
    OpenPipeline { pipeline_id: u64 },
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    HostChanged { epoch: u64 },
    ProjectChanged { epoch: u64 },
    ScreenChanged { screen: Screen },
    PrefsChanged,
    Command(ScreenCommand),
}

#[derive(Debug, Clone, Default)]
pub struct SidebarCounts {
    pub todos: Option<u32>,
    pub open_issues: Option<u32>,
    pub open_mrs: Option<u32>,
    pub live_pipelines: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ScreenContextSummary {
    pub can_review_mr: bool,
    pub active_mr_iid: Option<u64>,
    pub active_mr_label: Option<String>,
    pub active_pipeline_id: Option<u64>,
    pub active_pipeline_live: bool,
    pub active_pipeline_label: Option<String>,
    pub active_job_id: Option<u64>,
    pub active_job_manual: bool,
    pub active_job_label: Option<String>,
    pub active_web_url: Option<String>,
    pub has_active_job: bool,
    pub following_log: bool,
}
