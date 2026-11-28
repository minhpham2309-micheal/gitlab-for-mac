use reqwest::Method;

use super::client::GitLab;
use super::error::Result;
use super::types::Project;
use super::types_group::Group;

pub(crate) fn encode_group_path(full_path: &str) -> String {
    urlencoding::encode(full_path.trim_matches('/')).into_owned()
}

pub(crate) fn filter_shared_projects(group_full_path: &str, projects: Vec<Project>) -> Vec<Project> {
    let prefix = format!("{}/", group_full_path.trim_matches('/').to_ascii_lowercase());
    projects
        .into_iter()
        .filter(|p| !p.path_with_namespace.to_ascii_lowercase().starts_with(&prefix))
        .collect()
}

impl GitLab {
    pub async fn top_level_groups(&self, search: Option<&str>) -> Result<Vec<Group>> {
        let q = search.map(str::trim).filter(|s| !s.is_empty());
        let path = match q {
            Some(term) => format!(
                "/groups?search={}&all_available=true&order_by=name&sort=asc&per_page=100",
                urlencoding::encode(term)
            ),
            None => "/groups?top_level_only=true&all_available=true&order_by=name&sort=asc&per_page=100".into(),
        };
        self.send(Method::GET, &path).await
    }

    pub async fn group_by_path(&self, full_path: &str) -> Result<Group> {
        let enc = encode_group_path(full_path);
        self.send(Method::GET, &format!("/groups/{enc}?with_projects=false"))
            .await
    }

    pub async fn group_subgroups(&self, group_id: u64, search: Option<&str>) -> Result<Vec<Group>> {
        let extra = search
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("&search={}", urlencoding::encode(s)))
            .unwrap_or_default();
        self.send(
            Method::GET,
            &format!("/groups/{group_id}/subgroups?all_available=true&order_by=name&sort=asc&per_page=100{extra}"),
        )
        .await
    }

    pub async fn group_projects(
        &self,
        group_id: u64,
        group_full_path: &str,
        shared_only: bool,
        archived: bool,
        search: Option<&str>,
    ) -> Result<Vec<Project>> {
        let extra = search
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("&search={}", urlencoding::encode(s)))
            .unwrap_or_default();
        let with_shared = if shared_only { "true" } else { "false" };
        let arch = if archived { "true" } else { "false" };
        let items: Vec<Project> = self
            .send(
                Method::GET,
                &format!(
                    "/groups/{group_id}/projects?with_shared={with_shared}&include_subgroups=false&simple=true&archived={arch}&order_by=name&sort=asc&per_page=100{extra}"
                ),
            )
            .await?;
        if shared_only {
            Ok(filter_shared_projects(group_full_path, items))
        } else {
            Ok(items)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::gitlab::types_group::{
        build_group_tree_rows, GroupChildren, GroupSortField, GroupTreeRow,
    };
    use crate::gitlab::SortDir;

    #[test]
    fn encodes_nested_group_path_and_filters_shared_projects() {
        assert_eq!(encode_group_path("/acme/backend/game/"), "acme%2Fbackend%2Fgame");
        let direct = Project {
            id: 1, name: "svc-a".into(), path_with_namespace: "acme/backend/svc-a".into(),
            default_branch: Some("main".into()), web_url: "https://gitlab.example.com/acme/backend/svc-a".into(),
            last_activity_at: None, star_count: 1, avatar_url: None,
        };
        let shared = Project {
            id: 2, name: "shared-lib".into(), path_with_namespace: "platform/shared-lib".into(),
            default_branch: Some("main".into()), web_url: "https://gitlab.example.com/platform/shared-lib".into(),
            last_activity_at: None, star_count: 2, avatar_url: None,
        };
        let out = filter_shared_projects("acme/backend", vec![direct, shared]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, 2);
    }

    #[test]
    fn deserializes_group_and_builds_expanded_hierarchy_rows() {
        let g: Group = serde_json::from_str(
            r#"{"id":10,"name":"game","path":"game","full_name":"Acme / backend / game","full_path":"acme/backend/game","web_url":"https://gitlab.example.com/acme/backend/game","visibility":"private","created_at":"2026-01-10T00:00:00Z"}"#,
        ).unwrap();
        assert_eq!(g.display_name(), "game");
        assert_eq!(g.visibility_label(), "Private");
        assert_eq!(g.parent_full_path().as_deref(), Some("acme/backend"));

        let child_proj = Project {
            id: 99, name: "engine".into(), path_with_namespace: "acme/backend/game/engine".into(),
            default_branch: Some("main".into()), web_url: "https://gitlab.example.com/acme/backend/game/engine".into(),
            last_activity_at: None, star_count: 4, avatar_url: None,
        };
        let mut cache = HashMap::new();
        cache.insert(10, GroupChildren { subgroups: vec![], projects: vec![child_proj] });
        let mut expanded = HashSet::new();
        expanded.insert(10);

        let rows = build_group_tree_rows(
            &[g], &[], &cache, &expanded, &HashSet::new(), "", GroupSortField::Name, SortDir::Asc,
        );
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], GroupTreeRow::Subgroup { depth: 0, expanded: true, cached_projects: Some(1), .. }));
        assert!(matches!(rows[1], GroupTreeRow::Project { depth: 1, .. }));
    }
}
