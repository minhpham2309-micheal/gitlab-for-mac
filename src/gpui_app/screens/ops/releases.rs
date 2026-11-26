use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Sizable as _,
    },
    div, px, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window,
};

use crate::gitlab::Release;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::markdown::{render_markdown_with_uploads, MarkdownProjectContext};
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct ReleasesView {
    state: Entity<AppState>,
    releases: Vec<Release>,
    task: Option<Task<()>>,
    _sub: Subscription,
}

impl ReleasesView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _sub = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.releases.clear();
                if this.state.read(cx).screen == Screen::Releases {
                    this.load(cx);
                }
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
        Self { state, releases: Vec::new(), task: None, _sub }
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.releases(project.id).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
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
        let md_ctx = MarkdownProjectContext::from_state(self.state.read(cx), cx);
        let cards: Vec<_> = self
            .releases
            .iter()
            .enumerate()
            .map(|(ix, rel)| {
                let title = rel.name.clone().unwrap_or_else(|| rel.tag_name.clone());
                let date = rel.released_at.as_deref().and_then(|s| s.get(..10)).unwrap_or("").to_string();
                let notes = rel.description.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "_No release notes._".into());
                let self_url = rel.links.as_ref().and_then(|l| l.self_url.clone());
                let md_el = render_markdown_with_uploads(format!("rel-md-{ix}"), notes, md_ctx.clone(), cx);
                let theme = cx.theme();
                div()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(1.))
                                            .rounded_full()
                                            .bg(theme.primary.opacity(0.18))
                                            .text_color(theme.primary)
                                            .text_xs()
                                            .font_family(theme.mono_font_family.clone())
                                            .child(rel.tag_name.clone()),
                                    )
                                    .child(div().text_sm().text_color(theme.foreground).child(title))
                                    .child(div().text_xs().text_color(theme.muted_foreground).child(date)),
                            )
                            .children(self_url.map(|url| {
                                Button::new(SharedString::from(format!("rel-web-{ix}")))
                                    .ghost()
                                    .xsmall()
                                    .label("Open web")
                                    .on_click(move |_, _, _| {
                                        let _ = safe_open_url(&url);
                                    })
                            })),
                    )
                    .child(md_el)
            })
            .collect();
        let theme = cx.theme();

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
                    .child(div().text_base().text_color(theme.foreground).child(format!("Releases ({})", self.releases.len())))
                    .child(Button::new("rel-reload").outline().small().label("Reload").on_click(cx.listener(|this, _, _, cx| this.load(cx)))),
            )
            .child(
                div()
                    .id("releases-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(cards),
            )
    }
}
