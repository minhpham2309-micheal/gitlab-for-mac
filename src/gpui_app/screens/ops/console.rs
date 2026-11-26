use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
        ActiveTheme as _, Sizable as _,
    },
    div, px, rgb, uniform_list, AppContext as _, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription, Task, Window,
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
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.run_command(w, cx);
            }
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
        let Some(host) = st.active_host.as_ref().map(|h| h.hostname.clone()) else {
            return;
        };
        let host_epoch = st.host_epoch;
        let repo = st.project.as_ref().map(|p| p.path_with_namespace.clone());

        if let Some(sub) = is_sensitive_subcommand(&args) {
            let weak = cx.entity().downgrade();
            let preview = redact_tokens(&args.join(" "));
            open_destructive_confirm(
                DestructiveConfirm {
                    title: format!("Confirm sensitive glab '{sub}' command"),
                    action_label: "Run anyway".into(),
                    host: host.clone(),
                    project: repo.unwrap_or_else(|| "none".into()),
                    target: format!("glab {preview}"),
                    detail: "This subcommand can read or mutate local glab credentials/configuration.".into(),
                },
                move |_, cx| {
                    let _ = weak.update(cx, |v, cx| {
                        if v.state.read(cx).host_epoch == host_epoch {
                            v.exec(host.clone(), args.clone(), cx);
                        }
                    });
                },
                window,
                cx,
            );
            return;
        }
        inject_repo_flag(&mut args, repo.as_deref());
        self.exec(host, args, cx);
    }

    fn exec(&mut self, host: String, args: Vec<String>, cx: &mut Context<Self>) {
        let host_epoch = self.state.read(cx).host_epoch;
        self.state.update(cx, |s, cx| {
            s.busy = true;
            s.set_status("Running glab…", cx);
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let res = spawn_tokio(async move { glab::run(&host, args).await }).await;
            let _ = this.update(cx, |v, cx| {
                if v.state.read(cx).host_epoch != host_epoch {
                    return;
                }
                v.state.update(cx, |s, _| s.busy = false);
                let text = match res {
                    Ok(Ok(out)) => {
                        v.state.update(cx, |s, cx| s.set_status("glab finished", cx));
                        out
                    }
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
        let theme = cx.theme();
        let lines = self.output_lines.clone();
        let mono = theme.mono_font_family.clone();
        let mono_size = theme.mono_font_size;
        let row_h = px((f32::from(mono_size) * 1.55).ceil().max(20.));

        let output = uniform_list("console-output-list", lines.len(), move |range, _, _| {
            range
                .map(|ix| {
                    div()
                        .id(SharedString::from(format!("con-ln-{ix}")))
                        .h(row_h)
                        .px_3()
                        .font_family(mono.clone())
                        .text_size(mono_size)
                        .text_color(rgb(0xE8ECF1))
                        .whitespace_nowrap()
                        .child(lines[ix].clone())
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
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.input)))
                            .child(Button::new("console-run").primary().small().label("Run").on_click(cx.listener(|this, _, w, cx| this.run_command(w, cx)))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .children(QUICK_CMDS.iter().map(|&cmd| {
                                let inp = self.input.clone();
                                Button::new(SharedString::from(format!("qc-{cmd}")))
                                    .outline()
                                    .xsmall()
                                    .label(cmd)
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        inp.update(cx, |i, cx| i.set_value(cmd, w, cx));
                                        this.run_command(w, cx);
                                    }))
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().bg(rgb(0x121117)).child(output))
    }
}
