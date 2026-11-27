use serde::Deserialize;

use super::mr_can_review;
use super::types_mr::{MergeRequest, MrApprovals};
use super::types_project::ProjectDetail;

pub const ACCESS_NONE: u32 = 0;
pub const ACCESS_MINIMAL: u32 = 5;
pub const ACCESS_GUEST: u32 = 10;
pub const ACCESS_PLANNER: u32 = 15;
pub const ACCESS_REPORTER: u32 = 20;
pub const ACCESS_DEVELOPER: u32 = 30;
pub const ACCESS_MAINTAINER: u32 = 40;
pub const ACCESS_OWNER: u32 = 50;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ProjectAccess {
    #[serde(default)]
    pub access_level: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ProjectPermissionsRaw {
    #[serde(default)]
    pub project_access: Option<ProjectAccess>,
    #[serde(default)]
    pub group_access: Option<ProjectAccess>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MrUserPermissions {
    #[serde(default)]
    pub can_merge: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectPermissions {
    pub loaded: bool,
    pub access_level: u32,
    pub archived: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MrPermissions {
    pub can_merge: bool,
    pub can_approve: bool,
    pub can_unapprove: bool,
    pub can_edit: bool,
    pub can_close: bool,
    pub can_reopen: bool,
    pub can_rebase: bool,
    pub can_edit_metadata: bool,
    pub can_resolve_discussions: bool,
    pub can_comment: bool,
    pub can_todo: bool,
}

impl ProjectPermissions {
    pub const fn unknown() -> Self {
        Self { loaded: false, access_level: ACCESS_NONE, archived: false }
    }

    pub const fn with_level(access_level: u32, archived: bool) -> Self {
        Self { loaded: true, access_level, archived }
    }

    pub fn from_detail(detail: &ProjectDetail) -> Self {
        let project_level = detail
            .permissions
            .as_ref()
            .and_then(|p| p.project_access.as_ref())
            .map(|a| a.access_level)
            .unwrap_or(ACCESS_NONE);
        let group_level = detail
            .permissions
            .as_ref()
            .and_then(|p| p.group_access.as_ref())
            .map(|a| a.access_level)
            .unwrap_or(ACCESS_NONE);
        Self {
            loaded: true,
            access_level: project_level.max(group_level),
            archived: detail.archived,
        }
    }

    pub const fn is_read_only(&self) -> bool {
        !self.loaded || self.archived
    }

    pub const fn can_comment(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_GUEST
    }

    pub const fn can_todo(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_GUEST
    }

    pub const fn can_create_issue(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_GUEST
    }

    pub const fn can_manage_issue(&self, is_author: bool) -> bool {
        !self.is_read_only() && (self.access_level >= ACCESS_REPORTER || is_author)
    }

    pub const fn can_push_code(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_DEVELOPER
    }

    pub const fn can_create_mr(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_DEVELOPER
    }

    pub const fn can_bulk_edit_mrs(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_DEVELOPER
    }

    pub const fn can_run_pipelines(&self) -> bool {
        !self.is_read_only() && self.access_level >= ACCESS_DEVELOPER
    }

    pub fn mr_permissions(
        &self,
        mr: &MergeRequest,
        approvals: Option<&MrApprovals>,
        current_user_id: Option<u64>,
    ) -> MrPermissions {
        if self.is_read_only() {
            return MrPermissions::default();
        }
        let open = mr_can_review(&mr.state);
        let is_author = current_user_id.is_some()
            && mr.author.as_ref().map(|a| a.id) == current_user_id;
        let can_edit = self.access_level >= ACCESS_DEVELOPER || is_author;
        let can_merge = open
            && mr.user.as_ref().and_then(|u| u.can_merge) == Some(true);
        let can_approve = open
            && approvals.is_some_and(|a| a.user_can_approve && !a.user_has_approved);
        let can_unapprove = open
            && approvals.is_some_and(|a| a.user_has_approved);
        MrPermissions {
            can_merge,
            can_approve,
            can_unapprove,
            can_edit,
            can_close: open && can_edit,
            can_reopen: mr.state == "closed" && can_edit,
            can_rebase: open && can_edit,
            can_edit_metadata: can_edit,
            can_resolve_discussions: can_edit,
            can_comment: self.can_comment(),
            can_todo: self.can_todo(),
        }
    }
}
