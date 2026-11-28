use std::collections::HashSet;

use serde::Deserialize;

use super::types::{Pipeline, User};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PipelineDetail {
    pub id: u64,
    #[serde(default)]
    pub iid: Option<u64>,
    #[serde(default)]
    pub status: String,
    #[serde(default, rename = "ref")]
    pub git_ref: String,
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub web_url: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub queued_duration: Option<f64>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub tag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PipelineScope {
    #[default]
    All,
    Finished,
    Branches,
    Tags,
}

impl PipelineScope {
    pub const ALL: [Self; 4] = [Self::All, Self::Finished, Self::Branches, Self::Tags];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Finished => "Finished",
            Self::Branches => "Branches",
            Self::Tags => "Tags",
        }
    }

    pub fn matches(self, p: &Pipeline) -> bool {
        match self {
            Self::All => true,
            Self::Finished => matches!(p.status.as_str(), "success" | "failed" | "canceled" | "cancelled" | "skipped"),
            Self::Branches => !is_mr_ref(&p.git_ref) && !is_likely_tag(&p.git_ref),
            Self::Tags => is_likely_tag(&p.git_ref),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PipelineDetailTab {
    #[default]
    Pipeline,
    Jobs,
    Tests,
}

impl PipelineDetailTab {
    pub const ALL: [Self; 3] = [Self::Pipeline, Self::Jobs, Self::Tests];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pipeline => "Pipeline",
            Self::Jobs => "Jobs",
            Self::Tests => "Tests",
        }
    }
}

pub fn is_mr_ref(git_ref: &str) -> bool {
    git_ref.starts_with("refs/merge-requests/")
}

pub fn is_likely_tag(git_ref: &str) -> bool {
    let r = git_ref.trim();
    if let Some(rest) = r.strip_prefix('v') {
        rest.chars().next().is_some_and(|c| c.is_ascii_digit())
    } else {
        r.chars().next().is_some_and(|c| c.is_ascii_digit()) && r.contains('.')
    }
}

pub fn format_pipeline_ref(git_ref: &str) -> String {
    if let Some(rest) = git_ref.strip_prefix("refs/merge-requests/")
        && let Some(iid) = rest.split('/').next()
        && !iid.is_empty()
    {
        return format!("!{iid}");
    }
    git_ref.to_string()
}

pub fn pipeline_status_label(status: &str) -> &'static str {
    match status {
        "success" | "passed" => "Passed",
        "failed" => "Failed",
        "running" => "Running",
        "pending" => "Pending",
        "created" => "Created",
        "preparing" => "Preparing",
        "waiting_for_resource" | "waiting_for_callback" => "Waiting",
        "canceling" => "Canceling",
        "canceled" | "cancelled" => "Canceled",
        "manual" => "Manual",
        "skipped" => "Skipped",
        _ => "Unknown",
    }
}

pub fn format_hms_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub fn latest_pipeline_ids_by_ref(pipelines: &[Pipeline]) -> HashSet<u64> {
    let mut seen_refs = HashSet::new();
    let mut latest = HashSet::new();
    for p in pipelines {
        if seen_refs.insert(p.git_ref.as_str()) {
            latest.insert(p.id);
        }
    }
    latest
}

pub fn pipeline_kind_tag(p: &Pipeline) -> &'static str {
    if is_mr_ref(&p.git_ref) || p.source.as_deref() == Some("merge_request_event") {
        "merge request"
    } else if is_likely_tag(&p.git_ref) {
        "tag"
    } else {
        "branch"
    }
}

pub fn filter_pipelines(pipelines: &[Pipeline], scope: PipelineScope, query: &str) -> Vec<Pipeline> {
    let q = query.trim().to_ascii_lowercase();
    pipelines
        .iter()
        .filter(|p| {
            scope.matches(p)
                && (q.is_empty()
                    || p.id.to_string().contains(&q)
                    || p.git_ref.to_ascii_lowercase().contains(&q)
                    || p.sha.to_ascii_lowercase().contains(&q)
                    || p.status.to_ascii_lowercase().contains(&q)
                    || p.source.as_deref().is_some_and(|s| s.to_ascii_lowercase().contains(&q)))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: u64, status: &str, git_ref: &str, source: Option<&str>) -> Pipeline {
        Pipeline {
            id, iid: Some(id), status: status.into(), git_ref: git_ref.into(),
            sha: "abcdef123456".into(), web_url: String::new(), created_at: None, updated_at: None,
            source: source.map(str::to_owned),
        }
    }

    #[test]
    fn formats_refs_durations_scopes_and_latest_tags() {
        assert_eq!(format_pipeline_ref("refs/merge-requests/247/head"), "!247");
        assert_eq!(format_pipeline_ref("main"), "main");
        assert_eq!(format_hms_duration(392), "00:06:32");
        assert_eq!(pipeline_status_label("success"), "Passed");

        let pipes = vec![
            mk(3, "running", "refs/merge-requests/247/head", Some("merge_request_event")),
            mk(2, "success", "v1.1.0", Some("push")),
            mk(1, "success", "main", Some("push")),
        ];
        assert_eq!(pipeline_kind_tag(&pipes[0]), "merge request");
        assert_eq!(pipeline_kind_tag(&pipes[1]), "tag");
        assert_eq!(pipeline_kind_tag(&pipes[2]), "branch");
        assert_eq!(filter_pipelines(&pipes, PipelineScope::Finished, "").len(), 2);
        assert_eq!(filter_pipelines(&pipes, PipelineScope::Tags, "").len(), 1);
        assert_eq!(filter_pipelines(&pipes, PipelineScope::Branches, "main").len(), 1);
        assert!(latest_pipeline_ids_by_ref(&pipes).contains(&3));
    }
}
