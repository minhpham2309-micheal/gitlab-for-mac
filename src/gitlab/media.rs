use std::net::IpAddr;

use reqwest::Method;
use url::{Host, Url};

use crate::config::{is_loopback_host, HostAuth};

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::http_util::send_with_redirects_checked;

pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct FetchedImage {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

pub fn is_blocked_local_domain(domain: &str) -> bool {
    let trimmed = domain.trim_end_matches('.').to_ascii_lowercase();
    trimmed == "localhost" || trimmed.ends_with(".localhost")
}

pub fn is_private_or_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 64)
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_private_or_local_ip(IpAddr::V4(mapped));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || ((v6.segments()[0] & 0xfe00) == 0xfc00)
                || ((v6.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

pub fn should_send_token(url: &Url, auth: &HostAuth) -> bool {
    let Ok(auth_origin) = auth.origin() else {
        return false;
    };
    if url.origin() != auth_origin {
        return false;
    }
    match url.scheme() {
        "https" => true,
        "http" => is_loopback_host(url),
        _ => false,
    }
}

pub fn is_safe_media_target(url: &Url, auth: &HostAuth) -> bool {
    if !matches!(url.scheme(), "https" | "http") {
        return false;
    }
    if should_send_token(url, auth) {
        return true;
    }
    match url.host() {
        Some(Host::Domain(d)) => !is_blocked_local_domain(d),
        Some(Host::Ipv4(ip)) => !is_private_or_local_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => !is_private_or_local_ip(IpAddr::V6(ip)),
        None => false,
    }
}

fn valid_upload_segment(seg: &str) -> bool {
    !seg.is_empty()
        && seg != "."
        && seg != ".."
        && !seg.contains('/')
        && !seg.contains('\\')
        && !seg.contains('\0')
        && !seg.contains('?')
        && !seg.contains('#')
}

pub fn resolve_upload_url(raw_src: &str, project_id: Option<u64>, auth: &HostAuth) -> Option<Url> {
    let trimmed = raw_src.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("/uploads/") {
        let (secret, filename) = rest.split_once('/')?;
        if let Some(pid) = project_id {
            if valid_upload_segment(secret) && valid_upload_segment(filename) {
                let api_base = auth.api_base().ok()?;
                let raw = format!(
                    "{api_base}/projects/{pid}/uploads/{}/{}",
                    urlencoding::encode(secret),
                    urlencoding::encode(filename)
                );
                return Url::parse(&raw).ok();
            }
        }
        let base = auth.api_base_url().ok()?;
        return base.join(trimmed).ok();
    }
    if trimmed.starts_with('/') {
        let base = auth.api_base_url().ok()?;
        return base.join(trimmed).ok();
    }
    Url::parse(trimmed).ok()
}

impl GitLab {
    pub async fn fetch_media(&self, raw_src: &str, project_id: Option<u64>) -> Result<FetchedImage> {
        let url = resolve_upload_url(raw_src, project_id, &self.auth)
            .ok_or_else(|| GitLabError::Message(format!("invalid media URL '{raw_src}'")))?;
        if !is_safe_media_target(&url, &self.auth) {
            return Err(GitLabError::Message(format!("blocked unsafe media target '{url}'")));
        }
        let auth = self.auth.clone();
        let mut response = send_with_redirects_checked(
            &self.http, Method::GET, url, &self.origin, &self.auth.token, None, &[],
            move |next| is_safe_media_target(next, &auth),
        )
        .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GitLabError::from_status(status, body));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_ascii_lowercase();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len().saturating_add(chunk.len()) > MAX_IMAGE_BYTES {
                return Err(GitLabError::Message("image exceeds 10 MB safety cap".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(FetchedImage { bytes, content_type })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_with(proto: &str, host: &str) -> HostAuth {
        HostAuth { hostname: host.into(), api_host: host.into(), api_protocol: proto.into(), token: "glpat-secret".into(), user: None }
    }

    #[test]
    fn rewrites_project_uploads_to_rest_api_v4() {
        let auth = auth_with("https", "gl.example.com:8443");
        let url = resolve_upload_url("/uploads/deadbeef/shot.png", Some(42), &auth).unwrap();
        assert_eq!(url.as_str(), "https://gl.example.com:8443/api/v4/projects/42/uploads/deadbeef/shot.png");
        assert!(should_send_token(&url, &auth));
    }

    #[test]
    fn rejects_mismatched_port_lookalike_ipv4_mapped_and_localhost_subdomains() {
        let auth = auth_with("https", "gl.example.com:8443");
        for bad_token in [
            "https://gl.example.com:9999/uploads/a/b.png",
            "https://evil-gl.example.com:8443/uploads/a/b.png",
        ] {
            assert!(!should_send_token(&Url::parse(bad_token).unwrap(), &auth));
        }
        for bad_ssrf in [
            "http://127.0.0.1:8080/admin",
            "http://192.168.1.5/img.png",
            "http://169.254.169.254/latest/meta-data",
            "http://[::ffff:127.0.0.1]/secret",
            "http://[::ffff:169.254.169.254]/meta",
            "http://sub.localhost/a.png",
            "http://localhost./a.png",
        ] {
            assert!(!is_safe_media_target(&Url::parse(bad_ssrf).unwrap(), &auth), "should block {bad_ssrf}");
        }
    }
}
