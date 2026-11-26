use std::time::Duration;

use reqwest::{redirect, Client, Method};
use serde::de::DeserializeOwned;
use url::{Origin, Url};

use crate::config::HostAuth;

use super::error::{GitLabError, Result};
use super::http_util::send_with_redirects;

#[derive(Clone)]
pub struct GitLab {
    pub(super) http: Client,
    pub(super) stream_http: Client,
    pub(super) auth: HostAuth,
    pub(super) api_base: String,
    pub(super) origin: Origin,
}

impl GitLab {
    pub fn new(auth: HostAuth) -> Result<Self> {
        let api_url = auth
            .api_base_url()
            .map_err(|err| GitLabError::Message(err.to_string()))?;
        let origin = api_url.origin();
        let api_base = api_url.as_str().trim_end_matches('/').to_string();
        let http = Client::builder()
            .user_agent("gitlab-for-mac/0.1")
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        let stream_http = Client::builder()
            .user_agent("gitlab-for-mac/0.1")
            .redirect(redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            http,
            stream_http,
            auth,
            api_base,
            origin,
        })
    }

    pub fn host(&self) -> &str {
        &self.auth.hostname
    }

    pub fn auth(&self) -> &HostAuth {
        &self.auth
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    pub fn user(&self) -> Option<&str> {
        self.auth.user.as_deref()
    }

    pub(super) fn url(&self, path: &str) -> Result<Url> {
        let raw = format!("{}{path}", self.api_base);
        Url::parse(&raw).map_err(|err| GitLabError::Message(format!("invalid API URL: {err}")))
    }

    pub(super) async fn send<T: DeserializeOwned>(&self, method: Method, path: &str) -> Result<T> {
        self.send_body(method, path, Option::<serde_json::Value>::None)
            .await
    }

    pub(super) async fn send_body<T, B>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
        B: serde::Serialize,
    {
        let payload = body
            .map(|value| serde_json::to_vec(&value))
            .transpose()
            .map_err(|err| GitLabError::Message(err.to_string()))?;
        let response = send_with_redirects(
            &self.http,
            method,
            self.url(path)?,
            &self.origin,
            &self.auth.token,
            payload,
            &[],
        )
        .await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(GitLabError::from_status(
                status,
                String::from_utf8_lossy(&bytes).into_owned(),
            ));
        }
        serde_json::from_slice(&bytes).map_err(|err| GitLabError::Message(err.to_string()))
    }

    pub(super) async fn send_empty(&self, method: Method, path: &str) -> Result<()> {
        let response = send_with_redirects(
            &self.http,
            method,
            self.url(path)?,
            &self.origin,
            &self.auth.token,
            None,
            &[],
        )
        .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GitLabError::from_status(status, body));
        }
        Ok(())
    }
}
