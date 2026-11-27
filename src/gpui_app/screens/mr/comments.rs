use gpui_kit::{
    component::{diff::DiffLinePosition, WindowExt as _},
    Context, Window,
};

use crate::runtime::spawn_tokio;

use super::annotations::resolve_diff_position;
use super::MergeRequestsView;

impl MergeRequestsView {
    pub fn open_inline_composer(
        &mut self,
        pos: DiffLinePosition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selected.as_ref().is_some_and(|m| self.mr_permissions(m, cx).can_comment) { return; }
        self.inline_target = Some(pos);
        self.inline_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        cx.notify();
    }

    pub fn cancel_inline_composer(&mut self, cx: &mut Context<Self>) {
        self.inline_target = None;
        cx.notify();
    }

    pub fn start_reply(&mut self, disc_id: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.as_ref().is_some_and(|m| self.mr_permissions(m, cx).can_comment) { return; }
        self.replying_to = Some(disc_id);
        self.reply_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        self.overview_list.remeasure();
        cx.notify();
    }

    pub fn submit_mr_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.comment_input.read(cx).value().trim().to_string();
        if body.is_empty() { return; }
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) =
            (self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        if !self.mr_permissions(&mr, cx).can_comment { return; }
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        self.comment_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.create_mr_discussion(project.id, iid, &body, None).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || view.selected.as_ref().map(|m| m.iid) != Some(iid) {
                    return;
                }
                match res {
                    Ok(Ok(disc)) => { view.discussions.push(disc); view.rebuild_activity_cache(); }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn submit_inline_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.inline_input.read(cx).value().trim().to_string();
        let st = self.state.read(cx);
        let (Some(target), Some(mr), Some(client), Some(project)) =
            (self.inline_target.clone(), self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        if body.is_empty() || !self.mr_permissions(&mr, cx).can_comment { return; }
        let Some(refs) = mr.diff_refs.as_ref() else {
            window.push_notification("MR diff_refs not ready yet", cx);
            return;
        };
        let Some(position) = resolve_diff_position(&self.raw_diffs, &target, refs) else {
            window.push_notification("Could not map diff line to GitLab position", cx);
            return;
        };
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        self.inline_target = None;
        self.inline_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.create_mr_discussion(project.id, iid, &body, Some(position)).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || view.selected.as_ref().map(|m| m.iid) != Some(iid) {
                    return;
                }
                match res {
                    Ok(Ok(disc)) => { view.discussions.push(disc); view.rebuild_activity_cache(); }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn submit_reply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.reply_input.read(cx).value().trim().to_string();
        let st = self.state.read(cx);
        let (Some(disc_id), Some(mr), Some(client), Some(project)) =
            (self.replying_to.take(), self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        if body.is_empty() || !self.mr_permissions(&mr, cx).can_comment { return; }
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        self.reply_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let id_copy = disc_id.clone();
            let res = spawn_tokio(async move {
                client.reply_mr_discussion(project.id, iid, &disc_id, &body).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || view.selected.as_ref().map(|m| m.iid) != Some(iid) {
                    return;
                }
                match res {
                    Ok(Ok(note)) => {
                        if let Some(d) = view.discussions.iter_mut().find(|d| d.id == id_copy) {
                            d.notes.push(note);
                        }
                        view.overview_list.remeasure();
                    }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn resolve_discussion(&mut self, disc_id: String, resolved: bool, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(mr), Some(client), Some(project)) =
            (self.selected.clone(), st.gitlab.clone(), st.project.clone())
        else { return; };
        if !self.mr_permissions(&mr, cx).can_resolve_discussions { return; }
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.resolve_mr_discussion(project.id, iid, &disc_id, resolved).await.map_err(|e| e.to_string())
            })
            .await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep || view.selected.as_ref().map(|m| m.iid) != Some(iid) {
                    return;
                }
                match res {
                    Ok(Ok(updated)) => {
                        if let Some(d) = view.discussions.iter_mut().find(|d| d.id == updated.id) {
                            *d = updated;
                        }
                        view.overview_list.remeasure();
                    }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}
