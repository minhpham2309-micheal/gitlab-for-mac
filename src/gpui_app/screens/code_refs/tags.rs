use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, Sizable as _,
    },
    div, px, uniform_list, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Task, Window,
};

use crate::gitlab::RepoTag;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct TagsView {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    tags: Vec<RepoTag>,
    loading: bool,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl TagsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter tags…"));
        let sub_in = cx.subscribe_in(&filter, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) { cx.notify(); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.loading = false;
                this.tags.clear();
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
        Self { state, filter, tags: Vec::new(), loading: false, task: None, _subs: vec![sub_in, sub_st] }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.loading = true;
        cx.notify();
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.repo_tags(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(items)) => v.tags = items,
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for TagsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let q = self.filter.read(cx).value().trim().to_ascii_lowercase();
        let rows: Vec<RepoTag> = self.tags.iter()
            .filter(|t| q.is_empty() || t.name.to_ascii_lowercase().contains(&q))
            .cloned().collect();
        let count = rows.len();
        let state = self.state.clone();
        let list = uniform_list("tags-uniform-list", count, move |range, _, cx| {
            range.map(|ix| {
                let tag = &rows[ix];
                let name = tag.name.clone();
                let st_browse = state.clone();
                let st_hist = state.clone();
                let name_browse = name.clone();
                let name_hist = name.clone();
                let sub = tag.commit.as_ref()
                    .map(|c| format!("{} · {}", if c.short_id.is_empty() { c.id.get(..8).unwrap_or(&c.id) } else { &c.short_id }, c.title))
                    .or_else(|| tag.message.clone())
                    .unwrap_or_default();
                let theme = cx.theme();
                div()
                    .h(px(56.)).px_4().border_b_1().border_color(theme.border.opacity(0.45))
                    .hover(|s| s.bg(theme.list_hover))
                    .flex().items_center().justify_between().gap_3()
                    .child(
                        div().flex().items_center().gap_2p5().min_w_0()
                            .child(Icon::empty().path("icons/glab-tag.svg").size_4().text_color(theme.muted_foreground))
                            .child(
                                div().flex().flex_col().min_w_0()
                                    .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.foreground).truncate().child(name))
                                    .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(sub)),
                            ),
                    )
                    .child(
                        div().flex().items_center().gap_1p5().flex_shrink_0()
                            .child(
                                Button::new(SharedString::from(format!("tag-browse-{ix}"))).outline().xsmall().label("Browse files")
                                    .on_click(move |_, _, cx| st_browse.update(cx, |s, cx| s.open_overview_ref(name_browse.clone(), String::new(), cx))),
                            )
                            .child(
                                Button::new(SharedString::from(format!("tag-hist-{ix}"))).ghost().xsmall().label("Commits")
                                    .on_click(move |_, _, cx| st_hist.update(cx, |s, cx| s.open_commit_history(Some(name_hist.clone()), None, cx))),
                            ),
                    )
            }).collect()
        }).flex_1().size_full();

        div().size_full().flex().flex_col()
            .child(
                div().px_4().py_3().border_b_1().border_color(theme.border)
                    .flex().items_center().justify_between().gap_3()
                    .child(div().text_base().text_color(theme.foreground).child(format!("Tags ({count})")))
                    .child(div().w(px(300.)).child(Input::new(&self.filter).small())),
            )
            .child(list)
    }
}
