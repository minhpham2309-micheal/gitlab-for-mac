use std::rc::Rc;

use gpui_kit::{
    component::{button::ButtonVariant, ActiveTheme as _, WindowExt as _},
    div, App, ParentElement as _, Styled as _, Window,
};

#[derive(Debug, Clone)]
pub struct DestructiveConfirm {
    pub title: String,
    pub action_label: String,
    pub host: String,
    pub project: String,
    pub target: String,
    pub detail: String,
}

pub fn format_confirm_summary(req: &DestructiveConfirm) -> String {
    format!(
        "Host: {}\nProject: {}\nTarget: {}\n\n{}",
        req.host, req.project, req.target, req.detail
    )
}

pub fn open_destructive_confirm(
    req: DestructiveConfirm,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let on_confirm = Rc::new(on_confirm);
    window.open_alert_dialog(cx, move |alert, _, cx| {
        let on_confirm = Rc::clone(&on_confirm);
        let theme = cx.theme();
        let req = req.clone();
        alert
            .confirm()
            .centered()
            .title(req.title.clone())
            .description(req.detail.clone())
            .ok_text(req.action_label.clone())
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("Cancel")
            .on_ok(move |_, window, cx| {
                on_confirm(window, cx);
                true
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .bg(theme.muted)
                    .border_1()
                    .border_color(theme.border)
                    .text_xs()
                    .font_family(theme.mono_font_family.clone())
                    .child(format!("Host:    {}", req.host))
                    .child(format!("Project: {}", req.project))
                    .child(format!("Target:  {}", req.target)),
            )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_summary_includes_host_project_and_target() {
        let req = DestructiveConfirm {
            title: "Merge MR".into(),
            action_label: "Merge".into(),
            host: "gitlab.example.com".into(),
            project: "team/service".into(),
            target: "!42".into(),
            detail: "This will merge !42 into main.".into(),
        };
        let summary = format_confirm_summary(&req);
        assert!(summary.contains("gitlab.example.com"));
        assert!(summary.contains("team/service"));
        assert!(summary.contains("!42"));
    }
}
