use gpui_kit::{Context, Window};

use crate::gitlab::{Group, GroupChildren, GroupTab};
use crate::runtime::spawn_tokio;

use super::GroupsView;

impl GroupsView {
    pub(super) fn reset_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.list_task = None;
        self.child_tasks.clear();
        self.active_group = None;
        self.pending_path = None;
        self.tab = GroupTab::SubgroupsAndProjects;
        self.subgroups.clear();
        self.projects.clear();
        self.children_cache.clear();
        self.expanded.clear();
        self.loading_children.clear();
        (self.loading, self.error) = (false, None);
        self.search_input.update(cx, |i, cx| i.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn open_group_path(&mut self, full_path: Option<String>, cx: &mut Context<Self>) {
        self.list_task = None;
        self.child_tasks.clear();
        self.expanded.clear();
        self.loading_children.clear();
        self.error = None;
        self.tab = GroupTab::SubgroupsAndProjects;

        let Some(path) = full_path.filter(|p| !p.trim_matches('/').is_empty()) else {
            self.active_group = None;
            self.projects.clear();
            self.state.update(cx, |s, _| s.active_group_path = None);
            self.load_root_groups(None, cx);
            return;
        };
        if let Some(known) = self.find_known_group(&path) {
            self.enter_group(known, false, cx);
            return;
        }
        let st = self.state.read(cx);
        if st.offline_mode {
            let slug = path.rsplit('/').next().unwrap_or(&path).to_string();
            self.active_group = Some(Group {
                id: 0, name: slug.clone(), path: slug, full_name: path.clone(), full_path: path, ..Group::default()
            });
            self.subgroups.clear();
            self.projects.clear();
            cx.notify();
            return;
        }
        let Some(client) = st.gitlab.clone() else { return; };
        let h_ep = st.host_epoch;
        self.loading = true;
        cx.notify();
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.group_by_path(&path).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                if v.state.read(cx).host_epoch != h_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(group)) => v.enter_group(group, false, cx),
                    Ok(Err(err)) | Err(err) => {
                        v.error = Some(err.clone());
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                        cx.notify();
                    }
                }
            });
        }));
    }

    fn find_known_group(&self, full_path: &str) -> Option<Group> {
        let needle = full_path.trim_matches('/').to_ascii_lowercase();
        let eq = |g: &&Group| g.full_path.trim_matches('/').to_ascii_lowercase() == needle;
        self.active_group.as_ref().filter(eq).cloned().or_else(|| {
            self.subgroups
                .iter()
                .chain(self.children_cache.values().flat_map(|c| c.subgroups.iter()))
                .find(eq)
                .cloned()
        })
    }

    pub(super) fn enter_group(&mut self, group: Group, push_nav: bool, cx: &mut Context<Self>) {
        if let Some(prev) = self.active_group.as_ref()
            && self.tab == GroupTab::SubgroupsAndProjects
            && (!self.subgroups.is_empty() || !self.projects.is_empty())
        {
            self.children_cache.insert(
                prev.id,
                GroupChildren { subgroups: self.subgroups.clone(), projects: self.projects.clone() },
            );
        }
        if push_nav {
            let full_path = group.full_path.clone();
            self.state.update(cx, |s, cx| s.record_group_nav(Some(full_path), cx));
        }
        self.state.update(cx, |s, cx| {
            s.active_group_path = Some(group.full_path.clone());
            if !group.web_url.is_empty() { s.summary.active_web_url = Some(group.web_url.clone()); }
            cx.notify();
        });
        let gid = group.id;
        self.active_group = Some(group);
        self.tab = GroupTab::SubgroupsAndProjects;
        self.expanded.clear();
        if let Some(cached) = self.children_cache.get(&gid).cloned() {
            (self.subgroups, self.projects, self.loading) = (cached.subgroups, cached.projects, false);
            cx.notify();
            return;
        }
        self.subgroups.clear();
        self.projects.clear();
        self.load_active_group_contents(None, cx);
    }

    pub(super) fn load_root_groups(&mut self, search: Option<String>, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { cx.notify(); return; }
        let Some(client) = st.gitlab.clone() else { return; };
        let h_ep = st.host_epoch;
        (self.loading, self.error) = (true, None);
        cx.notify();
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.top_level_groups(search.as_deref()).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                if v.state.read(cx).host_epoch != h_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(groups)) => { v.subgroups = groups; v.projects.clear(); }
                    Ok(Err(err)) | Err(err) => {
                        v.error = Some(err.clone());
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn load_active_group_contents(&mut self, search: Option<String>, cx: &mut Context<Self>) {
        let Some(group) = self.active_group.clone() else { self.load_root_groups(search, cx); return; };
        let st = self.state.read(cx);
        if st.offline_mode { cx.notify(); return; }
        let Some(client) = st.gitlab.clone() else { return; };
        let (h_ep, tab, gid, full_path) = (st.host_epoch, self.tab, group.id, group.full_path);
        (self.loading, self.error) = (true, None);
        cx.notify();
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                match tab {
                    GroupTab::SubgroupsAndProjects => {
                        let (subs, projs) = tokio::join!(
                            client.group_subgroups(gid, search.as_deref()),
                            client.group_projects(gid, &full_path, false, false, search.as_deref()),
                        );
                        Ok::<_, String>((subs.unwrap_or_default(), projs.map_err(|e| e.to_string())?))
                    }
                    GroupTab::SharedProjects => Ok((
                        Vec::new(),
                        client.group_projects(gid, &full_path, true, false, search.as_deref()).await.map_err(|e| e.to_string())?,
                    )),
                    GroupTab::Inactive => {
                        let (subs, projs) = tokio::join!(
                            client.group_subgroups(gid, search.as_deref()),
                            client.group_projects(gid, &full_path, false, true, search.as_deref()),
                        );
                        Ok((subs.unwrap_or_default().into_iter().filter(Group::is_inactive).collect(), projs.unwrap_or_default()))
                    }
                }
            }).await;
            let _ = this.update(cx, |v, cx| {
                if v.state.read(cx).host_epoch != h_ep || v.active_group.as_ref().map(|g| g.id) != Some(gid) { return; }
                v.loading = false;
                match res {
                    Ok(Ok((subs, projs))) => {
                        if tab == GroupTab::SubgroupsAndProjects {
                            v.children_cache.insert(gid, GroupChildren { subgroups: subs.clone(), projects: projs.clone() });
                        }
                        (v.subgroups, v.projects) = (subs, projs);
                    }
                    Ok(Err(err)) | Err(err) => {
                        v.error = Some(err.clone());
                        v.state.update(cx, |s, cx| s.set_status(err, cx));
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn toggle_expand(&mut self, group: Group, cx: &mut Context<Self>) {
        let gid = group.id;
        if self.expanded.remove(&gid) { cx.notify(); return; }
        self.expanded.insert(gid);
        if self.children_cache.contains_key(&gid) || self.loading_children.contains(&gid) { cx.notify(); return; }
        let st = self.state.read(cx);
        if st.offline_mode { cx.notify(); return; }
        let Some(client) = st.gitlab.clone() else { cx.notify(); return; };
        let (h_ep, full_path) = (st.host_epoch, group.full_path);
        self.loading_children.insert(gid);
        cx.notify();
        let task = cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (subs, projs) = tokio::join!(
                    client.group_subgroups(gid, None),
                    client.group_projects(gid, &full_path, false, false, None),
                );
                (subs.unwrap_or_default(), projs.unwrap_or_default())
            }).await;
            let _ = this.update(cx, |v, cx| {
                if v.state.read(cx).host_epoch != h_ep { return; }
                v.loading_children.remove(&gid);
                v.child_tasks.remove(&gid);
                if let Ok((subgroups, projects)) = res {
                    v.children_cache.insert(gid, GroupChildren { subgroups, projects });
                }
                cx.notify();
            });
        });
        self.child_tasks.insert(gid, task);
    }
}
