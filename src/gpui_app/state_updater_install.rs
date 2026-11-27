use std::path::PathBuf;

use gpui_kit::Context;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::runtime::runtime_handle;
use crate::updater::client::download_and_verify;
use crate::updater::install::{extract_and_verify_bundle, stage_sibling_and_spawn_relaunch};
use crate::updater::{
    updates_cache_dir, DownloadProgress, InstallTarget, UpdateConfig, UpdateInfo,
};

use super::state::AppState;
use super::state_updater::UpdatePhase;

enum InstallStep {
    Progress(DownloadProgress),
    Verifying,
    Installing,
    ReadyToRestart,
    Failed(String),
}

struct AbortGuard(JoinHandle<()>);
impl Drop for AbortGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl AppState {
    pub fn cancel_update_download(&mut self, cx: &mut Context<Self>) {
        if let UpdatePhase::Downloading { info, .. } | UpdatePhase::Verifying { info } =
            self.update.phase.clone()
        {
            self.update_task = None;
            self.update.phase = UpdatePhase::Available(info);
            self.status = "Update download cancelled".into();
            cx.notify();
        }
    }

    pub fn start_update_install(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.update.info().cloned() else {
            return;
        };
        let InstallTarget::WritableBundle(current_bundle) = self.update.install_target.clone() else {
            return;
        };
        if self.update.is_busy() {
            return;
        }

        let cfg = UpdateConfig::for_current_app(
            self.prefs.update_channel,
            None,
            self.prefs.installed_canary_id.clone(),
        );
        let total = (info.asset_size > 0).then_some(info.asset_size);
        let info_box = Box::new(info.clone());
        self.update.phase = UpdatePhase::Downloading {
            info: info_box.clone(),
            downloaded: 0,
            total,
        };
        self.status = format!("Downloading v{}…", info.version);
        cx.notify();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let pid = std::process::id();
        let handle = runtime_handle().spawn(async move {
            run_install_pipeline(cfg, info, current_bundle, pid, tx).await;
        });
        let guard = AbortGuard(handle);

        self.update_task = Some(cx.spawn(async move |this, cx| {
            let _guard = guard;
            while let Some(step) = rx.recv().await {
                let mut should_quit = false;
                let _ = this.update(cx, |state, cx| {
                    match step {
                        InstallStep::Progress(p) => {
                            state.update.phase = UpdatePhase::Downloading {
                                info: info_box.clone(),
                                downloaded: p.downloaded_bytes,
                                total: p.total_bytes,
                            };
                        }
                        InstallStep::Verifying => {
                            state.update.phase = UpdatePhase::Verifying { info: info_box.clone() };
                            state.status = "Verifying archive and code signature…".into();
                        }
                        InstallStep::Installing => {
                            state.update.phase = UpdatePhase::Installing { info: info_box.clone() };
                            state.status = "Installing update and restarting…".into();
                        }
                        InstallStep::ReadyToRestart => {
                            if let Some(cid) = info_box.canary_id.clone() {
                                state.prefs.installed_canary_id = Some(cid);
                            }
                            state.prefs.dismissed_update_version = None;
                            state.prefs.update_etag = None;
                            state.save_prefs();
                            should_quit = true;
                        }
                        InstallStep::Failed(message) => {
                            state.status = format!("Update failed: {message}");
                            state.update.phase = UpdatePhase::Error {
                                message,
                                info: Some(info_box.clone()),
                            };
                        }
                    }
                    cx.notify();
                });
                if should_quit {
                    cx.update(|cx| cx.quit());
                    break;
                }
            }
        }));
    }
}

async fn run_install_pipeline(
    cfg: UpdateConfig,
    info: UpdateInfo,
    current_bundle: PathBuf,
    pid: u32,
    tx: mpsc::UnboundedSender<InstallStep>,
) {
    let cache_root = match updates_cache_dir() {
        Ok(dir) => dir.join(format!("run-{pid}")),
        Err(err) => {
            let _ = tx.send(InstallStep::Failed(err.to_string()));
            return;
        }
    };
    let progress_tx = tx.clone();
    let archive = match download_and_verify(&cfg, &info, &cache_root, move |p| {
        let _ = progress_tx.send(InstallStep::Progress(p));
    })
    .await
    {
        Ok(a) => a,
        Err(err) => {
            let _ = tx.send(InstallStep::Failed(err.to_string()));
            return;
        }
    };

    let _ = tx.send(InstallStep::Verifying);
    let staging_dir = cache_root.join("extracted");
    let zip_path = archive.zip_path;
    let bundle_for_verify = current_bundle.clone();
    let extracted = match tokio::task::spawn_blocking(move || {
        extract_and_verify_bundle(&zip_path, &staging_dir, &bundle_for_verify)
    })
    .await
    {
        Ok(Ok(path)) => path,
        Ok(Err(err)) => {
            let _ = tx.send(InstallStep::Failed(err.to_string()));
            return;
        }
        Err(err) => {
            let _ = tx.send(InstallStep::Failed(err.to_string()));
            return;
        }
    };

    let _ = tx.send(InstallStep::Installing);
    let swap_res = tokio::task::spawn_blocking(move || {
        stage_sibling_and_spawn_relaunch(&current_bundle, &extracted, &cache_root, pid)
    })
    .await;
    match swap_res {
        Ok(Ok(())) => {
            let _ = tx.send(InstallStep::ReadyToRestart);
        }
        Ok(Err(err)) => {
            let _ = tx.send(InstallStep::Failed(err.to_string()));
        }
        Err(err) => {
            let _ = tx.send(InstallStep::Failed(err.to_string()));
        }
    }
}
