use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use crate::nav::Screen;

use super::safe_url::safe_open_url;
use super::state::AppState;

pub const PANEL_BREADCRUMB_H: f32 = 40.0;

fn is_global_screen(screen: Screen) -> bool {
    matches!(screen, Screen::Projects | Screen::Todos | Screen::Console | Screen::Preferences)
}

pub fn render_panel_breadcrumb(state: &Entity<AppState>, cx: &App) -> Option<impl IntoElement + use<>> {
    let st = state.read(cx);
    let screen = st.screen;
    if screen == Screen::MergeRequests {
        return None;
    }

    let theme = cx.theme();
    let project = st.project.clone();
    let active_group_path = st.active_group_path.clone();
    let web_url = st
        .summary
        .active_web_url
        .clone()
        .or_else(|| project.as_ref().map(|p| p.web_url.clone()));

    let mut trail = div().flex().items_center().flex_wrap().gap_1p5().min_w_0().text_xs();

    if screen == Screen::Groups {
        if let Some(full_path) = active_group_path.as_deref().filter(|p| !p.is_empty()) {
            let st_root = state.clone();
            trail = trail.child(
                div()
                    .id("bc-groups-root")
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.foreground))
                    .on_click(move |_, _, cx| st_root.update(cx, |s, cx| s.open_group(None, cx)))
                    .child("Groups")
                    .test_support(),
            );
            let segs: Vec<&str> = full_path.split('/').filter(|s| !s.is_empty()).collect();
            let last_ix = segs.len().saturating_sub(1);
            let mut acc = String::new();
            for (ix, seg) in segs.into_iter().enumerate() {
                if !acc.is_empty() { acc.push('/'); }
                acc.push_str(seg);
                trail = trail.child(div().text_color(theme.muted_foreground).child("/"));
                if ix == last_ix {
                    trail = trail.child(div().font_semibold().text_color(theme.foreground).child(seg.to_string()));
                } else {
                    let target_path = acc.clone();
                    let st_seg = state.clone();
                    trail = trail.child(
                        div()
                            .id(SharedString::from(format!("bc-group-seg-{ix}")))
                            .cursor_pointer()
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.text_color(theme.foreground))
                            .on_click(move |_, _, cx| st_seg.update(cx, |s, cx| s.open_group(Some(target_path.clone()), cx)))
                            .child(seg.to_string())
                            .test_support(),
                    );
                }
            }
        } else {
            let st_work = state.clone();
            trail = trail
                .child(
                    div()
                        .id("bc-your-work")
                        .cursor_pointer()
                        .text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                        .on_click(move |_, _, cx| st_work.update(cx, |s, cx| s.set_screen(Screen::Projects, cx)))
                        .child("Your work")
                        .test_support(),
                )
                .child(div().text_color(theme.muted_foreground).child("/"))
                .child(div().font_semibold().text_color(theme.foreground).child("Groups"));
        }
    } else if is_global_screen(screen) || project.is_none() {
        let st_work = state.clone();
        trail = trail
            .child(
                div()
                    .id("bc-your-work")
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.foreground))
                    .on_click(move |_, _, cx| st_work.update(cx, |s, cx| s.set_screen(Screen::Projects, cx)))
                    .child("Your work")
                    .test_support(),
            )
            .child(div().text_color(theme.muted_foreground).child("/"))
            .child(div().font_semibold().text_color(theme.foreground).child(screen.label()));
    } else if let Some(project) = project {
        let segments: Vec<String> = project.path_with_namespace.split('/').map(str::to_owned).collect();
        let last_ix = segments.len().saturating_sub(1);
        let mut acc = String::new();
        for (ix, seg) in segments.into_iter().enumerate() {
            if ix > 0 {
                trail = trail.child(div().text_color(theme.muted_foreground).child("/"));
                acc.push('/');
            }
            acc.push_str(&seg);
            let is_project = ix == last_ix;
            let label = if is_project { project.name.clone() } else { seg };
            if is_project && screen == Screen::ProjectOverview {
                trail = trail.child(div().font_semibold().text_color(theme.foreground).child(label));
            } else {
                let st_seg = state.clone();
                let group_path = acc.clone();
                trail = trail.child(
                    div()
                        .id(SharedString::from(format!("ns-seg-{ix}")))
                        .cursor_pointer()
                        .text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                        .on_click(move |_, _, cx| {
                            if is_project {
                                st_seg.update(cx, |s, cx| s.set_screen(Screen::ProjectOverview, cx));
                            } else {
                                st_seg.update(cx, |s, cx| s.open_group(Some(group_path.clone()), cx));
                            }
                        })
                        .child(label)
                        .test_support(),
                );
            }
        }
        if screen != Screen::ProjectOverview {
            trail = trail
                .child(div().text_color(theme.muted_foreground).child("/"))
                .child(div().font_semibold().text_color(theme.foreground).child(screen.label()));
        }
    }

    let open_web = web_url.map(|url| {
        Button::new("panel-open-web")
            .ghost()
            .xsmall()
            .icon(Icon::new(IconName::ExternalLink).size_3p5())
            .on_click(move |_, _, _| { let _ = safe_open_url(&url); })
    });

    Some(
        div()
            .id("studio-panel-breadcrumb")
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
            .child(trail)
            .children(open_web)
            .test_support(),
    )
}
