use gpui_kit::{
    base::StyledExt as _,
    component::{
        input::Input,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window,
};
use serde_json::json;

use crate::gitlab::{GitLab, MergeRequest, User};
use crate::runtime::spawn_tokio;

use super::super::super::avatar::render_avatar;
use super::MergeRequestsView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberPickerKind {
    Assignees,
    Reviewers,
}

impl MergeRequestsView {
    pub fn toggle_member_picker(
        &mut self,
        kind: MemberPickerKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selected.as_ref().is_some_and(|m| self.mr_permissions(m, cx).can_edit_metadata) {
            self.member_picker = None;
            return;
        }
        if self.member_picker == Some(kind) {
            self.member_picker = None;
            cx.notify();
            return;
        }
        self.member_picker = Some(kind);
        self.member_filter_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        if !self.members_loaded && !self.members_loading {
            self.load_project_members(cx);
        }
        cx.notify();
    }

    pub fn load_project_members(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.members_loading = true;
        self.members_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.project_members(project.id).await.map_err(|e| e.to_string())
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.members_loading = false;
                match res {
                    Ok(Ok(users)) => {
                        v.project_members = users;
                        v.members_loaded = true;
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn toggle_mr_member(&mut self, kind: MemberPickerKind, user: User, cx: &mut Context<Self>) {
        if !self.selected.as_ref().is_some_and(|m| self.mr_permissions(m, cx).can_edit_metadata) { return; }
        let Some(mr) = self.selected.as_mut() else { return; };
        let list = match kind {
            MemberPickerKind::Assignees => &mut mr.assignees,
            MemberPickerKind::Reviewers => &mut mr.reviewers,
        };
        if let Some(pos) = list.iter().position(|u| u.id == user.id) {
            list.remove(pos);
        } else {
            list.push(user);
        }
        let ids: Vec<u64> = list.iter().map(|u| u.id).collect();
        self.commit_mr_member_ids(kind, ids, cx);
    }

    pub fn clear_mr_members(&mut self, kind: MemberPickerKind, cx: &mut Context<Self>) {
        if !self.selected.as_ref().is_some_and(|m| self.mr_permissions(m, cx).can_edit_metadata) { return; }
        let Some(mr) = self.selected.as_mut() else { return; };
        match kind {
            MemberPickerKind::Assignees => mr.assignees.clear(),
            MemberPickerKind::Reviewers => mr.reviewers.clear(),
        }
        self.commit_mr_member_ids(kind, Vec::new(), cx);
    }

    fn commit_mr_member_ids(&mut self, kind: MemberPickerKind, ids: Vec<u64>, cx: &mut Context<Self>) {
        cx.notify();
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(mr), Some(client), Some(project)) = (self.selected.clone(), st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        let body = match kind {
            MemberPickerKind::Assignees => json!({ "assignee_ids": ids }),
            MemberPickerKind::Reviewers => json!({ "reviewer_ids": ids }),
        };
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.update_mr_fields(project.id, iid, body).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || v.selected.as_ref().map(|m| m.iid) != Some(iid) { return; }
                match res {
                    Ok(Ok(updated)) => { v.selected = Some(updated); v.sync_summary(cx); cx.notify(); }
                    Ok(Err(e)) | Err(e) => v.state.update(cx, |s, cx| s.set_status(e, cx)),
                }
            });
        }));
    }

    pub fn set_fixture_members(&mut self, members: Vec<User>, cx: &mut Context<Self>) {
        self.project_members = members;
        self.members_loaded = true;
        self.members_loading = false;
        cx.notify();
    }
}

pub fn render_member_picker(
    view: &MergeRequestsView,
    mr: &MergeRequest,
    kind: MemberPickerKind,
    client: Option<&GitLab>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let selected_ids: Vec<u64> = match kind {
        MemberPickerKind::Assignees => mr.assignees.iter().map(|u| u.id).collect(),
        MemberPickerKind::Reviewers => mr.reviewers.iter().map(|u| u.id).collect(),
    };
    let q = view.member_filter_input.read(cx).value().trim().to_ascii_lowercase();
    let mut candidates: Vec<User> = view.project_members.clone();
    for u in mr.assignees.iter().chain(mr.reviewers.iter()).chain(view.participants.iter()) {
        if candidates.iter().all(|c| c.id != u.id) {
            candidates.push(u.clone());
        }
    }
    let filtered: Vec<User> = candidates
        .into_iter()
        .filter(|u| q.is_empty() || u.name.to_ascii_lowercase().contains(&q) || u.username.to_ascii_lowercase().contains(&q))
        .collect();

    let rows: Vec<_> = filtered
        .into_iter()
        .map(|u| {
            let checked = selected_ids.contains(&u.id);
            let uid = u.id;
            let click_user = u.clone();
            let av = render_avatar(u.avatar_url.as_deref(), &u.username, px(20.), client, cx);
            let theme = cx.theme();
            div()
                .id(SharedString::from(format!("mr-member-opt-{uid}")))
                .w_full().px_2().py_1p5().rounded_sm().cursor_pointer()
                .bg(if checked { theme.list_active.opacity(0.6) } else { gpui_kit::transparent_black() })
                .hover(|s| s.bg(theme.list_hover))
                .flex().items_center().justify_between().gap_2()
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_mr_member(kind, click_user.clone(), cx)))
                .child(
                    div().flex().items_center().gap_2().min_w_0()
                        .child(av)
                        .child(
                            div().flex().flex_col().min_w_0()
                                .child(div().text_xs().font_semibold().text_color(theme.foreground).truncate().child(u.name))
                                .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(format!("@{}", u.username))),
                        ),
                )
                .children(checked.then(|| Icon::new(IconName::Check).size_3p5().text_color(theme.primary)))
                .test_support()
        })
        .collect();

    let rows_empty = rows.is_empty();
    let theme = cx.theme();
    let empty_msg = if view.members_loading { "Loading project members…" } else { "No matching members" };

    div()
        .id("mr-member-picker")
        .mt_1().p_2().rounded_md().border_1().border_color(theme.border).bg(theme.popover)
        .flex().flex_col().gap_1p5()
        .child(Input::new(&view.member_filter_input).xsmall().cleanable(true))
        .children((!selected_ids.is_empty()).then(|| {
            div().id("mr-member-unassign").w_full().px_2().py_1().rounded_sm().cursor_pointer()
                .hover(|s| s.bg(theme.list_hover)).text_xs().text_color(theme.muted_foreground)
                .on_click(cx.listener(move |this, _, _, cx| this.clear_mr_members(kind, cx)))
                .child("Unassigned").test_support()
        }))
        .child(
            div().id("mr-member-picker-list").w_full().max_h(px(220.)).overflow_y_scroll().flex().flex_col().gap_0p5()
                .children(rows)
                .children(rows_empty.then(|| {
                    div().px_2().py_2().text_xs().text_color(theme.muted_foreground).child(empty_msg)
                })),
        )
        .test_support()
}
