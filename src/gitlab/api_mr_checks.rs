use reqwest::Method;
use serde::Deserialize;
use serde_json::json;
use url::Url;

use super::client::GitLab;
use super::error::{GitLabError, Result};
use super::http_util::send_with_redirects;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MergeabilityCheck {
    pub identifier: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MrMergeabilitySnapshot {
    pub conflicts: Option<bool>,
    pub detailed_merge_status: Option<String>,
    pub checks: Vec<MergeabilityCheck>,
}

const MR_MERGEABILITY_QUERY: &str = r#"
query($projectPath: ID!, $iid: String!) {
  project(fullPath: $projectPath) {
    mergeRequest(iid: $iid) {
      conflicts
      detailedMergeStatus
      mergeabilityChecks {
        identifier
        status
      }
    }
  }
}
"#;

#[derive(Deserialize)]
struct GqlResponse {
    #[serde(default)]
    data: Option<GqlData>,
    #[serde(default)]
    errors: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct GqlData {
    project: Option<GqlProject>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlProject {
    merge_request: Option<GqlMergeRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlMergeRequest {
    #[serde(default)]
    conflicts: Option<bool>,
    #[serde(default)]
    detailed_merge_status: Option<String>,
    #[serde(default)]
    mergeability_checks: Vec<MergeabilityCheck>,
}

pub(crate) fn graphql_url_from_api_base(api_base: &str) -> Result<Url> {
    let trimmed = api_base.trim_end_matches('/');
    let gql = trimmed
        .strip_suffix("/v4")
        .map(|prefix| format!("{prefix}/graphql"))
        .unwrap_or_else(|| format!("{trimmed}/graphql"));
    Url::parse(&gql).map_err(|err| GitLabError::Message(format!("invalid GraphQL URL: {err}")))
}

pub(crate) fn parse_mergeability_response(bytes: &[u8]) -> Option<MrMergeabilitySnapshot> {
    let parsed: GqlResponse = serde_json::from_slice(bytes).ok()?;
    if !parsed.errors.is_empty() {
        return None;
    }
    let mr = parsed.data?.project?.merge_request?;
    Some(MrMergeabilitySnapshot {
        conflicts: mr.conflicts,
        detailed_merge_status: mr.detailed_merge_status,
        checks: mr.mergeability_checks,
    })
}

impl GitLab {
    pub async fn mr_mergeability_checks(
        &self,
        project_path: &str,
        iid: u64,
    ) -> Option<MrMergeabilitySnapshot> {
        let url = graphql_url_from_api_base(&self.api_base).ok()?;
        let body = json!({
            "query": MR_MERGEABILITY_QUERY,
            "variables": { "projectPath": project_path, "iid": iid.to_string() }
        });
        let payload = serde_json::to_vec(&body).ok()?;
        let response = send_with_redirects(
            &self.http,
            Method::POST,
            url,
            &self.origin,
            &self.auth.token,
            Some(payload),
            &[],
        )
        .await
        .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        parse_mergeability_response(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_graphql_url_and_parses_mergeability_checks() {
        assert_eq!(
            graphql_url_from_api_base("https://gitlab.example.com/api/v4").unwrap().as_str(),
            "https://gitlab.example.com/api/graphql"
        );
        assert_eq!(
            graphql_url_from_api_base("https://gitlab.example.com/sub/api/v4/").unwrap().as_str(),
            "https://gitlab.example.com/sub/api/graphql"
        );

        let raw = br#"{
          "data": {
            "project": {
              "mergeRequest": {
                "conflicts": true,
                "detailedMergeStatus": "DISCUSSIONS_NOT_RESOLVED",
                "mergeabilityChecks": [
                  { "identifier": "DISCUSSIONS_NOT_RESOLVED", "status": "FAILED" },
                  { "identifier": "CONFLICT", "status": "FAILED" },
                  { "identifier": "CI_MUST_PASS", "status": "SUCCESS" }
                ]
              }
            }
          }
        }"#;
        let snap = parse_mergeability_response(raw).expect("valid snapshot");
        assert_eq!(snap.conflicts, Some(true));
        assert_eq!(snap.checks.len(), 3);
        assert_eq!(snap.checks[1].identifier, "CONFLICT");

        let err_raw = br#"{"errors":[{"message":"Field 'mergeabilityChecks' doesn't exist"}]}"#;
        assert!(parse_mergeability_response(err_raw).is_none());
    }
}
