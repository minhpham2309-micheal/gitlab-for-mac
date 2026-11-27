use gpui_kit::Context;

use crate::accounts::{self, build_host_auth};
use crate::config::HostAuth;
use crate::gitlab::{GitLab, ProjectPermissions, User};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::nav_history::NavEntry;
use super::state::AppState;
use super::state_types::{AppEvent, ScreenContextSummary, SidebarCounts};

#[derive(Debug, Clone, Default)]
pub struct AuthUiState {
    pub saved_hosts: Vec<HostAuth>,
    pub glab_hosts: Vec<HostAuth>,
    pub busy: bool,
    pub error: Option<String>,
    pub adding_account: bool,
}

impl AuthUiState {
    pub fn is_saved_host(&self, hostname: &str) -> bool {
        self.saved_hosts.iter().any(|h| h.hostname == hostname)
    }
}

impl AppState {
    pub fn is_authenticated(&self) -> bool {
        self.active_host.is_some() && !self.auth.adding_account
    }

    pub fn reload_discovered_accounts(&mut self, cx: &mut Context<Self>) {
        if !self.persist_prefs {
            return;
        }
        let discovered = accounts::discover_all();
        self.auth.saved_hosts = discovered.saved;
        self.auth.glab_hosts = discovered.glab;
        self.hosts = discovered.merged;
        self.auth.error = None;
        cx.notify();
    }

    pub fn open_add_account(&mut self, cx: &mut Context<Self>) {
        self.auth.adding_account = true;
        self.reload_discovered_accounts(cx);
        cx.notify();
    }

    pub fn cancel_add_account(&mut self, cx: &mut Context<Self>) {
        if self.active_host.is_some() {
            self.auth.adding_account = false;
            self.auth.error = None;
            self.auth.busy = false;
            self.auth_task = None;
            cx.notify();
        }
    }

    pub fn sign_in_with_token(&mut self, raw_host: String, raw_token: String, cx: &mut Context<Self>) {
        if self.auth.busy {
            return;
        }
        match build_host_auth(&raw_host, &raw_token) {
            Ok(host) => self.verify_and_sign_in(host, true, cx),
            Err(err) => {
                self.auth.error = Some(err);
                cx.notify();
            }
        }
    }

    pub fn sign_in_with_existing_host(&mut self, host: HostAuth, cx: &mut Context<Self>) {
        if !self.auth.busy {
            self.verify_and_sign_in(host, false, cx);
        }
    }

    fn verify_and_sign_in(&mut self, host: HostAuth, persist_account: bool, cx: &mut Context<Self>) {
        let client = match GitLab::new(host.clone()) {
            Ok(c) => c,
            Err(err) => {
                self.auth.error = Some(err.to_string());
                cx.notify();
                return;
            }
        };
        if self.offline_mode {
            self.complete_sign_in(host, None, persist_account, cx);
            return;
        }

        self.auth.busy = true;
        self.auth.error = None;
        cx.notify();
        self.auth_task = Some(cx.spawn(async move |this, cx| {
            let verify = spawn_tokio(async move { client.current_user().await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |state, cx| {
                state.auth.busy = false;
                match verify {
                    Ok(Ok(user)) => {
                        let mut verified = host;
                        verified.user = Some(user.username.clone());
                        state.complete_sign_in(verified, Some(user), persist_account, cx);
                    }
                    Ok(Err(err)) | Err(err) => {
                        state.auth.error = Some(format!("Sign-in failed: {err}"));
                        cx.notify();
                    }
                }
            });
        }));
    }

    fn complete_sign_in(
        &mut self,
        host: HostAuth,
        user: Option<User>,
        persist_account: bool,
        cx: &mut Context<Self>,
    ) {
        if persist_account && self.persist_prefs {
            match accounts::upsert_account(&host) {
                Ok(saved) => {
                    self.auth.saved_hosts = saved;
                    self.hosts = accounts::merge_hosts(&self.auth.saved_hosts, &self.auth.glab_hosts);
                }
                Err(err) => {
                    self.auth.error = Some(format!("Failed to store credentials: {err}"));
                    cx.notify();
                    return;
                }
            }
        } else if !self.hosts.iter().any(|h| h.hostname == host.hostname) {
            self.hosts.push(host.clone());
        }

        let client = match GitLab::new(host.clone()) {
            Ok(c) => c,
            Err(err) => {
                self.auth.error = Some(err.to_string());
                cx.notify();
                return;
            }
        };

        self.clear_session_state(cx);
        let hostname = host.hostname.clone();
        self.active_host = Some(host);
        self.gitlab = Some(client);
        self.current_user = user;
        self.auth.adding_account = false;
        self.auth.error = None;
        self.auth.busy = false;
        self.prefs.host = Some(hostname.clone());
        self.prefs.project = None;
        self.prefs.signed_in = Some(true);
        self.save_prefs();
        self.status = format!("Signed in on {hostname}");
        cx.emit(AppEvent::HostChanged { epoch: self.host_epoch });
        self.refresh_host_data(cx);
        cx.notify();
    }

    pub fn sign_out(&mut self, forget_saved_token: bool, cx: &mut Context<Self>) {
        if forget_saved_token
            && let Some(name) = self.active_host.as_ref().map(|h| h.hostname.clone())
        {
            self.remove_saved_host_internal(&name);
        }
        self.clear_session_state(cx);
        self.active_host = None;
        self.gitlab = None;
        self.current_user = None;
        self.auth.adding_account = false;
        self.auth.error = None;
        self.auth.busy = false;
        self.prefs.signed_in = Some(false);
        self.prefs.project = None;
        self.save_prefs();
        self.status = "Signed out".into();
        cx.emit(AppEvent::HostChanged { epoch: self.host_epoch });
        cx.notify();
    }

    pub fn remove_saved_account(&mut self, hostname: &str, cx: &mut Context<Self>) {
        self.remove_saved_host_internal(hostname);
        if self.active_host.as_ref().map(|h| h.hostname.as_str()) == Some(hostname) {
            self.sign_out(false, cx);
        } else {
            cx.notify();
        }
    }

    fn remove_saved_host_internal(&mut self, hostname: &str) {
        if self.persist_prefs {
            if let Ok(saved) = accounts::remove_account(hostname) {
                self.auth.saved_hosts = saved;
            }
        } else {
            self.auth.saved_hosts.retain(|h| h.hostname != hostname);
        }
        self.hosts = accounts::merge_hosts(&self.auth.saved_hosts, &self.auth.glab_hosts);
    }

    fn clear_session_state(&mut self, cx: &mut Context<Self>) {
        self.host_load_task = None;
        self.counts_task = None;
        self.auth_task = None;
        self.host_epoch = self.host_epoch.wrapping_add(1);
        self.project_epoch = self.project_epoch.wrapping_add(1);
        self.projects.clear();
        self.project = None;
        self.project_permissions = ProjectPermissions::unknown();
        self.active_ref = None;
        self.counts = SidebarCounts::default();
        self.summary = ScreenContextSummary::default();
        self.screen = Screen::Projects;
        self.history.reset(NavEntry::screen(None, Screen::Projects));
        self.busy = false;
        if let Some(cache) = cx.try_global::<super::image_cache::ImageCache>() {
            cache.clear();
        }
        if let Some(cache) = cx.try_global::<super::repo_cache::RepoCache>() {
            cache.clear();
        }
    }
}
