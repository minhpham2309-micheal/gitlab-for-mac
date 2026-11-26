use gpui_kit::{
    component::{tooltip::Tooltip, ActiveTheme as _, Icon, IconName},
    div, prelude::FluentBuilder as _, px, App, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _,
};

use crate::nav::{Screen, SidebarSection};
use crate::text_util::ellipsis;

use super::avatar::render_avatar;
use super::safe_url::safe_open_url;
use super::state::AppState;

pub fn build_web_section_url(web_url: &str, default_branch: &str, section: SidebarSection) -> Option<String> {
    let subpath = section.web_subpath(default_branch)?;
    Some(format!("{}{subpath}", web_url.trim_end_matches('/')))
}

pub fn render_project_card(state: &Entity<AppState>, collapsed: bool, cx: &mut App) -> impl IntoElement {
    let st = state.read(cx);
    let active_overview = st.screen == Screen::ProjectOverview;
    let client = st.gitlab.clone();
    let project = st.project.clone();
    let name = project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "Select project…".into());
    let av_url = project.as_ref().and_then(|p| p.avatar_url.clone());
    let has_proj = project.is_some();

    let av_size = if collapsed { px(24.) } else { px(26.) };
    let av = div()
        .id("sidebar-project-avatar")
        .rounded_full()
        .flex_shrink_0()
        .child(render_avatar(av_url.as_deref(), &name, av_size, client.as_ref(), cx))
        .test_support();
    let theme = cx.theme();
    let click_state = state.clone();
    let tooltip_name: SharedString = name.clone().into();
    let resting_bg = if active_overview {
        theme.sidebar_accent
    } else if collapsed {
        gpui_kit::transparent_black()
    } else {
        theme.secondary.opacity(0.6)
    };
    let fg = if active_overview { theme.sidebar_accent_foreground } else { theme.sidebar_foreground };

    let card = div()
        .id("sidebar-project-card")
        .w_full()
        .rounded_md()
        .cursor_pointer()
        .bg(resting_bg)
        .text_color(fg)
        .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.sidebar_accent_foreground))
        .flex()
        .items_center()
        .when(collapsed, |el| {
            el.h(px(32.)).justify_center().tooltip(move |w, cx| Tooltip::new(tooltip_name.clone()).build(w, cx))
        })
        .when(!collapsed, |el| el.px_2().py_1p5().gap_2())
        .on_click(move |_, _, cx| {
            click_state.update(cx, |s, cx| {
                s.set_screen(if has_proj { Screen::ProjectOverview } else { Screen::Projects }, cx);
            });
        })
        .child(av)
        .when(!collapsed, |el| {
            el.child(div().flex_1().min_w_0().text_xs().truncate().child(ellipsis(&name, 26)))
        })
        .test_support();

    if collapsed {
        return card.into_any_element();
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().px_2().text_xs().text_color(theme.muted_foreground).child("Project"))
        .child(card)
        .into_any_element()
}

pub fn render_pinned_section(state: &Entity<AppState>, collapsed: bool, cx: &App) -> impl IntoElement {
    let st = state.read(cx);
    let theme = cx.theme();
    let (open, has_proj, active, pinned) = (st.pinned_open, st.project.is_some(), st.screen, st.prefs.pinned());
    let (issues_n, mrs_n, todos_n) = (st.counts.open_issues, st.counts.open_mrs, st.counts.todos);
    let toggle_st = state.clone();

    if collapsed {
        return div().into_any_element();
    }

    let mut box_el = div()
        .w_full()
        .flex()
        .flex_col()
        .gap_0p5()
        .pb_2()
        .border_b_1()
        .border_color(theme.sidebar_border.opacity(0.6))
        .child(
            div()
                .id("sidebar-pinned-toggle")
                .w_full()
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.sidebar_accent))
                .flex()
                .items_center()
                .justify_between()
                .on_click(move |_, _, cx| toggle_st.update(cx, |s, cx| s.toggle_pinned(cx)))
                .child(div().text_xs().text_color(theme.muted_foreground).child("Pinned"))
                .child(
                    Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight })
                        .size_3()
                        .text_color(theme.muted_foreground),
                )
                .test_support(),
        );

    if open {
        for screen in pinned {
            let cnt = match screen {
                Screen::Issues => issues_n.unwrap_or(0),
                Screen::MergeRequests => mrs_n.unwrap_or(0),
                Screen::Todos => todos_n.unwrap_or(0),
                _ => 0,
            };
            let click_st = state.clone();
            let is_active = active == screen;
            let label_fg = if has_proj { theme.sidebar_foreground } else { theme.muted_foreground.opacity(0.5) };
            box_el = box_el.child(
                div()
                    .id(SharedString::from(format!("pinned-{}", screen.key())))
                    .w_full()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if is_active { theme.sidebar_accent } else { gpui_kit::transparent_black() })
                    .when(has_proj, |s| {
                        s.cursor_pointer()
                            .hover(|h| h.bg(theme.sidebar_accent))
                            .on_click(move |_, _, cx| click_st.update(cx, |s, cx| s.set_screen(screen, cx)))
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(label_fg).child(screen.label()))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(cnt.to_string()))
                    .test_support(),
            );
        }
    }
    box_el.into_any_element()
}

pub fn open_web_section(state: &Entity<AppState>, section: SidebarSection, cx: &mut App) {
    let url = {
        let st = state.read(cx);
        st.project.as_ref().and_then(|p| {
            let br = p.default_branch.as_deref().unwrap_or("main");
            build_web_section_url(&p.web_url, br, section)
        })
    };
    if let Some(url) = url {
        let msg = match safe_open_url(&url) {
            Ok(m) | Err(m) => m,
        };
        state.update(cx, |s, cx| s.set_status(msg, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpui_app::safe_url::validate_open_url;

    #[test]
    fn builds_valid_gitlab_web_urls_for_all_non_native_sections() {
        for sec in [SidebarSection::Manage, SidebarSection::Secure, SidebarSection::Operate, SidebarSection::Monitor, SidebarSection::Analyze] {
            let url = build_web_section_url("https://gl.example.com/team/app/", "main", sec).unwrap();
            assert!(validate_open_url(&url).is_ok(), "invalid url {url}");
            assert!(url.starts_with("https://gl.example.com/team/app/-/"));
        }
    }
}
