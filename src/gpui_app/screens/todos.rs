use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Sizable as _,
    },
    div, px, uniform_list, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, Window,
};

use crate::gitlab::Todo;
use crate::nav::Screen;

use super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct TodosView {
    pub(crate) state: Entity<AppState>,
    pub(crate) todos: Vec<Todo>,
    pub(crate) load_task: Option<Task<()>>,
    _sub: Subscription,
}

impl TodosView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } => {
                this.load_task = None;
                this.todos.clear();
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Todos => this.load(cx),
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Todos =>
            {
                this.load(cx);
            }
            _ => {}
        });
        Self {
            state,
            todos: Vec::new(),
            load_task: None,
            _sub,
        }
    }
}

impl Render for TodosView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let todos = self.todos.clone();
        let count = todos.len();
        let weak = cx.entity().downgrade();

        let list = uniform_list("todos-uniform-list", count, move |range, _, cx| {
            let theme = cx.theme();
            range
                .map(|ix| {
                    let todo = todos[ix].clone();
                    let w_open = weak.clone();
                    let w_done = weak.clone();
                    let todo_open = todo.clone();
                    let todo_id = todo.id;
                    let proj = todo
                        .project
                        .as_ref()
                        .map(|p| p.path_with_namespace.clone())
                        .unwrap_or_default();
                    div()
                        .id(SharedString::from(format!("todo-row-{todo_id}")))
                        .h(px(62.))
                        .px_4()
                        .border_b_1()
                        .border_color(theme.border.opacity(0.5))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .on_click(move |_, _, cx| {
                            let _ = w_open.update(cx, |v, cx| v.open_todo(todo_open.clone(), cx));
                        })
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .px_1p5()
                                                .rounded_sm()
                                                .bg(theme.primary.opacity(0.18))
                                                .text_color(theme.primary)
                                                .text_xs()
                                                .child(todo.action_name.clone()),
                                        )
                                        .child(div().text_xs().text_color(theme.muted_foreground).child(proj)),
                                )
                                .child(div().text_sm().text_color(theme.foreground).truncate().child(todo.body)),
                        )
                        .child(
                            Button::new(SharedString::from(format!("todo-done-{todo_id}")))
                                .outline()
                                .xsmall()
                                .label("Done")
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let _ = w_done.update(cx, |v, cx| v.mark_done(todo_id, cx));
                                }),
                        )
                })
                .collect()
        })
        .flex_1()
        .size_full();

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_base().text_color(theme.foreground).child(format!("To-Do List ({count})")))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("todos-reload")
                                    .outline()
                                    .small()
                                    .label("Reload")
                                    .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                            )
                            .children((count > 0).then(|| {
                                Button::new("todos-mark-all")
                                    .danger()
                                    .small()
                                    .label("Mark all done")
                                    .on_click(cx.listener(|this, _, w, cx| this.mark_all_confirmed(w, cx)))
                            })),
                    ),
            )
            .child(list)
    }
}
