use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState, Textarea, TextareaState},
        ActiveTheme as _, Sizable as _, WindowExt as _,
    },
    div, AppContext as _, Context, ParentElement as _, Styled as _, Window,
};

use super::super::super::safe_url::safe_open_url;
use super::MergeRequestsView;

impl MergeRequestsView {
    pub fn open_edit_mr_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mr) = self.selected.clone() else { return; };
        if !self.mr_permissions(&mr, cx).can_edit { return; }
        let st = self.state.read(cx);
        let (h_ep, p_ep, iid) = (st.host_epoch, st.project_epoch, mr.iid);
        let edit_web_url = format!("{}/edit", mr.web_url.trim_end_matches('/'));
        let title_val = mr.title.clone();
        let desc_val = mr.description.clone().unwrap_or_default();

        let title_input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Merge request title");
            input.set_value(title_val, window, cx);
            input
        });
        let desc_input = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .placeholder("Description (Markdown supported)");
            input.set_value(desc_val, window, cx);
            input
        });
        let weak = cx.entity().downgrade();

        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let t_ref = title_input.clone();
            let d_ref = desc_input.clone();
            let weak_ok = weak.clone();
            let web = edit_web_url.clone();
            let muted = cx.theme().muted_foreground;
            dialog
                .confirm()
                .centered()
                .title(format!("Edit merge request !{iid}"))
                .description("Update title and description, or open the full form in GitLab Web.")
                .ok_text("Save changes")
                .cancel_text("Cancel")
                .on_ok(move |_, _, cx| {
                    let title = t_ref.read(cx).value().trim().to_string();
                    let desc = d_ref.read(cx).value().to_string();
                    if !title.is_empty() {
                        let _ = weak_ok.update(cx, |v, cx| {
                            let cur = v.state.read(cx);
                            if cur.host_epoch == h_ep && cur.project_epoch == p_ep {
                                v.save_mr_edit(title, desc, cx);
                            }
                        });
                    }
                    true
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2p5()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_xs().text_color(muted).child("Title"))
                                .child(Input::new(&title_input)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_xs().text_color(muted).child("Description"))
                                .child(Textarea::new(&desc_input).h(gpui_kit::px(140.))),
                        )
                        .child(
                            div().flex().justify_end().child(
                                Button::new("mr-edit-open-web")
                                    .ghost()
                                    .xsmall()
                                    .label("Open full editor in GitLab Web ↗")
                                    .on_click(move |_, _, _| { let _ = safe_open_url(&web); }),
                            ),
                        ),
                )
        });
    }
}
