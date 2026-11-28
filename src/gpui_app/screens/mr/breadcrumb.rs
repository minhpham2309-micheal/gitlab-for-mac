use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::gitlab::Project;
use crate::nav::Screen;

use super::super::super::breadcrumb::PANEL_BREADCRUMB_H;
use super::super::super::safe_url::safe_open_url;
use super::super::super::state::AppState;
use super::MergeRequestsView;

pub fn render_mr_breadcrumb(
    project: &Project,
    active_mr_iid: Option<u64>,
    app_state: &Entity<AppState>,
    cx: &mut Context<MergeRequestsView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let segments: Vec<&str> = project.path_with_namespace.split('/').collect();
    let last_ix = segments.len().saturating_sub(1);
    let web_url = match active_mr_iid {
        Some(iid) => format!("{}/-/merge_requests/{iid}", project.web_url.trim_end_matches('/')),
        None => format!("{}/-/merge_requests", project.web_url.trim_end_matches('/')),
    };

    let mut row = div()
        .flex()
        .items_center()
        .flex_wrap()
        .gap_1p5()
        .min_w_0()
        .text_xs();

    for (ix, seg) in segments.into_iter().enumerate() {
        if ix > 0 {
            row = row.child(div().text_color(theme.muted_foreground).child("/"));
        }
        let is_project = ix == last_ix;
        let label = if is_project { project.name.clone() } else { seg.to_string() };
        let st = app_state.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("mr-bc-seg-{ix}")))
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.text_color(theme.foreground))
                .on_click(move |_, _, cx| {
                    let target = if is_project { Screen::ProjectOverview } else { Screen::Projects };
                    st.update(cx, |s, cx| s.set_screen(target, cx));
                })
                .child(label),
        );
    }

    row = row.child(div().text_color(theme.muted_foreground).child("/"));

    if let Some(iid) = active_mr_iid {
        row = row
            .child(
                div()
                    .id("mr-bc-list")
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.foreground))
                    .on_click(cx.listener(|this, _, _, cx| this.back_to_list(cx)))
                    .child("Merge requests")
                    .test_support(),
            )
            .child(div().text_color(theme.muted_foreground).child("/"))
            .child(
                div()
                    .id("mr-bc-iid")
                    .font_semibold()
                    .text_color(theme.foreground)
                    .child(format!("!{iid}"))
                    .test_support(),
            );
    } else {
        row = row.child(
            div()
                .id("mr-bc-list-active")
                .font_semibold()
                .text_color(theme.foreground)
                .child("Merge requests")
                .test_support(),
        );
    }

    div()
        .id("mr-breadcrumb-bar")
        .w_full()
        .h(px(PANEL_BREADCRUMB_H))
        .flex_shrink_0()
        .px_6()
        .border_b_1()
        .border_color(theme.border.opacity(0.75))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(row)
        .child(
            Button::new("mr-bc-open-web")
                .ghost()
                .xsmall()
                .icon(Icon::new(IconName::ExternalLink).size_3p5())
                .on_click(move |_, _, _| {
                    let _ = safe_open_url(&web_url);
                }),
        )
        .test_support()
}
