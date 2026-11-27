use std::fmt;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::{self, is_loopback_host, HostAuth};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAccount {
    pub hostname: String,
    pub api_host: String,
    pub api_protocol: String,
    pub token: String,
    #[serde(default)]
    pub user: Option<String>,
}

impl fmt::Debug for StoredAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredAccount")
            .field("hostname", &self.hostname)
            .field("api_host", &self.api_host)
            .field("api_protocol", &self.api_protocol)
            .field("token", &"[REDACTED]")
            .field("user", &self.user)
            .finish()
    }
}

impl From<&HostAuth> for StoredAccount {
    fn from(h: &HostAuth) -> Self {
        Self {
            hostname: h.hostname.clone(),
            api_host: h.api_host.clone(),
            api_protocol: h.api_protocol.clone(),
            token: h.token.clone(),
            user: h.user.clone(),
        }
    }
}

impl From<StoredAccount> for HostAuth {
    fn from(a: StoredAccount) -> Self {
        Self {
            hostname: a.hostname,
            api_host: a.api_host,
            api_protocol: a.api_protocol,
            token: a.token,
            user: a.user,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountsFile {
    #[serde(default)]
    accounts: Vec<StoredAccount>,
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveredAccounts {
    pub saved: Vec<HostAuth>,
    pub glab: Vec<HostAuth>,
    pub merged: Vec<HostAuth>,
}

pub fn accounts_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("gitlab-for-mac").join("accounts.json"))
}

pub fn load_saved() -> Vec<HostAuth> {
    accounts_path().map(|p| load_saved_from(&p)).unwrap_or_default()
}

pub fn load_saved_from(path: &Path) -> Vec<HostAuth> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new(); };
    let Ok(parsed) = serde_json::from_str::<AccountsFile>(&text) else { return Vec::new(); };
    parsed
        .accounts
        .into_iter()
        .filter(|a| !a.hostname.trim().is_empty() && !a.token.trim().is_empty())
        .map(HostAuth::from)
        .collect()
}

pub fn save_accounts_to(path: &Path, accounts: &[HostAuth]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    let payload = AccountsFile { accounts: accounts.iter().map(StoredAccount::from).collect() };
    let json = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    let tmp = path.with_file_name(format!(".accounts.{}.tmp", std::process::id()));
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(&tmp).map_err(|e| e.to_string())?;
    file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    #[cfg(unix)]
    let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn upsert_account_in(path: &Path, account: &HostAuth) -> Result<Vec<HostAuth>, String> {
    let mut list = load_saved_from(path);
    if let Some(existing) = list.iter_mut().find(|h| h.hostname == account.hostname) {
        *existing = account.clone();
    } else {
        list.push(account.clone());
    }
    save_accounts_to(path, &list)?;
    Ok(list)
}

pub fn upsert_account(account: &HostAuth) -> Result<Vec<HostAuth>, String> {
    let path = accounts_path().ok_or_else(|| "unable to determine macOS config directory".to_string())?;
    upsert_account_in(&path, account)
}

pub fn remove_account_in(path: &Path, hostname: &str) -> Result<Vec<HostAuth>, String> {
    let mut list = load_saved_from(path);
    list.retain(|h| h.hostname != hostname);
    save_accounts_to(path, &list)?;
    Ok(list)
}

pub fn remove_account(hostname: &str) -> Result<Vec<HostAuth>, String> {
    let path = accounts_path().ok_or_else(|| "unable to determine macOS config directory".to_string())?;
    remove_account_in(&path, hostname)
}

pub fn merge_hosts(saved: &[HostAuth], glab: &[HostAuth]) -> Vec<HostAuth> {
    let mut merged = saved.to_vec();
    for host in glab {
        if !merged.iter().any(|h| h.hostname == host.hostname) {
            merged.push(host.clone());
        }
    }
    merged
}

pub fn discover_all() -> DiscoveredAccounts {
    let saved = load_saved();
    let glab = config::load().map(|c| c.hosts).unwrap_or_default();
    let merged = merge_hosts(&saved, &glab);
    DiscoveredAccounts { saved, glab, merged }
}

pub fn build_host_auth(raw_host: &str, raw_token: &str) -> Result<HostAuth, String> {
    let token = raw_token.trim();
    if token.is_empty() {
        return Err("Enter a GitLab Personal Access Token.".into());
    }
    let trimmed = raw_host.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Enter a GitLab host (for example gitlab.com).".into());
    }
    let with_scheme = if trimmed.contains("://") { trimmed.to_string() } else { format!("https://{trimmed}") };
    let parsed = Url::parse(&with_scheme).map_err(|e| format!("Invalid GitLab host: {e}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Host URL must not contain embedded credentials.".into());
    }
    let proto = parsed.scheme().to_ascii_lowercase();
    if proto != "https" && !(proto == "http" && is_loopback_host(&parsed)) {
        return Err("Only https:// GitLab hosts are allowed (http:// is allowed on localhost only).".into());
    }
    let host_str = parsed.host_str().ok_or_else(|| "Missing hostname.".to_string())?;
    let host_port = parsed.port().map(|p| format!("{host_str}:{p}")).unwrap_or_else(|| host_str.to_string());
    let sub_path = parsed.path().trim_end_matches('/');
    let api_host = if sub_path.is_empty() || sub_path == "/" { host_port.clone() } else { format!("{host_port}{sub_path}") };
    let auth = HostAuth {
        hostname: host_port,
        api_host,
        api_protocol: proto,
        token: token.to_string(),
        user: None,
    };
    auth.api_base_url().map_err(|e| e.to_string())?;
    Ok(auth)
}

pub fn personal_access_token_url(raw_host: &str) -> Option<String> {
    let host = build_host_auth(raw_host, "placeholder").ok()?;
    Some(format!(
        "{}://{}/-/user_settings/personal_access_tokens?name=GitLab+for+Mac&scopes=api,read_user,read_repository",
        host.api_protocol, host.api_host
    ))
}
