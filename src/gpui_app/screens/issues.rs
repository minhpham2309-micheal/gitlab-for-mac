use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, AppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{Issue, IssueFilter, Note};
use crate::nav::Screen;

use super::super::avatar::render_avatar;
use super::super::markdown::MarkdownProjectContext;
use super::super::pajamas_theme::status_badge_palette;
use super::super::safe_url::safe_open_url;
use super::super::state::{AppEvent, AppState, ScreenCommand};
use super::issues_detail::render_issue_detail;

pub struct IssuesView {
    pub(crate) state: Entity<AppState>,
    pub(crate) filter: IssueFilter,
    pub(crate) filter_input: Entity<InputState>,
    pub(crate) issues: Vec<Issue>,
    pub(crate) selected: Option<Issue>,
    pub(crate) pending_iid: Option<u64>,
    pub(crate) notes: Vec<Note>,
    pub(crate) comment_input: Entity<InputState>,
    pub(crate) list_task: Option<Task<()>>,
    pub(crate) notes_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl IssuesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search or filter work items…"));
        let comment_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add a comment…"));
        let sub_f = cx.subscribe_in(&filter_input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                (this.list_task, this.notes_task, this.selected) = (None, None, None);
                this.issues.clear();
                this.notes.clear();
                if this.state.read(cx).screen == Screen::Issues { this.load_list(cx); }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Issues && this.issues.is_empty() => this.load_list(cx),
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Issues => this.load_list(cx),
            AppEvent::Command(ScreenCommand::OpenIssue { iid, .. }) => { this.pending_iid = Some(*iid); this.load_list(cx); }
            AppEvent::PermissionsChanged => cx.notify(),
            _ => {}
        });
        Self {
            state, filter: IssueFilter::Open, filter_input, issues: Vec::new(), selected: None,
            pending_iid: None, notes: Vec::new(), comment_input, list_task: None, notes_task: None,
            _subs: vec![sub_f, sub],
        }
    }

    pub fn set_fixture_data(&mut self, issues: Vec<Issue>, notes: Vec<Note>, selected: Option<Issue>, cx: &mut Context<Self>) {
        (self.issues, self.notes, self.selected) = (issues, notes, selected);
        cx.notify();
    }

    pub(crate) fn back_to_list(&mut self, cx: &mut Context<Self>) {
        (self.selected, self.notes_task) = (None, None);
        self.notes.clear();
        cx.notify();
    }
}

impl Render for IssuesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (md_ctx, client, perms, me_id, project_web) = {
            let st = self.state.read(cx);
            (
                MarkdownProjectContext::from_state(st, cx),
                st.gitlab.clone(),
                st.project_permissions,
                st.current_user.as_ref().map(|u| u.id),
                st.project.as_ref().map(|p| p.web_url.clone()).filter(|u| !u.is_empty()),
            )
        };

        if let Some(selected) = self.selected.clone() {
            let is_author = me_id.is_some() && selected.author.as_ref().map(|a| a.id) == me_id;
            return render_issue_detail(
                Some(selected), &self.notes, &self.comment_input, md_ctx, client.as_ref(),
                perms.can_manage_issue(is_author), perms.can_comment(), cx,
            );
        }

        let q = self.filter_input.read(cx).value().trim().to_ascii_lowercase();
        let filtered: Vec<Issue> = self.issues.iter().filter(|i| {
            q.is_empty() || i.title.to_ascii_lowercase().contains(&q)
                || format!("#{}", i.iid).contains(&q)
                || i.labels.iter().any(|l| l.to_ascii_lowercase().contains(&q))
                || i.author.as_ref().is_some_and(|a| a.name.to_ascii_lowercase().contains(&q))
        }).cloned().collect();
        let count = filtered.len();
        let new_issue_url = project_web.as_ref().map(|u| format!("{}/-/issues/new", u.trim_end_matches('/')));
        let can_create = perms.can_comment();

        let (dark, border, head_bg, bg, hover_bg, fg, muted, input_b, primary, sec_bg) = {
            let t = cx.theme();
            (t.is_dark(), t.border, t.list_head, t.background, t.list_hover, t.foreground, t.muted_foreground, t.input, t.primary, t.secondary)
        };

        let tabs = div().flex().items_center().gap_1().children(IssueFilter::ALL.into_iter().map(|f| {
            let active = self.filter == f;
            div().id(SharedString::from(format!("iss-tab-{}", f.api_state())))
                .px_3().py_2p5().cursor_pointer()
                .border_b_2().border_color(if active { primary } else { gpui_kit::transparent_black() })
                .text_sm().text_color(if active { fg } else { muted }).hover(move |s| s.text_color(fg))
                .on_click(cx.listener(move |v, _, _, cx| { v.filter = f; v.load_list(cx); }))
                .child(div().when(active, |d| d.font_semibold()).child(f.label()))
                .test_support()
        }));

        let actions = div().flex().items_center().gap_1p5().pb_1p5()
            .child(Button::new("iss-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                .on_click(cx.listener(|v, _, _, cx| v.load_list(cx))))
            .children(new_issue_url.clone().filter(|_| can_create).map(|u| {
                Button::new("iss-new-item").primary().small().label("New item")
                    .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
            }));

        let body = if filtered.is_empty() {
            div().id("issues-empty-state").flex_1().py_12().flex().flex_col().items_center().justify_center().gap_3()
                .child(div().size(px(64.)).rounded_full().bg(head_bg).border_1().border_color(border).flex().items_center().justify_center()
                    .child(Icon::empty().path("icons/glab-issue.svg").size_6().text_color(muted)))
                .child(div().text_lg().font_bold().text_color(fg).child("Track bugs, plan features, and organize your efforts with work items"))
                .child(div().text_xs().text_color(muted).child("Use work items to collaborate on ideas, solve problems, and plan your project."))
                .children(new_issue_url.filter(|_| can_create).map(|u| {
                    Button::new("iss-empty-new").primary().small().label("New item")
                        .on_click(move |_, _, _| { let _ = safe_open_url(&u); })
                }))
                .test_support().into_any_element()
        } else {
            let mut row_els = Vec::with_capacity(filtered.len());
            for iss in filtered {
                let iid = iss.iid;
                let clicked = iss.clone();
                let pal = status_badge_palette(&iss.state, dark);
                let rel = iss.updated_at.as_deref().or(iss.created_at.as_deref()).map(format_relative).unwrap_or_default();
                let author = iss.author.as_ref().map(|a| a.name.clone()).unwrap_or_default();
                let meta = match (author.is_empty(), rel.is_empty()) {
                    (false, false) => format!("#{iid} · updated {rel} by {author}"),
                    (false, true) => format!("#{iid} · by {author}"),
                    (true, false) => format!("#{iid} · updated {rel}"),
                    (true, true) => format!("#{iid}"),
                };
                let av = iss.assignees.first().map(|a| render_avatar(a.avatar_url.as_deref(), &a.name, px(22.), client.as_ref(), cx));
                row_els.push(
                    div().id(SharedString::from(format!("iss-row-{iid}"))).w_full().px_3().py_3()
                        .border_b_1().border_color(border.opacity(0.6)).cursor_pointer().hover(move |s| s.bg(hover_bg.opacity(0.45)))
                        .flex().items_center().justify_between().gap_3()
                        .on_click(cx.listener(move |v, _, _, cx| v.select_issue(clicked.clone(), cx)))
                        .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                            .child(div().flex().items_center().gap_2().min_w_0()
                                .child(Icon::empty().path("icons/glab-issue.svg").size_3p5().text_color(pal.icon))
                                .child(div().text_sm().font_semibold().text_color(fg).truncate().child(iss.title.clone()))
                                .children(iss.labels.iter().take(3).map(|l| {
                                    div().px_1p5().py(px(1.)).rounded_full().bg(sec_bg).border_1().border_color(border).text_xs().text_color(fg).child(l.clone())
                                })))
                            .child(div().text_xs().text_color(muted).child(meta)))
                        .child(div().flex().items_center().gap_2().flex_shrink_0()
                            .children(av)
                            .child(div().text_xs().text_color(muted).child(format!("💬 {}", iss.user_notes_count)))
                            .child(div().px_2().py(px(1.)).rounded_full().bg(pal.bg).text_color(pal.text).text_xs().font_semibold().child(if iss.state == "opened" { "Open" } else { "Closed" })))
                        .test_support(),
                );
            }
            div().id("issues-list-scroll").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().pb_6().children(row_els).test_support().into_any_element()
        };

        div().id("issues-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                .child(div().w_full().border_b_1().border_color(border).flex().items_end().justify_between().gap_4().child(tabs).child(actions))
                .child(div().mt_3().mb_2().p_2().rounded_md().bg(head_bg).border_1().border_color(border)
                    .child(div().h(px(32.)).rounded_md().bg(bg).border_1().border_color(input_b).px_2().flex().items_center().gap_1p5()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.filter_input).appearance(false).cleanable(true).small()))
                        .child(Icon::new(IconName::Search).size_3p5().text_color(muted))))
                .child(div().pb_2().text_xs().text_color(muted).child(format!("{count} items")))
                .child(body))
            .test_support().into_any_element()
    }
}
