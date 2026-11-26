use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Sizable as _,
    },
    div, px, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, Window,
};

use crate::gitlab::SearchBlob;
use crate::nav::Screen;
use crate::runtime::spawn_tokio;

use super::super::super::diff_hl::language_from_path;
use super::super::super::markdown::{safe_markdown, MarkdownProjectContext};
use super::super::super::state::{AppEvent, AppState, ScreenCommand};

pub struct SearchView {
    state: Entity<AppState>,
    input: Entity<InputState>,
    hits: Vec<SearchBlob>,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl SearchView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search code blobs in active project…"));
        let sub_in = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.run_search(cx);
            }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                this.task = None;
                this.hits.clear();
                cx.notify();
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Search => {
                this.run_search(cx);
            }
            _ => {}
        });
        Self { state, input, hits: Vec::new(), task: None, _subs: vec![sub_in, sub_st] }
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().trim().to_string();
        if query.is_empty() { return; }
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.search(project.id, &query).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                match res {
                    Ok(Ok(items)) => {
                        let n = items.len();
                        v.hits = items;
                        v.state.update(cx, |s, cx| s.set_status(format!("{n} search matches"), cx));
                    }
                    Ok(Err(err)) | Err(err) => v.state.update(cx, |s, cx| s.set_status(err, cx)),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for SearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let md_ctx = MarkdownProjectContext::default();

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
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.input)))
                    .child(Button::new("search-run").primary().small().label("Search").on_click(cx.listener(|this, _, _, cx| this.run_search(cx)))),
            )
            .child(
                div()
                    .id("search-results-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(self.hits.iter().enumerate().map(|(ix, hit)| {
                        let path = hit.path.clone().or_else(|| hit.filename.clone()).unwrap_or_else(|| "file".into());
                        let lang = language_from_path(&path);
                        let line = hit.startline.unwrap_or(1);
                        let git_ref = hit.git_ref.clone().unwrap_or_default();
                        let snippet = hit.data.clone().unwrap_or_default().replace("```", "``\u{200b}`");
                        let fenced = format!("```{lang}\n{snippet}\n```");
                        let open_state = self.state.clone();
                        let open_path = path.clone();
                        let open_ref = (!git_ref.is_empty()).then(|| git_ref.clone());
                        let open_line = u32::try_from(line).ok();
                        div()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.sidebar)
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.foreground).truncate().child(path))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .flex_shrink_0()
                                            .child(div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).text_xs().text_color(theme.muted_foreground).child(format!("{git_ref}:{line}")))
                                            .child(
                                                Button::new(SharedString::from(format!("search-open-{ix}")))
                                                    .outline()
                                                    .xsmall()
                                                    .label("Open file")
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(pid) = open_state.read(cx).project.as_ref().map(|p| p.id) {
                                                            open_state.update(cx, |s, cx| {
                                                                s.open_repo_file(pid, open_ref.clone(), open_path.clone(), None, open_line, cx);
                                                            });
                                                        }
                                                    }),
                                            ),
                                    ),
                            )
                            .child(div().p_2().child(safe_markdown(SharedString::from(format!("search-snip-{ix}")), fenced, md_ctx.clone())))
                    })),
            )
    }
}
