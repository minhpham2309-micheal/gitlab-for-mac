use gpui_kit::Context;

use crate::runtime::spawn_tokio;
use crate::updater::client::check_for_update;
use crate::updater::install::detect_current_install_target;
use crate::updater::{
    now_unix_secs, InstallTarget, UpdateChannel, UpdateCheckResult, UpdateConfig, UpdateInfo,
    AUTO_CHECK_INTERVAL_SECS,
};

use super::state::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatePhase {
    Idle,
    Checking,
    UpToDate { checked_at: u64, note: Option<String> },
    Available(Box<UpdateInfo>),
    Downloading { info: Box<UpdateInfo>, downloaded: u64, total: Option<u64> },
    Verifying { info: Box<UpdateInfo> },
    Installing { info: Box<UpdateInfo> },
    Error { message: String, info: Option<Box<UpdateInfo>> },
}

#[derive(Debug, Clone)]
pub struct UpdateUiState {
    pub phase: UpdatePhase,
    pub install_target: InstallTarget,
    pub modal_open: bool,
}

impl UpdateUiState {
    pub fn detect() -> Self {
        Self::with_target(detect_current_install_target())
    }

    pub fn with_target(install_target: InstallTarget) -> Self {
        Self { phase: UpdatePhase::Idle, install_target, modal_open: false }
    }

    pub fn info(&self) -> Option<&UpdateInfo> {
        match &self.phase {
            UpdatePhase::Available(info)
            | UpdatePhase::Downloading { info, .. }
            | UpdatePhase::Verifying { info }
            | UpdatePhase::Installing { info } => Some(info),
            UpdatePhase::Error { info, .. } => info.as_deref(),
            _ => None,
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.phase,
            UpdatePhase::Checking
                | UpdatePhase::Downloading { .. }
                | UpdatePhase::Verifying { .. }
                | UpdatePhase::Installing { .. }
        )
    }

    pub fn can_dismiss_modal(&self) -> bool {
        !matches!(self.phase, UpdatePhase::Installing { .. })
    }

    pub fn should_show_topbar_pill(&self, dismissed_version: Option<&str>) -> bool {
        match &self.phase {
            UpdatePhase::Available(info) => dismissed_version != Some(info.version.as_str()),
            UpdatePhase::Downloading { .. }
            | UpdatePhase::Verifying { .. }
            | UpdatePhase::Installing { .. } => true,
            _ => false,
        }
    }

    pub fn topbar_label(&self) -> Option<String> {
        match &self.phase {
            UpdatePhase::Available(info) => Some(format!("Update v{}", info.version)),
            UpdatePhase::Downloading { downloaded, total, .. } => Some(
                total
                    .filter(|&t| t > 0)
                    .map(|t| format!("Updating {}%", ((downloaded.saturating_mul(100)) / t).min(100)))
                    .unwrap_or_else(|| "Downloading…".into()),
            ),
            UpdatePhase::Verifying { .. } => Some("Verifying…".into()),
            UpdatePhase::Installing { .. } => Some("Restarting…".into()),
            _ => None,
        }
    }
}

impl AppState {
    pub fn maybe_auto_check_updates(&mut self, cx: &mut Context<Self>) {
        if self.offline_mode || !self.prefs.auto_check_updates() {
            return;
        }
        let now = now_unix_secs();
        if self.prefs.last_update_check_epoch.is_some_and(|t| now.saturating_sub(t) < AUTO_CHECK_INTERVAL_SECS) {
            return;
        }
        self.check_for_updates(false, cx);
    }

    pub fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.offline_mode || self.update.is_busy() {
            return;
        }
        let prev_phase = self.update.phase.clone();
        self.update.phase = UpdatePhase::Checking;
        if manual {
            self.status = "Checking for updates…".into();
        }
        let cfg = UpdateConfig::for_current_app(
            self.prefs.update_channel,
            if manual { None } else { self.prefs.update_etag.clone() },
            self.prefs.installed_canary_id.clone(),
        );
        cx.notify();

        self.update_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { check_for_update(&cfg).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |state, cx| state.apply_update_check_result(res, prev_phase, manual, cx));
        }));
    }

    fn apply_update_check_result(
        &mut self,
        res: Result<Result<UpdateCheckResult, String>, String>,
        prev_phase: UpdatePhase,
        manual: bool,
        cx: &mut Context<Self>,
    ) {
        let now = now_unix_secs();
        match res {
            Ok(Ok(UpdateCheckResult::Available { info, etag })) => {
                self.prefs.last_update_check_epoch = Some(now);
                self.prefs.update_etag = etag;
                self.save_prefs();
                let ver = info.version.clone();
                self.update.phase = UpdatePhase::Available(info);
                if manual {
                    self.update.modal_open = true;
                    self.status = format!("Update v{ver} is available");
                }
            }
            Ok(Ok(UpdateCheckResult::UpToDate { current_version, etag, note })) => {
                self.prefs.last_update_check_epoch = Some(now);
                self.prefs.update_etag = etag;
                self.save_prefs();
                if manual {
                    self.status = note
                        .clone()
                        .unwrap_or_else(|| format!("GitLab for Mac v{current_version} is up to date"));
                }
                self.update.phase = UpdatePhase::UpToDate { checked_at: now, note };
            }
            Ok(Ok(UpdateCheckResult::NotModified)) => {
                self.prefs.last_update_check_epoch = Some(now);
                self.save_prefs();
                self.update.phase = match prev_phase {
                    UpdatePhase::Available(info) => UpdatePhase::Available(info),
                    _ => UpdatePhase::UpToDate { checked_at: now, note: None },
                };
                if manual {
                    self.status = "Already on the latest release".into();
                }
            }
            Ok(Err(err)) | Err(err) => {
                if manual {
                    self.update.phase = UpdatePhase::Error { message: err.clone(), info: None };
                    self.status = format!("Update check failed: {err}");
                } else {
                    self.update.phase = prev_phase;
                }
            }
        }
        cx.notify();
    }

    pub fn open_update_modal(&mut self, cx: &mut Context<Self>) {
        self.update.modal_open = true;
        cx.notify();
    }

    pub fn close_update_modal(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.update.modal_open || !self.update.can_dismiss_modal() {
            return false;
        }
        self.update.modal_open = false;
        cx.notify();
        true
    }

    pub fn dismiss_available_update(&mut self, cx: &mut Context<Self>) {
        if let Some(info) = self.update.info() {
            self.prefs.dismissed_update_version = Some(info.version.clone());
            self.save_prefs();
        }
        self.update.modal_open = false;
        cx.notify();
    }

    pub fn set_update_channel(&mut self, channel: UpdateChannel, cx: &mut Context<Self>) {
        if self.prefs.update_channel == channel {
            return;
        }
        self.prefs.update_channel = channel;
        self.prefs.update_etag = None;
        self.prefs.dismissed_update_version = None;
        self.save_prefs();
        self.update.phase = UpdatePhase::Idle;
        cx.notify();
    }

    pub fn set_auto_check_updates(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.prefs.auto_check_updates = Some(enabled);
        self.save_prefs();
        cx.notify();
    }
}
