use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, TestSupportExt as _, Window,
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
    searched: bool,
    loading: bool,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl SearchView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search code blobs in active project…"));
        let sub_in = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) { this.run_search(cx); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| match ev {
            AppEvent::HostChanged { .. } | AppEvent::ProjectChanged { .. } => {
                (this.task, this.searched, this.loading) = (None, false, false);
                this.hits.clear();
                cx.notify();
            }
            AppEvent::Command(ScreenCommand::Refresh) if this.state.read(cx).screen == Screen::Search => {
                this.run_search(cx);
            }
            _ => {}
        });
        Self { state, input, hits: Vec::new(), searched: false, loading: false, task: None, _subs: vec![sub_in, sub_st] }
    }

    pub fn set_fixture_data(&mut self, hits: Vec<SearchBlob>, cx: &mut Context<Self>) {
        (self.hits, self.searched, self.loading) = (hits, true, false);
        cx.notify();
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().trim().to_string();
        if query.is_empty() { return; }
        let st = self.state.read(cx);
        if st.offline_mode { return; }
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else { return; };
        let (h_ep, p_ep) = (st.host_epoch, st.project_epoch);
        (self.loading, self.searched) = (true, true);
        cx.notify();
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { client.search(project.id, &query).await.map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                let cur = v.state.read(cx);
                if cur.host_epoch != h_ep || cur.project_epoch != p_ep { return; }
                v.loading = false;
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
        let project_ns = self.state.read(cx).project.as_ref().map(|p| p.path_with_namespace.clone());
        let theme = cx.theme();
        let md_ctx = MarkdownProjectContext::default();
        let hit_count = self.hits.len();

        let body = if self.loading {
            div().py_12().flex().items_center().justify_center().text_sm().text_color(theme.muted_foreground)
                .child("Searching code…").into_any_element()
        } else if self.hits.is_empty() {
            div().id("search-empty").py_12().flex().flex_col().items_center().justify_center().gap_2()
                .child(div().size(px(52.)).rounded_full().bg(theme.list_head).border_1().border_color(theme.border).flex().items_center().justify_center()
                    .child(Icon::new(IconName::Search).size_5().text_color(theme.muted_foreground)))
                .child(div().text_sm().font_semibold().text_color(theme.foreground)
                    .child(if self.searched { "No matching code blobs found" } else { "Search repository code" }))
                .child(div().text_xs().text_color(theme.muted_foreground)
                    .child("Enter a symbol, filename, or phrase and press Enter to search."))
                .test_support().into_any_element()
        } else {
            div().w_full().flex().flex_col().gap_3().pb_6().children(self.hits.iter().enumerate().map(|(ix, hit)| {
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
                div().id(SharedString::from(format!("search-hit-{ix}")))
                    .rounded_md().border_1().border_color(theme.border).overflow_hidden().flex().flex_col()
                    .child(
                        div().px_3p5().py_2().bg(theme.list_head).border_b_1().border_color(theme.border)
                            .flex().items_center().justify_between().gap_2()
                            .child(div().flex().items_center().gap_2().min_w_0()
                                .child(Icon::new(IconName::File).size_3p5().text_color(theme.muted_foreground))
                                .child(div().text_xs().font_family(theme.mono_font_family.clone()).font_semibold().text_color(theme.foreground).truncate().child(path)))
                            .child(div().flex().items_center().gap_1p5().flex_shrink_0()
                                .child(div().px_1p5().py(px(1.)).rounded_sm().bg(theme.secondary).font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.muted_foreground).child(format!("{git_ref}:{line}")))
                                .child(Button::new(SharedString::from(format!("search-open-{ix}"))).outline().xsmall().label("Open file")
                                    .on_click(move |_, _, cx| {
                                        if let Some(pid) = open_state.read(cx).project.as_ref().map(|p| p.id) {
                                            open_state.update(cx, |s, cx| s.open_repo_file(pid, open_ref.clone(), open_path.clone(), None, open_line, cx));
                                        }
                                    }))),
                    )
                    .child(div().p_3().bg(theme.background).child(safe_markdown(SharedString::from(format!("search-snip-{ix}")), fenced, md_ctx.clone())))
                    .test_support()
            })).into_any_element()
        };

        div().id("search-screen").size_full().flex().flex_col().items_center().px_6().pt_3().overflow_hidden()
            .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col()
                .child(div().pb_2().flex().items_center().justify_between().gap_3()
                    .child(div().flex().items_center().gap_2p5().min_w_0()
                        .child(div().text_2xl().font_bold().text_color(theme.foreground).child("Search"))
                        .children(project_ns.map(|ns| div().px_2().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground).truncate().child(ns))))
                    .children(self.searched.then(|| div().text_xs().text_color(theme.muted_foreground).child(format!("{hit_count} matches")))))
                .child(div().mb_3().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border).flex().items_center().gap_2()
                    .child(div().flex_1().min_w_0().h(px(32.)).rounded_md().bg(theme.background).border_1().border_color(theme.input).px_2().flex().items_center().gap_1p5()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.input).appearance(false).cleanable(true).small()))
                        .child(Icon::new(IconName::Search).size_3p5().text_color(theme.muted_foreground)))
                    .child(Button::new("search-run").primary().small().label("Search").on_click(cx.listener(|this, _, _, cx| this.run_search(cx)))))
                .child(div().id("search-results-scroll").flex_1().min_h_0().overflow_y_scroll().pr_1().child(body)))
            .test_support()
    }
}
