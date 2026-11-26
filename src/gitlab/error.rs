use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum GitLabError {
    #[error("{0}")]
    Message(String),
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
}

impl From<reqwest::Error> for GitLabError {
    fn from(err: reqwest::Error) -> Self {
        GitLabError::Message(err.without_url().to_string())
    }
}

impl GitLabError {
    pub fn from_status(status: reqwest::StatusCode, body: String) -> Self {
        let snippet: String = body.chars().take(240).collect();
        GitLabError::Http {
            status: status.as_u16(),
            body: snippet,
        }
    }
}

pub type Result<T> = std::result::Result<T, GitLabError>;
