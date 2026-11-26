#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Projects,
    ProjectOverview,
    Repository,
    Pipelines,
    MergeRequests,
    Branches,
    Tags,
    Commits,
    Issues,
    Todos,
    Releases,
    Schedules,
    Runners,
    Search,
    Console,
    Preferences,
}

impl Screen {
    pub const ALL: [Screen; 16] = [
        Screen::Projects, Screen::ProjectOverview, Screen::Repository, Screen::Pipelines,
        Screen::MergeRequests, Screen::Branches, Screen::Tags, Screen::Commits,
        Screen::Issues, Screen::Todos, Screen::Releases, Screen::Schedules,
        Screen::Runners, Screen::Search, Screen::Console, Screen::Preferences,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Screen::Projects => "Projects",
            Screen::ProjectOverview => "Project overview",
            Screen::Repository => "Repository",
            Screen::Pipelines => "Pipelines",
            Screen::MergeRequests => "Merge requests",
            Screen::Branches => "Branches",
            Screen::Tags => "Tags",
            Screen::Commits => "Commits",
            Screen::Issues => "Issues",
            Screen::Todos => "To-Do List",
            Screen::Releases => "Releases",
            Screen::Schedules => "Pipeline schedules",
            Screen::Runners => "Runners",
            Screen::Search => "Code search",
            Screen::Console => "glab console",
            Screen::Preferences => "Preferences",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Screen::Projects => "projects",
            Screen::ProjectOverview => "overview",
            Screen::Repository => "repository",
            Screen::Pipelines => "pipelines",
            Screen::MergeRequests => "merge_requests",
            Screen::Branches => "branches",
            Screen::Tags => "tags",
            Screen::Commits => "commits",
            Screen::Issues => "issues",
            Screen::Todos => "todos",
            Screen::Releases => "releases",
            Screen::Schedules => "schedules",
            Screen::Runners => "runners",
            Screen::Search => "search",
            Screen::Console => "console",
            Screen::Preferences => "preferences",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    Plan,
    Code,
    Build,
    Deploy,
}

impl NavSection {
    pub const ALL: [Self; 4] = [Self::Plan, Self::Code, Self::Build, Self::Deploy];

    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::Code => "Code",
            Self::Build => "Build",
            Self::Deploy => "Deploy",
        }
    }

    pub fn screens(self) -> &'static [Screen] {
        match self {
            Self::Plan => &[Screen::Issues],
            Self::Code => &[
                Screen::Repository, Screen::MergeRequests, Screen::Branches,
                Screen::Tags, Screen::Commits, Screen::Search,
            ],
            Self::Build => &[Screen::Pipelines, Screen::Schedules, Screen::Runners],
            Self::Deploy => &[Screen::Releases],
        }
    }

    pub fn default_screen(self) -> Screen { self.screens()[0] }
    pub fn contains(self, screen: Screen) -> bool { self.screens().contains(&screen) }
    pub fn bit(self) -> u8 { 1 << self as u8 }
    pub fn of(screen: Screen) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.contains(screen))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    Manage, Plan, Code, Build, Secure, Deploy, Operate, Monitor, Analyze,
}

impl SidebarSection {
    pub const ALL: [Self; 9] = [
        Self::Manage, Self::Plan, Self::Code, Self::Build, Self::Secure,
        Self::Deploy, Self::Operate, Self::Monitor, Self::Analyze,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Manage => "Manage",
            Self::Plan => "Plan",
            Self::Code => "Code",
            Self::Build => "Build",
            Self::Secure => "Secure",
            Self::Deploy => "Deploy",
            Self::Operate => "Operate",
            Self::Monitor => "Monitor",
            Self::Analyze => "Analyze",
        }
    }

    pub fn native(self) -> Option<NavSection> {
        match self {
            Self::Plan => Some(NavSection::Plan),
            Self::Code => Some(NavSection::Code),
            Self::Build => Some(NavSection::Build),
            Self::Deploy => Some(NavSection::Deploy),
            _ => None,
        }
    }

    pub fn web_subpath(self, default_branch: &str) -> Option<String> {
        let br = if default_branch.trim().is_empty() { "main" } else { default_branch };
        match self {
            Self::Manage => Some("/-/project_members".into()),
            Self::Secure => Some("/-/security/discover".into()),
            Self::Operate => Some("/-/environments".into()),
            Self::Monitor => Some("/-/alerts".into()),
            Self::Analyze => Some(format!("/-/graphs/{}", urlencoding::encode(br))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitlab_nav_tree_groups_screens_and_nine_sidebar_sections() {
        assert_eq!(SidebarSection::ALL.len(), 9);
        assert_eq!(NavSection::of(Screen::Repository), Some(NavSection::Code));
        assert_eq!(NavSection::of(Screen::Commits), Some(NavSection::Code));
        assert_eq!(NavSection::of(Screen::ProjectOverview), None);
        assert_eq!(SidebarSection::Manage.web_subpath("main").as_deref(), Some("/-/project_members"));
        assert_eq!(SidebarSection::Analyze.web_subpath("dev").as_deref(), Some("/-/graphs/dev"));
        for screen in Screen::ALL {
            assert_eq!(Screen::from_key(screen.key()), Some(screen));
            let grouped = NavSection::of(screen).is_some();
            let ungrouped = matches!(
                screen,
                Screen::Projects | Screen::ProjectOverview | Screen::Todos | Screen::Console | Screen::Preferences
            );
            assert_eq!(grouped, !ungrouped, "{screen:?}");
        }
    }
}
