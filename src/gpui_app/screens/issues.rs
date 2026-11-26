use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::InputState,
        resizable::{h_resizable, resizable_panel},
        ActiveTheme as _, Selectable as _, Sizable as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, Window,
};

use crate::gitlab::{Issue, IssueFilter, Note};
use crate::nav::Screen;

use super::super::markdown::MarkdownProjectContext;
use super::super::state::{AppEvent, AppState, ScreenCommand};
use super::issues_detail::render_issue_detail;

pub struct IssuesView {
    pub(crate) state: Entity<AppState>,
    pub(crate) filter: IssueFilter,
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
        let comment_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add a comment…"));
        let sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.list_task = None;
                this.notes_task = None;
                this.issues.clear();
                this.selected = None;
                this.notes.clear();
                if this.state.read(cx).screen == Screen::Issues {
                    this.load_list(cx);
                }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen }
                if *screen == Screen::Issues && this.issues.is_empty() =>
            {
                this.load_list(cx);
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Issues =>
            {
                this.load_list(cx);
            }
            AppEvent::Command(ScreenCommand::OpenIssue { iid, .. }) => {
                this.pending_iid = Some(*iid);
                this.load_list(cx);
            }
            _ => {}
        });
        Self {
            state,
            filter: IssueFilter::Open,
            issues: Vec::new(),
            selected: None,
            pending_iid: None,
            notes: Vec::new(),
            comment_input,
            list_task: None,
            notes_task: None,
            _subs: vec![sub],
        }
    }
}

impl Render for IssuesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let issues = self.issues.clone();
        let selected_iid = self.selected.as_ref().map(|i| i.iid);
        let weak = cx.entity().downgrade();

        let filter_bar = div()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .children(IssueFilter::ALL.into_iter().map(|f| {
                Button::new(SharedString::from(format!("iss-f-{}", f.api_state())))
                    .ghost()
                    .xsmall()
                    .selected(self.filter == f)
                    .label(f.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter = f;
                        this.load_list(cx);
                    }))
            }));

        let list = uniform_list("issues-uniform-list", issues.len(), move |range, _, cx| {
            let theme = cx.theme();
            range
                .map(|ix| {
                    let iss = issues[ix].clone();
                    let active = selected_iid == Some(iss.iid);
                    let w = weak.clone();
                    let clicked = iss.clone();
                    div()
                        .id(SharedString::from(format!("iss-row-{}", iss.iid)))
                        .h(px(56.))
                        .px_3()
                        .border_b_1()
                        .border_color(theme.border.opacity(0.5))
                        .cursor_pointer()
                        .bg(if active {
                            theme.list_active
                        } else {
                            theme.colors.list
                        })
                        .hover(|s| s.bg(theme.list_hover))
                        .flex()
                        .flex_col()
                        .justify_center()
                        .on_click(move |_, _, cx| {
                            let _ = w.update(cx, |v, cx| v.select_issue(clicked.clone(), cx));
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .truncate()
                                .child(format!("#{} {}", iss.iid, iss.title)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("{} · 💬 {}", iss.state, iss.user_notes_count)),
                        )
                })
                .collect()
        })
        .flex_1()
        .size_full();

        let left = div()
            .size_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme.border)
            .child(filter_bar)
            .child(list);

        let md_ctx = MarkdownProjectContext::from_state(self.state.read(cx), cx);
        let client = self.state.read(cx).gitlab.clone();
        let right = render_issue_detail(
            self.selected.clone(),
            &self.notes,
            &self.comment_input,
            md_ctx,
            client.as_ref(),
            cx,
        );

        h_resizable("issues-split")
            .child(
                resizable_panel()
                    .size(px(320.))
                    .size_range(px(240.)..px(500.))
                    .child(left),
            )
            .child(resizable_panel().child(right))
    }
}
