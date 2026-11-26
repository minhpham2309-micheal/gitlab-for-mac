use crate::nav::Screen;

const MAX_HISTORY: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavTarget {
    Screen,
    OverviewDir {
        git_ref: String,
        path: String,
    },
    RepoFile {
        git_ref: Option<String>,
        path: String,
        blob_sha: Option<String>,
        line: Option<u32>,
    },
    Commits {
        git_ref: Option<String>,
        path: Option<String>,
    },
    Pipeline {
        pipeline_id: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavEntry {
    pub project_id: Option<u64>,
    pub screen: Screen,
    pub target: NavTarget,
}

impl NavEntry {
    pub fn screen(project_id: Option<u64>, screen: Screen) -> Self {
        Self { project_id, screen, target: NavTarget::Screen }
    }
}

#[derive(Clone, Debug)]
pub struct NavHistory {
    back: Vec<NavEntry>,
    forward: Vec<NavEntry>,
    current: NavEntry,
}

impl NavHistory {
    pub fn new(initial: NavEntry) -> Self {
        Self { back: Vec::new(), forward: Vec::new(), current: initial }
    }

    pub fn reset(&mut self, initial: NavEntry) {
        self.back.clear();
        self.forward.clear();
        self.current = initial;
    }

    pub fn current(&self) -> &NavEntry {
        &self.current
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn push(&mut self, entry: NavEntry) -> bool {
        if self.current == entry {
            return false;
        }
        let prev = std::mem::replace(&mut self.current, entry);
        self.back.push(prev);
        if self.back.len() > MAX_HISTORY {
            self.back.remove(0);
        }
        self.forward.clear();
        true
    }

    pub fn go_back(&mut self) -> Option<NavEntry> {
        let prev = self.back.pop()?;
        let cur = std::mem::replace(&mut self.current, prev.clone());
        self.forward.push(cur);
        Some(prev)
    }

    pub fn go_forward(&mut self) -> Option<NavEntry> {
        let next = self.forward.pop()?;
        let cur = std::mem::replace(&mut self.current, next.clone());
        self.back.push(cur);
        Some(next)
    }

    pub fn last_overview_path(&self, project_id: Option<u64>) -> String {
        std::iter::once(&self.current)
            .chain(self.back.iter().rev())
            .find_map(|e| {
                if e.project_id == project_id
                    && let NavTarget::OverviewDir { path, .. } = &e.target {
                        return Some(path.clone());
                    }
                None
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_dedups_and_back_forward_round_trip() {
        let mut h = NavHistory::new(NavEntry::screen(None, Screen::Projects));
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());

        let mrs = NavEntry::screen(Some(1), Screen::MergeRequests);
        assert!(h.push(mrs.clone()));
        assert!(!h.push(mrs.clone()));
        assert!(h.can_go_back());
        assert!(!h.can_go_forward());

        let back = h.go_back().unwrap();
        assert_eq!(back.screen, Screen::Projects);
        assert!(!h.can_go_back());
        assert!(h.can_go_forward());

        let fwd = h.go_forward().unwrap();
        assert_eq!(fwd, mrs);
        assert!(h.can_go_back());
        assert!(!h.can_go_forward());
    }

    #[test]
    fn new_push_clears_forward_and_caps_back_stack() {
        let mut h = NavHistory::new(NavEntry::screen(None, Screen::Projects));
        h.push(NavEntry::screen(Some(1), Screen::Issues));
        h.go_back();
        assert!(h.can_go_forward());

        h.push(NavEntry::screen(Some(1), Screen::Pipelines));
        assert!(!h.can_go_forward());

        for i in 0..(MAX_HISTORY + 10) {
            h.push(NavEntry {
                project_id: Some(1),
                screen: Screen::Pipelines,
                target: NavTarget::Pipeline { pipeline_id: i as u64 },
            });
        }
        assert_eq!(h.back.len(), MAX_HISTORY);

        h.reset(NavEntry::screen(None, Screen::Projects));
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());
    }

    #[test]
    fn last_overview_path_tracks_current_project_dir() {
        let mut h = NavHistory::new(NavEntry::screen(None, Screen::Projects));
        h.push(NavEntry {
            project_id: Some(7),
            screen: Screen::ProjectOverview,
            target: NavTarget::OverviewDir { git_ref: "main".into(), path: "src/app".into() },
        });
        h.push(NavEntry::screen(Some(7), Screen::MergeRequests));
        assert_eq!(h.last_overview_path(Some(7)), "src/app");
        assert_eq!(h.last_overview_path(Some(8)), "");
    }
}
