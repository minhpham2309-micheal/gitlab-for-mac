use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, rgb, uniform_list, AppContext as _, ClipboardItem, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Task, TestSupportExt as _, Window,
};

use crate::gitlab::log_sanitize::strip_to_plain;
use crate::glab;
use crate::runtime::spawn_tokio;

use super::super::super::confirm::{open_destructive_confirm, DestructiveConfirm};
use super::super::super::state::{AppEvent, AppState};
use super::console_args::{inject_repo_flag, is_sensitive_subcommand, parse_console_args, redact_tokens};

const QUICK_CMDS: &[&str] = &["ci status", "ci list", "mr list", "issue list"];

pub struct ConsoleView {
    state: Entity<AppState>,
    input: Entity<InputState>,
    output_lines: Vec<String>,
    task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl ConsoleView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut st = InputState::new(window, cx).placeholder("glab command (e.g. ci status)…");
            st.set_value("ci status", window, cx);
            st
        });
        let sub_in = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, w, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) { this.run_command(w, cx); }
        });
        let sub_st = cx.subscribe_in(&state, window, |this, _, ev: &AppEvent, _, cx| {
            if matches!(ev, AppEvent::HostChanged { .. }) {
                this.task = None;
                this.output_lines.clear();
                cx.notify();
            }
        });
        Self { state, input, output_lines: Vec::new(), task: None, _subs: vec![sub_in, sub_st] }
    }

    pub fn line_count(&self) -> usize { self.output_lines.len() }

    pub fn set_fixture_output(&mut self, lines: Vec<String>, cx: &mut Context<Self>) {
        self.output_lines = lines;
        cx.notify();
    }

    fn run_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.input.read(cx).value().to_string();
        let mut args = match parse_console_args(&raw) {
            Ok(a) if !a.is_empty() => a,
            Ok(_) => return,
            Err(err) => {
                self.output_lines = vec![err.clone()];
                self.state.update(cx, |s, cx| s.set_status(err, cx));
                cx.notify();
                return;
            }
        };
        let st = self.state.read(cx);
        let Some(host) = st.active_host.as_ref().map(|h| h.hostname.clone()) else { return; };
        let host_epoch = st.host_epoch;
        let repo = st.project.as_ref().map(|p| p.path_with_namespace.clone());

        if let Some(sub) = is_sensitive_subcommand(&args) {
            let weak = cx.entity().downgrade();
            let preview = redact_tokens(&args.join(" "));
            open_destructive_confirm(
                DestructiveConfirm {
                    title: format!("Confirm sensitive glab '{sub}' command"), action_label: "Run anyway".into(),
                    host: host.clone(), project: repo.unwrap_or_else(|| "none".into()), target: format!("glab {preview}"),
                    detail: "This subcommand can read or mutate local glab credentials/configuration.".into(),
                },
                move |_, cx| {
                    let _ = weak.update(cx, |v, cx| {
                        if v.state.read(cx).host_epoch == host_epoch { v.exec(host.clone(), args.clone(), cx); }
                    });
                },
                window, cx,
            );
            return;
        }
        inject_repo_flag(&mut args, repo.as_deref());
        self.exec(host, args, cx);
    }

    fn exec(&mut self, host: String, args: Vec<String>, cx: &mut Context<Self>) {
        let host_epoch = self.state.read(cx).host_epoch;
        self.state.update(cx, |s, cx| { s.busy = true; s.set_status("Running glab…", cx); });
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { glab::run(&host, args).await }).await;
            let _ = this.update(cx, |v, cx| {
                if v.state.read(cx).host_epoch != host_epoch { return; }
                v.state.update(cx, |s, _| s.busy = false);
                let text = match res {
                    Ok(Ok(out)) => { v.state.update(cx, |s, cx| s.set_status("glab finished", cx)); out }
                    Ok(Err(err)) | Err(err) => {
                        let safe = redact_tokens(&err);
                        v.state.update(cx, |s, cx| s.set_status(safe.clone(), cx));
                        safe
                    }
                };
                let cleaned = redact_tokens(&strip_to_plain(&text));
                v.output_lines = cleaned.lines().map(str::to_string).collect();
                cx.notify();
            });
        }));
    }
}

impl Render for ConsoleView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (host_label, repo_label) = {
            let st = self.state.read(cx);
            (
                st.active_host.as_ref().map(|h| h.hostname.clone()).unwrap_or_default(),
                st.project.as_ref().map(|p| p.path_with_namespace.clone()),
            )
        };
        let theme = cx.theme();
        let dark = theme.is_dark();
        let term_bg = if dark { rgb(0x0D0D0D) } else { rgb(0x161618) };
        let lines = self.output_lines.clone();
        let has_out = !lines.is_empty();
        let copy_text = lines.join("\n");
        let st_copy = self.state.clone();
        let mono = theme.mono_font_family.clone();
        let mono_size = theme.mono_font_size;
        let row_h = px((f32::from(mono_size) * 1.55).ceil().max(20.));

        let terminal_body = if lines.is_empty() {
            div().id("console-empty").flex_1().flex().items_center().justify_center()
                .font_family(mono).text_xs().text_color(rgb(0x898889))
                .child("Run a glab command or pick a quick shortcut above.").test_support().into_any_element()
        } else {
            uniform_list("console-output-list", lines.len(), move |range, _, _| {
                range.map(|ix| {
                    div().id(SharedString::from(format!("con-ln-{ix}"))).h(row_h).px_3()
                        .font_family(mono.clone()).text_size(mono_size).text_color(rgb(0xECECEC))
                        .whitespace_nowrap().child(lines[ix].clone())
                }).collect()
            }).flex_1().size_full().into_any_element()
        };

        div().id("console-screen").size_full().flex().flex_col().items_center().px_6().pt_3().pb_4().overflow_hidden()
            .child(div().w_full().max_w(px(1200.)).flex_1().min_h_0().flex().flex_col().gap_3()
                .child(div().flex().items_center().justify_between().gap_3()
                    .child(div().flex().items_center().gap_2p5().min_w_0()
                        .child(div().text_2xl().font_bold().text_color(theme.foreground).child("glab Console"))
                        .children((!host_label.is_empty()).then(|| div().px_2().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground).child(host_label)))
                        .children(repo_label.map(|r| div().px_2().py(px(1.)).rounded_full().bg(theme.secondary).border_1().border_color(theme.border).text_xs().text_color(theme.muted_foreground).truncate().child(r))))
                    .child(div().flex().items_center().gap_1p5()
                        .children(has_out.then(|| Button::new("console-copy").outline().xsmall().icon(Icon::new(IconName::Copy).size_3()).label("Copy")
                            .on_click(move |_, _, cx| { cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone())); st_copy.update(cx, |s, cx| s.set_status("Copied console output", cx)); })))
                        .children(has_out.then(|| Button::new("console-clear").ghost().xsmall().label("Clear")
                            .on_click(cx.listener(|v, _, _, cx| { v.output_lines.clear(); cx.notify(); }))))))
                .child(div().p_2().rounded_md().bg(theme.list_head).border_1().border_color(theme.border).flex().flex_col().gap_2()
                    .child(div().flex().items_center().gap_2()
                        .child(div().flex_1().min_w_0().h(px(32.)).rounded_md().bg(theme.background).border_1().border_color(theme.input).px_2().flex().items_center().gap_1p5()
                            .child(div().font_family(theme.mono_font_family.clone()).text_xs().text_color(theme.muted_foreground).child("glab"))
                            .child(div().flex_1().min_w_0().child(Input::new(&self.input).appearance(false).cleanable(true).small())))
                        .child(Button::new("console-run").primary().small().label("Run").on_click(cx.listener(|this, _, w, cx| this.run_command(w, cx)))))
                    .child(div().flex().items_center().gap_1p5().children(QUICK_CMDS.iter().map(|&cmd| {
                        let inp = self.input.clone();
                        Button::new(SharedString::from(format!("qc-{cmd}"))).outline().xsmall().label(cmd)
                            .on_click(cx.listener(move |this, _, w, cx| { inp.update(cx, |i, cx| i.set_value(cmd, w, cx)); this.run_command(w, cx); }))
                    }))))
                .child(div().flex_1().min_h_0().rounded_md().border_1().border_color(theme.border).bg(term_bg).py_2().overflow_hidden().flex().flex_col().child(terminal_body)))
            .test_support()
    }
}
