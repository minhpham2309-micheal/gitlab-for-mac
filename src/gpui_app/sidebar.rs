use gpui_kit::{
    component::{
        sidebar::{Sidebar, SidebarCollapsible, SidebarGroup, SidebarMenu, SidebarMenuItem},
        ActiveTheme as _, Icon, IconName,
    },
    div, prelude::FluentBuilder as _, px, App, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Styled as _, TestSupportExt as _,
};

use crate::nav::{Screen, SidebarSection};

use super::pajamas_theme::status_color;
use super::sidebar_footer::render_sidebar_footer;
use super::sidebar_project_card::{open_web_section, render_pinned_section, render_project_card};
use super::state::AppState;

fn screen_icon(screen: Screen) -> Icon {
    match screen {
        Screen::Projects => Icon::new(IconName::Folder),
        Screen::ProjectOverview => Icon::new(IconName::LayoutDashboard),
        Screen::Repository => Icon::new(IconName::FolderClosed),
        Screen::Todos => Icon::empty().path("icons/glab-todo.svg"),
        Screen::Issues => Icon::empty().path("icons/glab-issue.svg"),
        Screen::MergeRequests => Icon::empty().path("icons/glab-mr.svg"),
        Screen::Branches => Icon::empty().path("icons/glab-branch.svg"),
        Screen::Tags => Icon::empty().path("icons/glab-tag.svg"),
        Screen::Commits => Icon::new(IconName::FileText),
        Screen::Search => Icon::empty().path("icons/glab-search.svg"),
        Screen::Pipelines => Icon::empty().path("icons/glab-rocket.svg"),
        Screen::Schedules => Icon::new(IconName::Calendar),
        Screen::Runners => Icon::new(IconName::Cpu),
        Screen::Releases => Icon::empty().path("icons/glab-tag.svg"),
        Screen::Console => Icon::empty().path("icons/glab-terminal.svg"),
        Screen::Preferences => Icon::empty().path("icons/glab-settings.svg"),
    }
}

fn section_icon(section: SidebarSection) -> Icon {
    match section {
        SidebarSection::Manage => Icon::new(IconName::User),
        SidebarSection::Plan => Icon::empty().path("icons/glab-issue.svg"),
        SidebarSection::Code => Icon::empty().path("icons/glab-mr.svg"),
        SidebarSection::Build => Icon::empty().path("icons/glab-rocket.svg"),
        SidebarSection::Secure => Icon::new(IconName::Info),
        SidebarSection::Deploy => Icon::empty().path("icons/glab-tag.svg"),
        SidebarSection::Operate => Icon::new(IconName::Network),
        SidebarSection::Monitor => Icon::new(IconName::Eye),
        SidebarSection::Analyze => Icon::new(IconName::ChartPie),
    }
}

fn attach_count(item: SidebarMenuItem, count: Option<u32>) -> SidebarMenuItem {
    let Some(n) = count.filter(|&v| v > 0) else { return item; };
    let label = if n > 99 { "99+".to_string() } else { n.to_string() };
    item.suffix(move |_, cx| {
        let theme = cx.theme();
        div()
            .px_1p5()
            .rounded_full()
            .bg(theme.secondary)
            .text_color(theme.muted_foreground)
            .text_xs()
            .child(label.clone())
    })
}

pub fn render_sidebar(state: &Entity<AppState>, cx: &mut App) -> impl IntoElement + use<> {
    let st = state.read(cx);
    let collapsed = st.sidebar_collapsed;
    let active_screen = st.screen;
    let has_project = st.project.is_some();
    let sidebar_w = st.prefs.sidebar_width() as f32;
    let (todos_n, issues_n, mrs_n, live_pipes) =
        (st.counts.todos, st.counts.open_issues, st.counts.open_mrs, st.counts.live_pipelines);
    let nav_open_bits = st.nav_open;

    let make_leaf = |screen: Screen, disabled: bool| {
        let entity = state.clone();
        let item = SidebarMenuItem::new(screen.label())
            .icon(screen_icon(screen))
            .active(active_screen == screen)
            .disable(disabled)
            .on_click(move |_, _, cx| entity.update(cx, |s, cx| s.set_screen(screen, cx)));
        match screen {
            Screen::Todos => attach_count(item, todos_n),
            Screen::Issues => attach_count(item, issues_n),
            Screen::MergeRequests => attach_count(item, mrs_n),
            Screen::Pipelines if live_pipes > 0 => item.suffix(move |_, cx| {
                let c = status_color("running", cx.theme().is_dark());
                div()
                    .px_1p5()
                    .rounded_full()
                    .bg(c.opacity(0.18))
                    .text_color(c)
                    .text_xs()
                    .child(format!("● {live_pipes}"))
            }),
            _ => item,
        }
    };

    let mut project_menu = SidebarMenu::new();
    for section in SidebarSection::ALL {
        if let Some(native) = section.native() {
            let children = native.screens().iter().map(|&s| make_leaf(s, !has_project));
            let is_open = nav_open_bits & native.bit() != 0;
            project_menu = project_menu.child(
                SidebarMenuItem::new(section.label())
                    .icon(section_icon(section))
                    .default_open(is_open)
                    .click_to_toggle(true)
                    .disable(!has_project)
                    .children(children),
            );
        } else {
            let web_st = state.clone();
            project_menu = project_menu.child(
                SidebarMenuItem::new(section.label())
                    .icon(section_icon(section))
                    .disable(!has_project)
                    .suffix(|_, cx| {
                        Icon::new(IconName::ExternalLink)
                            .size_3()
                            .text_color(cx.theme().muted_foreground.opacity(0.65))
                    })
                    .on_click(move |_, _, cx| open_web_section(&web_st, section, cx)),
            );
        }
    }

    let border = cx.theme().sidebar_border;
    let project_card = render_project_card(state, collapsed, cx);
    let header = div()
        .id("sidebar-header-content")
        .w_full()
        .flex()
        .flex_col()
        .when(collapsed, |el| {
            el.items_center().pb_2().border_b_1().border_color(border.opacity(0.6))
        })
        .child(project_card)
        .when(!collapsed, |el| el.gap_2().child(render_pinned_section(state, false, cx)))
        .test_support();

    let mut tools_menu = SidebarMenu::new()
        .child(make_leaf(Screen::Projects, false))
        .child(make_leaf(Screen::Console, false))
        .child(make_leaf(Screen::Preferences, false));
    if collapsed {
        tools_menu = tools_menu.mt_2().pt_2().border_t_1().border_color(border.opacity(0.6));
    }

    Sidebar::new("glab-super-sidebar")
        .w(px(sidebar_w))
        .collapsible(SidebarCollapsible::Icon)
        .collapsed(collapsed)
        .header(header)
        .footer(render_sidebar_footer(state, collapsed, cx))
        .child(SidebarGroup::new("").child(project_menu))
        .child(SidebarGroup::new("Tools").child(tools_menu))
}
