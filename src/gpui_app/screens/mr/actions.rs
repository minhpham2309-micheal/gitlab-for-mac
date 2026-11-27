use gpui_kit::{Context, Window};

use crate::gitlab::{mr_can_review, MrFilter, MrListCounts, MrListQuery, MrSortField};
use crate::runtime::spawn_tokio;

use super::MergeRequestsView;

impl MergeRequestsView {
    pub(crate) fn clear_detail(&mut self, cx: &mut Context<Self>) {
        (self.detail_task, self.mutation_task, self.selected) = (None, None, None);
        self.active_tab = 0;
        self.approvals = None;
        self.approvals_expanded = false;
        self.merge_details_expanded = true;
        self.commits.clear();
        self.mr_pipelines.clear();
        self.head_pipeline_jobs.clear();
        self.head_pipeline_bridges.clear();
        self.cached_stages.clear();
        self.cached_activity.clear();
        self.overview_list.reset(4);
        self.participants.clear();
        self.award_emojis.clear();
        self.todo_added = false;
        self.raw_diffs.clear();
        self.fallback_notices.clear();
        self.has_truncated = false;
        self.discussions.clear();
        self.discussions_error = None;
        self.inline_target = None;
        self.replying_to = None;
        self.diff_state.update(cx, |ds, cx| ds.set_files(Vec::new(), cx));
    }

    pub(crate) fn reset_state(&mut self, cx: &mut Context<Self>) {
        (self.list_task, self.counts_task) = (None, None);
        self.mrs.clear();
        self.counts = MrListCounts::default();
        self.page = 1;
        self.next_page = None;
        self.list_loading = false;
        self.clear_detail(cx);
        self.sync_summary(cx);
        cx.notify();
    }

    pub(crate) fn sync_summary(&self, cx: &mut Context<Self>) {
        let mr = self.selected.clone();
        self.state.update(cx, |s, _| {
            s.summary.can_review_mr = mr.as_ref().is_some_and(|m| mr_can_review(&m.state));
            s.summary.active_mr_iid = mr.as_ref().map(|m| m.iid);
            s.summary.active_mr_label = mr.as_ref().map(|m| format!("!{} {}", m.iid, m.title));
            if let Some(m) = &mr {
                s.summary.active_web_url = Some(m.web_url.clone());
            } else if let Some(p) = &s.project {
                s.summary.active_web_url = Some(format!("{}/-/merge_requests", p.web_url.trim_end_matches('/')));
            }
        });
    }

    pub fn load_list(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { cx.notify(); return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        let search = self.search_input.read(cx).value().trim().to_string();
        let query = MrListQuery {
            state: self.filter.api_state().to_string(),
            search: search.clone(),
            order_by: self.sort_field,
            sort: self.sort_dir,
            page: self.page,
            per_page: self.per_page,
        };
        self.list_loading = true;
        self.state.update(cx, |s, cx| { s.busy = true; s.set_status("Loading merge requests…", cx); });

        let c_counts = client.clone();
        let search_counts = search;
        self.counts_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { c_counts.merge_request_counts(project.id, &search_counts).await }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch == h_ep && cur.project_epoch == p_ep && let Ok(c) = res {
                    v.counts = c;
                    cx.notify();
                }
            });
        }));

        self.list_task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                client.merge_requests_page(project.id, &query).await.map_err(|e| e.to_string())
            }).await;
            let _ = this.update(cx, |view, cx| {
                let cur = view.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                view.list_loading = false;
                view.state.update(cx, |s, _| s.busy = false);
                match res {
                    Ok(Ok(pg)) => {
                        let n = pg.items.len();
                        view.mrs = pg.items;
                        view.page = pg.page;
                        view.next_page = pg.next_page;
                        view.state.update(cx, |s, cx| s.set_status(format!("{n} merge requests"), cx));
                        if let Some(pending) = view.pending_iid.take()
                            && let Some(found) = view.mrs.iter().find(|m| m.iid == pending).cloned()
                        {
                            view.select_mr(found, false, cx);
                            return;
                        }
                    }
                    Ok(Err(err)) | Err(err) => view.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }

    pub fn back_to_list(&mut self, cx: &mut Context<Self>) {
        self.clear_detail(cx);
        self.state.update(cx, |s, cx| s.record_mr_list(cx));
        self.sync_summary(cx);
        cx.notify();
    }

    pub(crate) fn show_list_without_history(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_some() {
            self.clear_detail(cx);
            self.sync_summary(cx);
            cx.notify();
        }
    }

    pub fn set_filter(&mut self, f: MrFilter, cx: &mut Context<Self>) {
        self.filter = f;
        self.page = 1;
        self.load_list(cx);
    }

    pub fn set_sort_field(&mut self, field: MrSortField, cx: &mut Context<Self>) {
        self.sort_field = field;
        self.page = 1;
        self.load_list(cx);
    }

    pub fn toggle_sort_dir(&mut self, cx: &mut Context<Self>) {
        self.sort_dir = self.sort_dir.toggle();
        self.page = 1;
        self.load_list(cx);
    }

    pub fn set_per_page(&mut self, n: usize, cx: &mut Context<Self>) {
        self.per_page = n;
        self.page = 1;
        self.load_list(cx);
    }

    pub fn go_page(&mut self, p: usize, cx: &mut Context<Self>) {
        self.page = p.max(1);
        self.load_list(cx);
    }

    pub fn submit_search(&mut self, cx: &mut Context<Self>) {
        let q = self.search_input.read(cx).value().trim().to_string();
        if !q.is_empty() && !self.search_history.iter().any(|h| h == &q) {
            self.search_history.insert(0, q);
            self.search_history.truncate(8);
        }
        self.page = 1;
        self.load_list(cx);
    }

    pub fn apply_history_search(&mut self, q: String, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |i, cx| i.set_value(q, window, cx));
        self.submit_search(cx);
    }
}
