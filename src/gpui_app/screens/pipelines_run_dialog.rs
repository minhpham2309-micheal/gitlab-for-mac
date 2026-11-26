use gpui_kit::{
    component::{
        button::Button,
        input::{Input, InputState},
        Sizable as _, WindowExt as _,
    },
    div, AppContext as _, Context, ParentElement as _, SharedString, Styled as _, Window,
};

use crate::runtime::spawn_tokio;

use super::pipelines::PipelinesView;

impl PipelinesView {
    pub fn open_run_pipeline_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.state.read(cx);
        let (Some(client), Some(project)) = (st.gitlab.clone(), st.project.clone()) else {
            return;
        };
        let (h_ep, p_ep, offline) = (st.host_epoch, st.project_epoch, st.offline_mode);
        let default_branch = project.default_branch.clone().unwrap_or_else(|| "main".into());
        let branch_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Branch or tag…");
            input.set_value(default_branch.clone(), window, cx);
            input
        });
        let weak = cx.entity().downgrade();
        if self.branches.is_empty() && !offline {
            let pid = project.id;
            self.branches_task = Some(cx.spawn(async move |this, cx| {
                if let Ok(Ok(bs)) =
                    spawn_tokio(async move { client.branches(pid).await.map_err(|e| e.to_string()) })
                        .await
                {
                    let _ = this.update(cx, |v, cx| {
                        let cur = v.state.read(cx);
                        if cur.host_epoch == h_ep && cur.project_epoch == p_ep {
                            v.branches = bs.into_iter().map(|b| b.name).collect();
                            cx.notify();
                            cx.refresh_windows();
                        }
                    });
                }
            }));
        }
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let input_ref = branch_input.clone();
            let weak_ok = weak.clone();
            let chips = weak
                .read_with(cx, |v, _| v.branches.clone())
                .unwrap_or_default();
            dialog
                .confirm()
                .centered()
                .title("Run pipeline")
                .description("Select or enter a branch/tag to trigger a new CI pipeline.")
                .ok_text("Run pipeline")
                .on_ok(move |_, _, cx| {
                    let git_ref = input_ref.read(cx).value().trim().to_string();
                    if !git_ref.is_empty() {
                        let _ = weak_ok.update(cx, |v, cx| {
                            let cur = v.state.read(cx);
                            if cur.host_epoch == h_ep && cur.project_epoch == p_ep {
                                v.trigger_run_pipeline(git_ref, cx);
                            }
                        });
                    }
                    true
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Input::new(&branch_input))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_1()
                                .children(chips.into_iter().take(8).map(|name| {
                                    let inp = branch_input.clone();
                                    let n = name.clone();
                                    Button::new(SharedString::from(format!("br-{name}")))
                                        .outline()
                                        .xsmall()
                                        .label(name)
                                        .on_click(move |_, w, cx| {
                                            inp.update(cx, |i, cx| i.set_value(n.clone(), w, cx));
                                        })
                                })),
                        ),
                )
        });
    }
}
