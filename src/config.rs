use std::fmt;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use thiserror::Error;
use url::{Host, Origin, Url};

pub use crate::config_parse::parse;

#[derive(Clone, PartialEq, Eq)]
pub struct HostAuth {
    pub hostname: String,
    pub api_host: String,
    pub api_protocol: String,
    pub token: String,
    pub user: Option<String>,
}

impl fmt::Debug for HostAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostAuth")
            .field("hostname", &self.hostname)
            .field("api_host", &self.api_host)
            .field("api_protocol", &self.api_protocol)
            .field("token", &"[REDACTED]")
            .field("user", &self.user)
            .finish()
    }
}

impl HostAuth {
    pub fn api_base_url(&self) -> Result<Url, ConfigError> {
        let proto = self.api_protocol.trim().to_ascii_lowercase();
        let raw = format!("{proto}://{}/api/v4", self.api_host.trim().trim_end_matches('/'));
        let url = Url::parse(&raw)
            .map_err(|err| ConfigError::InvalidHost(format!("{raw}: {err}")))?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ConfigError::InvalidHost(format!(
                "credentials not allowed in api_host for {}",
                self.hostname
            )));
        }
        match url.scheme() {
            "https" => {}
            "http" if is_loopback_host(&url) => {}
            scheme => {
                return Err(ConfigError::InvalidHost(format!(
                    "disallowed protocol '{scheme}' for {}",
                    self.hostname
                )));
            }
        }
        if url.host().is_none() {
            return Err(ConfigError::InvalidHost(format!(
                "missing host for {}",
                self.hostname
            )));
        }
        Ok(url)
    }

    pub fn api_base(&self) -> Result<String, ConfigError> {
        Ok(self.api_base_url()?.as_str().trim_end_matches('/').to_string())
    }

    pub fn origin(&self) -> Result<Origin, ConfigError> {
        Ok(self.api_base_url()?.origin())
    }
}

pub fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        None => false,
    }
}

impl fmt::Display for HostAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hostname)
    }
}

#[derive(Debug, Clone, Default)]
pub struct GlabConfig {
    pub default_host: Option<String>,
    pub hosts: Vec<HostAuth>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("glab config not found at {0}")]
    Missing(PathBuf),
    #[error("failed to read glab config: {0}")]
    Io(#[from] std::io::Error),
    #[error("no GitLab hosts in glab config")]
    NoHosts,
    #[error("invalid GitLab host configuration: {0}")]
    InvalidHost(String),
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("glab-cli").join("config.yml"))
}

pub fn load() -> Result<GlabConfig, ConfigError> {
    if let Some(primary) = config_path() {
        if primary.exists() {
            return load_from(&primary);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let fallback = home.join(".config/glab-cli/config.yml");
        if fallback.exists() {
            return load_from(&fallback);
        }
        return Err(ConfigError::Missing(fallback));
    }
    Err(ConfigError::Missing(PathBuf::from("glab-cli/config.yml")))
}

pub fn load_from(path: &Path) -> Result<GlabConfig, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::Missing(path.to_path_buf()));
    }
    parse(&fs::read_to_string(path)?)
}

pub fn pick_host(cfg: &GlabConfig, preferred: Option<&str>) -> Option<HostAuth> {
    preferred
        .and_then(|name| cfg.hosts.iter().find(|host| host.hostname == name).cloned())
        .or_else(|| {
            cfg.default_host.as_ref().and_then(|name| {
                cfg.hosts.iter().find(|host| &host.hostname == name).cloned()
            })
        })
        .or_else(|| cfg.hosts.first().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cfg() -> GlabConfig {
        parse("host: gitlab.com\nhosts:\n  gitlab.com:\n    api_protocol: https\n    api_host: gitlab.com\n    token: \"glpat-test-a\"\n    user: !!null alice\n  gitlab.example.com:\n    token: glpat-test-b\n  empty.example:\n    token:\n").unwrap()
    }

    #[test]
    fn parses_two_hosts_and_ignores_empty_token() {
        let cfg = sample_cfg();
        assert_eq!(cfg.default_host.as_deref(), Some("gitlab.com"));
        assert_eq!(cfg.hosts.len(), 2);
        assert_eq!(cfg.hosts[0].hostname, "gitlab.com");
        assert_eq!(cfg.hosts[0].user.as_deref(), Some("alice"));
        assert_eq!(cfg.hosts[1].api_base().unwrap(), "https://gitlab.example.com/api/v4");
    }

    #[test]
    fn debug_redacts_token() {
        let cfg = sample_cfg();
        let dbg = format!("{:?}", cfg.hosts[0]);
        assert!(dbg.contains("[REDACTED]"));
        assert!(!dbg.contains("glpat-test-a"));
    }

    #[test]
    fn validates_api_base_and_origin() {
        let mut host = sample_cfg().hosts[0].clone();
        host.api_protocol = "http".into();
        assert!(host.api_base().is_err());

        host.api_host = "127.0.0.1:8080".into();
        assert_eq!(host.api_base().unwrap(), "http://127.0.0.1:8080/api/v4");

        let mut other = host.clone();
        other.api_host = "127.0.0.1:9090".into();
        assert_ne!(host.origin().unwrap(), other.origin().unwrap());
    }

    #[test]
    fn pick_host_prefers_saved_then_default_then_first() {
        let cfg = sample_cfg();
        assert_eq!(pick_host(&cfg, None).unwrap().hostname, "gitlab.com");
        assert_eq!(pick_host(&cfg, Some("gitlab.example.com")).unwrap().hostname, "gitlab.example.com");
        assert_eq!(pick_host(&cfg, Some("missing")).unwrap().hostname, "gitlab.com");
    }
}
