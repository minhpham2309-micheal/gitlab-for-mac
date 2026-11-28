use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use super::types::Project;
use super::types_mr::SortDir;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct Group {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub full_path: String,
    #[serde(default)]
    pub web_url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub marked_for_deletion_on: Option<String>,
}

impl Group {
    pub fn display_name(&self) -> &str {
        if !self.name.trim().is_empty() {
            &self.name
        } else if !self.path.trim().is_empty() {
            &self.path
        } else {
            &self.full_path
        }
    }

    pub fn visibility_label(&self) -> &'static str {
        match self.visibility.as_deref() {
            Some("public") => "Public",
            Some("internal") => "Internal",
            _ => "Private",
        }
    }

    pub fn is_inactive(&self) -> bool {
        self.marked_for_deletion_on.is_some()
    }

    pub fn parent_full_path(&self) -> Option<String> {
        let trimmed = self.full_path.trim_matches('/');
        trimmed.rsplit_once('/').map(|(parent, _)| parent.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupTab {
    #[default]
    SubgroupsAndProjects,
    SharedProjects,
    Inactive,
}

impl GroupTab {
    pub const ALL: [Self; 3] = [Self::SubgroupsAndProjects, Self::SharedProjects, Self::Inactive];

    pub fn label(self) -> &'static str {
        match self {
            Self::SubgroupsAndProjects => "Subgroups and projects",
            Self::SharedProjects => "Shared projects",
            Self::Inactive => "Inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupSortField {
    #[default]
    Name,
    CreatedAt,
    UpdatedAt,
}

impl GroupSortField {
    pub const ALL: [Self; 3] = [Self::Name, Self::CreatedAt, Self::UpdatedAt];

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::CreatedAt => "Created date",
            Self::UpdatedAt => "Updated date",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GroupChildren {
    pub subgroups: Vec<Group>,
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone)]
pub enum GroupTreeRow {
    Subgroup {
        group: Group,
        depth: usize,
        expanded: bool,
        loading: bool,
        cached_subgroups: Option<usize>,
        cached_projects: Option<usize>,
    },
    Project {
        project: Project,
        depth: usize,
    },
}

fn cmp_opt_str(a: Option<&str>, b: Option<&str>) -> std::cmp::Ordering {
    a.unwrap_or("").cmp(b.unwrap_or(""))
}

fn sort_groups(items: &mut [Group], field: GroupSortField, dir: SortDir) {
    items.sort_by(|a, b| {
        let ord = match field {
            GroupSortField::Name => a.display_name().to_ascii_lowercase().cmp(&b.display_name().to_ascii_lowercase()),
            GroupSortField::CreatedAt | GroupSortField::UpdatedAt => cmp_opt_str(a.created_at.as_deref(), b.created_at.as_deref()),
        };
        if dir == SortDir::Desc { ord.reverse() } else { ord }
    });
}

fn sort_projects(items: &mut [Project], field: GroupSortField, dir: SortDir) {
    items.sort_by(|a, b| {
        let ord = match field {
            GroupSortField::Name => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
            GroupSortField::CreatedAt | GroupSortField::UpdatedAt => cmp_opt_str(a.last_activity_at.as_deref(), b.last_activity_at.as_deref()),
        };
        if dir == SortDir::Desc { ord.reverse() } else { ord }
    });
}

fn group_matches(g: &Group, q: &str) -> bool {
    q.is_empty()
        || g.name.to_ascii_lowercase().contains(q)
        || g.full_path.to_ascii_lowercase().contains(q)
        || g.description.as_deref().is_some_and(|d| d.to_ascii_lowercase().contains(q))
}

fn project_matches(p: &Project, q: &str) -> bool {
    q.is_empty()
        || p.name.to_ascii_lowercase().contains(q)
        || p.path_with_namespace.to_ascii_lowercase().contains(q)
}

#[allow(clippy::too_many_arguments)]
pub fn build_group_tree_rows(
    subgroups: &[Group],
    projects: &[Project],
    children_cache: &HashMap<u64, GroupChildren>,
    expanded: &HashSet<u64>,
    loading: &HashSet<u64>,
    query: &str,
    sort_field: GroupSortField,
    sort_dir: SortDir,
) -> Vec<GroupTreeRow> {
    let q = query.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    append_level(
        subgroups, projects, children_cache, expanded, loading, &q, sort_field, sort_dir, 0, &mut out,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn append_level(
    subgroups: &[Group],
    projects: &[Project],
    children_cache: &HashMap<u64, GroupChildren>,
    expanded: &HashSet<u64>,
    loading: &HashSet<u64>,
    q: &str,
    sort_field: GroupSortField,
    sort_dir: SortDir,
    depth: usize,
    out: &mut Vec<GroupTreeRow>,
) {
    let mut gs: Vec<Group> = subgroups.iter().filter(|g| group_matches(g, q)).cloned().collect();
    sort_groups(&mut gs, sort_field, sort_dir);
    for g in gs {
        let gid = g.id;
        let is_exp = expanded.contains(&gid);
        let is_loading = loading.contains(&gid);
        let cached = children_cache.get(&gid);
        out.push(GroupTreeRow::Subgroup {
            group: g,
            depth,
            expanded: is_exp,
            loading: is_loading,
            cached_subgroups: cached.map(|c| c.subgroups.len()),
            cached_projects: cached.map(|c| c.projects.len()),
        });
        if is_exp && let Some(ch) = cached {
            append_level(
                &ch.subgroups, &ch.projects, children_cache, expanded, loading, q, sort_field, sort_dir,
                depth + 1, out,
            );
        }
    }

    let mut ps: Vec<Project> = projects.iter().filter(|p| project_matches(p, q)).cloned().collect();
    sort_projects(&mut ps, sort_field, sort_dir);
    for project in ps {
        out.push(GroupTreeRow::Project { project, depth });
    }
}
