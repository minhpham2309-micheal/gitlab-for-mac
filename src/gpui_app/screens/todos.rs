use std::collections::HashSet;

use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, prelude::FluentBuilder as _, px, rgb, AppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::time_util::format_relative;
use crate::gitlab::{filter_todos, todo_action_phrase, Todo, TodoTab};
use crate::nav::Screen;

use super::super::avatar::render_avatar;
use super::super::pajamas_theme::status_badge_palette;
use super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct TodosView {
    pub(crate) state: Entity<AppState>,
    pub(crate) filter_input: Entity<InputState>,
    pub(crate) tab: TodoTab,
    pub(crate) pending_todos: Vec<Todo>,
    pub(crate) done_todos: Vec<Todo>,
    pub(crate) snoozed_ids: HashSet<u64>,
    pub(crate) selected_ids: HashSet<u64>,
    pub(crate) loading: bool,
    pub(crate) load_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl TodosView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter to-do items"));
        let sub_in = cx.subscribe_in(&filter_input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } => {
                (this.load_task, this.loading, this.tab) = (None, false, TodoTab::Pending);
                this.pending_todos.clear();
                this.done_todos.clear();
                this.snoozed_ids.clear();
                this.selected_ids.clear();
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Todos => this.load(cx),
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Todos => { this.load(cx); }
            _ => {}
        });
        Self {
            state, filter_input, tab: TodoTab::Pending, pending_todos: Vec::new(), done_todos: Vec::new(),
            snoozed_ids: HashSet::new(), selected_ids: HashSet::new(), loading: false, load_task: None,
            _subs: vec![sub_in, sub_st],
        }
    }

    pub fn tab(&self) -> TodoTab { self.tab }
    pub fn selected_count(&self) -> usize { self.selected_ids.len() }
    pub fn pending_count(&self) -> usize { self.pending_todos.iter().filter(|t| !self.snoozed_ids.contains(&t.id)).count() }
    pub fn snoozed_count(&self) -> usize { self.pending_todos.iter().filter(|t| self.snoozed_ids.contains(&t.id)).count() }

    pub fn set_fixture_data(&mut self, pending: Vec<Todo>, done: Vec<Todo>, cx: &mut Context<Self>) {
        (self.pending_todos, self.done_todos, self.loading) = (pending, done, false);
        self.snoozed_ids.clear();
        self.selected_ids.clear();
        cx.notify();
    }

    pub(crate) fn set_tab(&mut self, tab: TodoTab, cx: &mut Context<Self>) {
        if self.tab == tab { return; }
        self.tab = tab;
        self.selected_ids.clear();
        if tab == TodoTab::Done && self.done_todos.is_empty() { self.load_done(cx); }
        cx.notify();
    }
}

impl Render for TodosView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.filter_input.read(cx).value().to_string();
        let client = self.state.read(cx).gitlab.clone();
        let raw_items: Vec<Todo> = match self.tab {
            TodoTab::Pending => self.pending_todos.iter().filter(|t| !self.snoozed_ids.contains(&t.id)).cloned().collect(),
            TodoTab::Snoozed => self.pending_todos.iter().filter(|t| self.snoozed_ids.contains(&t.id)).cloned().collect(),
            TodoTab::Done => self.done_todos.clone(),
        };
        let rows = filter_todos(&raw_items, &query);
        let visible_ids: Vec<u64> = rows.iter().map(|t| t.id).collect();
        let all_selected = !visible_ids.is_empty() && visible_ids.iter().all(|id| self.selected_ids.contains(id));
        let (pending_n, snoozed_n, sel_n, cur_tab) = (self.pending_count(), self.snoozed_count(), self.selected_ids.len(), self.tab);

        let (dark, border, head_bg, bg, hover_bg, fg, muted, input_b, primary, primary_fg, sec_bg) = {
            let t = cx.theme();
            (t.is_dark(), t.border, t.list_head, t.background, t.list_hover, t.foreground, t.muted_foreground, t.input, t.primary, t.primary_foreground, t.secondary)
        };
        let (inact_bg, inact_fg): (gpui_kit::Hsla, gpui_kit::Hsla) = if dark { (rgb(0x383839).into(), rgb(0xBFBFBF).into()) } else { (sec_bg, muted) };

        let tabs = div().flex().items_center().gap_1().children(TodoTab::ALL.into_iter().map(|t| {
            let active = cur_tab == t;
            let cnt = match t { TodoTab::Pending => Some(pending_n), TodoTab::Snoozed if snoozed_n > 0 => Some(snoozed_n), _ => None };
            let (bbg, bfg) = if active { (primary, primary_fg) } else { (inact_bg, inact_fg) };
            let key = match t { TodoTab::Pending => "pending", TodoTab::Snoozed => "snoozed", TodoTab::Done => "done" };
            div().id(SharedString::from(format!("todos-tab-{key}"))).px_3().py_2p5().cursor_pointer()
                .border_b_2().border_color(if active { primary } else { gpui_kit::transparent_black() })
                .flex().items_center().gap_1p5().text_sm().text_color(if active { fg } else { muted }).hover(move |s| s.text_color(fg))
                .on_click(cx.listener(move |v, _, _, cx| v.set_tab(t, cx)))
                .child(div().when(active, |d| d.font_semibold()).child(t.label()))
                .children(cnt.map(|n| div().px_1p5().py(px(1.)).rounded_full().bg(bbg).text_color(bfg).text_xs().font_semibold().child(n.to_string())))
                .test_support()
        }));

        let mut row_els = Vec::with_capacity(rows.len());
        for todo in rows {
            let tid = todo.id;
            let checked = self.selected_ids.contains(&tid);
            let author_name = todo.author.as_ref().map(|u| if u.name.is_empty() { u.username.clone() } else { u.name.clone() }).unwrap_or_else(|| "GitLab".into());
            let av = render_avatar(todo.author.as_ref().and_then(|u| u.avatar_url.as_deref()), &author_name, px(30.), client.as_ref(), cx);
            let target_state = todo.target.as_ref().and_then(|t| t.state.as_deref()).filter(|s| matches!(*s, "closed" | "merged")).map(|s| {
                let pal = status_badge_palette(s, dark);
                let lbl = if s == "merged" { "Merged" } else { "Closed" };
                div().px_1p5().py(px(1.)).rounded_full().bg(pal.bg).text_color(pal.text).text_xs().font_semibold().child(lbl)
            });
            let target_icon = match todo.target_type.as_deref() {
                Some("MergeRequest") => "icons/glab-mr.svg",
                _ => "icons/glab-issue.svg",
            };
            let title_line = {
                let t_title = todo.target.as_ref().and_then(|t| t.title.as_deref()).unwrap_or(&todo.body);
                let proj = todo.project.as_ref().map(|p| p.path_with_namespace.as_str()).unwrap_or("");
                let ref_num = todo.target.as_ref().and_then(|t| t.iid).map(|n| {
                    if todo.target_type.as_deref() == Some("MergeRequest") { format!(" !{n}") } else { format!(" #{n}") }
                }).unwrap_or_default();
                if proj.is_empty() { format!("{t_title}{ref_num}") } else { format!("{t_title} · {proj}{ref_num}") }
            };
            let phrase = todo_action_phrase(&todo);
            let rel = todo.created_at.as_deref().map(format_relative).unwrap_or_default();
            let todo_open = todo.clone();
            let check_pal = status_badge_palette("success", dark);

            row_els.push(
                div().id(SharedString::from(format!("todo-row-{tid}"))).w_full().px_3().py_3()
                    .border_b_1().border_color(border.opacity(0.6)).cursor_pointer().hover(move |s| s.bg(hover_bg.opacity(0.45)))
                    .flex().items_center().justify_between().gap_3()
                    .on_click(cx.listener(move |v, _, _, cx| v.open_todo(todo_open.clone(), cx)))
                    .child(
                        div().flex_1().min_w_0().flex().items_center().gap_3()
                            .children((cur_tab != TodoTab::Done).then(|| {
                                div().id(SharedString::from(format!("todo-check-{tid}")))
                                    .size(px(16.)).rounded(px(3.)).flex_shrink_0().cursor_pointer()
                                    .border_1().border_color(if checked { check_pal.icon } else { input_b })
                                    .bg(if checked { check_pal.bg } else { gpui_kit::transparent_black() })
                                    .flex().items_center().justify_center()
                                    .on_click(cx.listener(move |v, _, _, cx| { cx.stop_propagation(); v.toggle_select_id(tid, cx); }))
                                    .children(checked.then(|| Icon::new(IconName::Check).size(px(11.)).text_color(check_pal.icon)))
                                    .test_support()
                            }))
                            .child(av)
                            .child(
                                div().flex_1().min_w_0().flex().flex_col().gap_1()
                                    .child(div().flex().items_center().gap_1p5().min_w_0()
                                        .children(target_state)
                                        .child(Icon::empty().path(target_icon).size_3p5().text_color(muted))
                                        .child(div().text_xs().text_color(muted).truncate().child(title_line)))
                                    .child(div().text_xs().font_semibold().text_color(fg).truncate().child(phrase)),
                            ),
                    )
                    .child(
                        div().flex().items_center().gap_2().flex_shrink_0()
                            .children((!rel.is_empty()).then(|| div().text_xs().text_color(muted).child(rel)))
                            .children((cur_tab == TodoTab::Pending).then(|| {
                                Button::new(SharedString::from(format!("todo-snooze-{tid}"))).outline().xsmall().label("Snooze")
                                    .on_click(cx.listener(move |v, _, _, cx| { cx.stop_propagation(); v.snooze_todo(tid, cx); }))
                            }))
                            .children((cur_tab == TodoTab::Snoozed).then(|| {
                                Button::new(SharedString::from(format!("todo-unsnooze-{tid}"))).outline().xsmall().label("Unsnooze")
                                    .on_click(cx.listener(move |v, _, _, cx| { cx.stop_propagation(); v.unsnooze_todo(tid, cx); }))
                            }))
                            .children((cur_tab != TodoTab::Done).then(|| {
                                Button::new(SharedString::from(format!("todo-done-{tid}"))).outline().xsmall().icon(Icon::new(IconName::Check).size_3())
                                    .on_click(cx.listener(move |v, _, _, cx| { cx.stop_propagation(); v.mark_done(tid, cx); }))
                            })),
                    )
                    .test_support(),
            );
        }

        let bulk_bar = (cur_tab != TodoTab::Done && !visible_ids.is_empty()).then(|| {
            let ids_toggle = visible_ids.clone();
            let check_pal = status_badge_palette("success", dark);
            div().id("todos-bulk-bar").w_full().px_3().py_2().border_b_1().border_color(border)
                .flex().items_center().justify_between().gap_3()
                .child(
                    div().id("todos-select-all").cursor_pointer().flex().items_center().gap_2()
                        .on_click(cx.listener(move |v, _, _, cx| v.toggle_select_all(&ids_toggle, cx)))
                        .child(div().size(px(16.)).rounded(px(3.)).border_1().border_color(if all_selected { check_pal.icon } else { input_b })
                            .bg(if all_selected { check_pal.bg } else { gpui_kit::transparent_black() }).flex().items_center().justify_center()
                            .children(all_selected.then(|| Icon::new(IconName::Check).size(px(11.)).text_color(check_pal.icon))))
                        .child(div().text_xs().text_color(fg).child("Select all")).test_support(),
                )
                .child(div().flex().items_center().gap_1p5()
                    .children((sel_n > 0).then(|| {
                        Button::new("todos-mark-selected").primary().xsmall().label(format!("Mark done ({sel_n})"))
                            .on_click(cx.listener(|v, _, _, cx| v.mark_selected_done(cx)))
                    }))
                    .children((cur_tab == TodoTab::Pending && pending_n > 0).then(|| {
                        Button::new("todos-mark-all").outline().xsmall().label("Mark all done")
                            .on_click(cx.listener(|v, _, w, cx| v.mark_all_confirmed(w, cx)))
                    })))
                .test_support()
        });

        let body = if row_els.is_empty() {
            div().id("todos-empty").py_12().flex().items_center().justify_center().text_sm().text_color(muted)
                .child(if self.loading { "Loading to-dos…" } else { "All caught up — no to-do items here." }).test_support().into_any_element()
        } else {
            div().w_full().flex().flex_col().pb_6().children(row_els).into_any_element()
        };

        div().id("todos-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                .child(div().pb_2().text_2xl().font_bold().text_color(fg).child("To-Do List"))
                .child(div().w_full().border_b_1().border_color(border).flex().items_end().justify_between().gap_4()
                    .child(tabs)
                    .child(div().pb_1p5().child(Button::new("todos-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                        .on_click(cx.listener(|v, _, _, cx| v.load(cx))))))
                .child(div().mt_3().mb_2().p_2().rounded_md().bg(head_bg).border_1().border_color(border)
                    .child(div().h(px(32.)).rounded_md().bg(bg).border_1().border_color(input_b).px_2().flex().items_center().gap_1p5()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.filter_input).appearance(false).cleanable(true).small()))
                        .child(Icon::new(IconName::Search).size_3p5().text_color(muted))))
                .children(bulk_bar)
                .child(div().id("todos-scroll").flex_1().min_h_0().overflow_y_scroll().pr_1().child(body)))
            .test_support()
    }
}
