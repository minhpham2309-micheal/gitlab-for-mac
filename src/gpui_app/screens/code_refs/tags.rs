use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    TestSupportExt as _, Window,
};

use crate::gitlab::{filter_and_sort_tags, Pipeline, RepoTag, SortDir, TagSortField};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};
use super::tags_rows::render_tag_row;

pub struct TagsView {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    tags: Vec<RepoTag>,
    pipelines: Vec<Pipeline>,
    sort_field: TagSortField,
    sort_dir: SortDir,
    loading: bool,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl TagsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by tag name"));
        let sub_in = cx.subscribe_in(&filter, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.loading = false;
                this.tags.clear();
                this.pipelines.clear();
                if this.state.read(cx).screen == Screen::Tags { this.load(cx); }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Tags => {
                if this.tags.is_empty() && !this.loading { this.load(cx); }
            }
            AppEvent::Command(ScreenCommand::Refresh)
                if this.state.read(cx).screen == Screen::Tags => { this.load(cx); }
            _ => {}
        });
        Self {
            state, filter, tags: Vec::new(), pipelines: Vec::new(),
            sort_field: TagSortField::Updated, sort_dir: SortDir::Desc,
            loading: false, task: None, _subs: vec![sub_in, sub_st],
        }
    }

    pub fn sort_field(&self) -> TagSortField { self.sort_field }
    pub fn sort_dir(&self) -> SortDir { self.sort_dir }

    pub fn set_sort_field(&mut self, field: TagSortField, cx: &mut Context<Self>) {
        if self.sort_field != field { self.sort_field = field; cx.notify(); }
    }

    pub fn toggle_sort_dir(&mut self, cx: &mut Context<Self>) {
        self.sort_dir = self.sort_dir.toggle();
        cx.notify();
    }

    pub fn set_fixture_data(&mut self, tags: Vec<RepoTag>, pipelines: Vec<Pipeline>, cx: &mut Context<Self>) {
        (self.tags, self.pipelines, self.loading) = (tags, pipelines, false);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.loading = true;
        cx.notify();
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move {
                let (tags, pipes) = tokio::join!(client.repo_tags(project.id), client.pipelines(project.id));
                tags.map(|t| (t, pipes.unwrap_or_default())).map_err(|e| e.to_string())
            }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok((tags, pipes))) => { v.tags = tags; v.pipelines = pipes; }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for TagsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.filter.read(cx).value().to_string();
        let rows = filter_and_sort_tags(&self.tags, &query, self.sort_field, self.sort_dir);
        let project_web = self.state.read(cx).project.as_ref().map(|p| p.web_url.clone()).filter(|u| !u.is_empty());
        let sort_field = self.sort_field;
        let sort_icon = match self.sort_dir { SortDir::Desc => IconName::SortDescending, SortDir::Asc => IconName::SortAscending };
        let weak = cx.entity().downgrade();
        let st_releases = self.state.clone();
        let theme = cx.theme();

        let header = div().w_full().pb_3().flex().items_start().justify_between().gap_4()
            .child(
                div().flex().flex_col().gap_1()
                    .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Tags"))
                    .child(div().text_xs().text_color(theme.muted_foreground).child("Tags give the ability to mark specific points in history as being important")),
            )
            .child(
                div().flex().items_center().gap_1p5().flex_shrink_0()
                    .child(Button::new("tags-open-releases").outline().small()
                        .icon(Icon::empty().path("icons/glab-rocket.svg").size_3p5().text_color(theme.muted_foreground)).label("Releases")
                        .on_click(move |_, _, cx| st_releases.update(cx, |s, cx| s.set_screen(Screen::Releases, cx))))
                    .child(Button::new("tags-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                        .on_click(cx.listener(|v, _, _, cx| v.load(cx))))
                    .children(project_web.clone().map(|u| {
                        let url = format!("{}/-/tags", u.trim_end_matches('/'));
                        Button::new("tags-open-web").ghost().small().icon(Icon::new(IconName::ExternalLink).size_3p5())
                            .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
                    })),
            );

        let filter_bar = div()
            .id("tags-filter-bar").mb_2().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border)
            .flex().items_center().gap_2()
            .child(
                div().flex_1().min_w_0().h(px(32.)).rounded_md().bg(theme.background)
                    .border_1().border_color(theme.input).px_2().flex().items_center().gap_1p5()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.filter).appearance(false).cleanable(true).small()))
                    .child(Icon::new(IconName::Search).size_3p5().text_color(theme.muted_foreground)),
            )
            .child(
                div().flex().items_center().flex_shrink_0()
                    .child(Button::new("tags-sort-field").outline().small().label(format!("{} ▾", sort_field.label()))
                        .dropdown_menu(move |mut menu, _, _| {
                            for f in TagSortField::ALL {
                                let w = weak.clone();
                                menu = menu.item(PopupMenuItem::new(f.label()).checked(sort_field == f).on_click(move |_, _, cx| {
                                    let _ = w.update(cx, |v, cx| v.set_sort_field(f, cx));
                                }));
                            }
                            menu
                        }))
                    .child(Button::new("tags-sort-dir").outline().small().icon(Icon::new(sort_icon).size_3p5())
                        .on_click(cx.listener(|v, _, _, cx| v.toggle_sort_dir(cx)))),
            )
            .test_support();

        let body = if rows.is_empty() {
            div().py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child(if self.loading { "Loading tags…" } else { "No tags found." }).into_any_element()
        } else {
            let mut list = Vec::with_capacity(rows.len());
            for (ix, tag) in rows.iter().enumerate() {
                list.push(render_tag_row(ix, tag, &self.pipelines, project_web.as_deref(), &self.state, cx));
            }
            div().w_full().flex().flex_col().pb_6().children(list).into_any_element()
        };

        div()
            .id("tags-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(
                div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                    .child(header).child(filter_bar)
                    .child(div().id("tags-scroll").flex_1().min_h_0().overflow_y_scroll().pr_1().child(body)),
            )
            .test_support()
    }
}
