use std::collections::HashSet;

use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::{sort_releases, Release, ReleaseSortField, SortDir};
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::markdown::MarkdownProjectContext;
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};
use super::releases_card::render_release_card;

pub struct ReleasesView {
    pub(super) state: Entity<AppState>,
    releases: Vec<Release>,
    sort_field: ReleaseSortField,
    sort_dir: SortDir,
    pub(super) collapsed_assets: HashSet<String>,
    loading: bool,
    task: Option<Task<()>>,
    _sub: Subscription,
}

impl ReleasesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.loading = false;
                this.releases.clear();
                this.collapsed_assets.clear();
                if this.state.read(cx).screen == Screen::Releases { this.load(cx); }
                cx.notify();
            }
            AppEvent::ScreenChanged { screen } if *screen == Screen::Releases && this.releases.is_empty() => {
                this.load(cx);
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Releases => {
                this.load(cx);
            }
            _ => {}
        });
        Self {
            state, releases: Vec::new(), sort_field: ReleaseSortField::ReleasedAt,
            sort_dir: SortDir::Desc, collapsed_assets: HashSet::new(), loading: false, task: None, _sub,
        }
    }

    pub fn set_fixture_data(&mut self, releases: Vec<Release>, cx: &mut Context<Self>) {
        (self.releases, self.loading) = (releases, false);
        self.collapsed_assets.clear();
        cx.notify();
    }

    pub fn is_assets_collapsed(&self, tag: &str) -> bool { self.collapsed_assets.contains(tag) }

    pub(super) fn toggle_assets(&mut self, tag: String, cx: &mut Context<Self>) {
        if !self.collapsed_assets.remove(&tag) { self.collapsed_assets.insert(tag); }
        cx.notify();
    }

    pub fn set_sort_field(&mut self, field: ReleaseSortField, cx: &mut Context<Self>) {
        if self.sort_field != field { self.sort_field = field; cx.notify(); }
    }

    pub fn toggle_sort_dir(&mut self, cx: &mut Context<Self>) {
        self.sort_dir = self.sort_dir.toggle();
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
            let res = spawn_tokio(async move { client.releases(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
                match res {
                    Ok(Ok(items)) => v.releases = items,
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for ReleasesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (md_ctx, client, project_web) = {
            let st = self.state.read(cx);
            (
                MarkdownProjectContext::from_state(st, cx),
                st.gitlab.clone(),
                st.project.as_ref().map(|p| p.web_url.clone()).filter(|u| !u.is_empty()),
            )
        };
        let ordered = sort_releases(&self.releases, self.sort_field, self.sort_dir);
        let sort_field = self.sort_field;
        let sort_icon = match self.sort_dir { SortDir::Desc => IconName::SortDescending, SortDir::Asc => IconName::SortAscending };
        let weak = cx.entity().downgrade();

        let cards: Vec<_> = ordered.iter().enumerate().map(|(ix, rel)| {
            render_release_card(ix, rel, self.is_assets_collapsed(&rel.tag_name), md_ctx.clone(), client.as_ref(), &self.state, cx)
        }).collect();

        let theme = cx.theme();
        let header = div().w_full().pb_4().flex().items_center().justify_between().gap_4()
            .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Releases"))
            .child(
                div().flex().items_center().gap_1p5().flex_shrink_0()
                    .child(Button::new("rel-sort-field").outline().small().label(format!("{} ▾", sort_field.label()))
                        .dropdown_menu(move |mut menu, _, _| {
                            for f in ReleaseSortField::ALL {
                                let w = weak.clone();
                                menu = menu.item(PopupMenuItem::new(f.label()).checked(sort_field == f).on_click(move |_, _, cx| {
                                    let _ = w.update(cx, |v, cx| v.set_sort_field(f, cx));
                                }));
                            }
                            menu
                        }))
                    .child(Button::new("rel-sort-dir").outline().small().icon(Icon::new(sort_icon).size_3p5())
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sort_dir(cx))))
                    .child(Button::new("rel-reload").outline().small().icon(Icon::new(IconName::RefreshCw).size_3p5())
                        .on_click(cx.listener(|this, _, _, cx| this.load(cx))))
                    .children(project_web.map(|u| {
                        let url = format!("{}/-/releases", u.trim_end_matches('/'));
                        Button::new("rel-open-web").ghost().small().icon(Icon::new(IconName::ExternalLink).size_3p5())
                            .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
                    })),
            );

        let body = if cards.is_empty() {
            div().id("releases-empty").py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child(if self.loading { "Loading releases…" } else { "No releases yet." }).test_support().into_any_element()
        } else {
            div().w_full().flex().flex_col().gap_4().pb_6().children(cards).into_any_element()
        };

        div().id("releases-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(
                div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                    .child(header)
                    .child(div().id("releases-scroll").flex_1().min_h_0().overflow_y_scroll().pr_1().child(body)),
            )
            .test_support()
    }
}
