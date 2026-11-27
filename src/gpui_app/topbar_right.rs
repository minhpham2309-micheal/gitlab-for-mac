use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        spinner::Spinner,
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _,
};

use crate::nav::Screen;

use super::avatar::render_avatar;
use super::pajamas_theme::status_color;
use super::safe_url::safe_open_url;
use super::state::{AppState, ScreenCommand};

fn count_pill(
    id: &'static str,
    icon_path: &'static str,
    count: Option<u32>,
    disabled: bool,
    target: Screen,
    state: Entity<AppState>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let label = count.map(|n| if n > 99 { "99+".into() } else { n.to_string() }).unwrap_or_else(|| "0".into());
    div()
        .id(id)
        .h(px(26.))
        .px_2()
        .rounded_md()
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .text_color(if disabled { theme.muted_foreground.opacity(0.45) } else { theme.foreground })
        .hover(|s| if disabled { s } else { s.bg(theme.secondary_hover) })
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            if !disabled {
                state.update(cx, |s, cx| s.set_screen(target, cx));
            }
        })
        .child(Icon::empty().path(icon_path).size_3p5().text_color(theme.muted_foreground))
        .child(SharedString::from(label))
}

pub fn render_top_bar_right(state: &Entity<AppState>, cx: &mut App) -> impl IntoElement + use<> {
    let st = state.read(cx);
    let has_project = st.project.is_some();
    let project_web = st.project.as_ref().map(|p| p.web_url.clone());
    let issues_count = st.counts.open_issues;
    let mrs_count = st.counts.open_mrs;
    let todos_count = st.counts.todos;
    let client = st.gitlab.clone();
    let user_avatar = st.current_user.as_ref().and_then(|u| u.avatar_url.clone());
    let user_label = st
        .current_user
        .as_ref()
        .map(|u| u.username.clone())
        .or_else(|| st.active_host.as_ref().and_then(|h| h.user.clone()))
        .unwrap_or_else(|| "U".into());

    let active_host_label = st
        .active_host
        .as_ref()
        .map(|h| h.hostname.clone())
        .unwrap_or_else(|| "No host".into());
    let perms = st.project_permissions;
    let show_update = st
        .update
        .should_show_topbar_pill(st.prefs.dismissed_update_version.as_deref());
    let update_label = st.update.topbar_label();
    let update_busy = st.update.is_busy();
    let plus_state = state.clone();
    let web_for_issue = project_web;
    let plus_btn = Button::new("topbar-quick-plus")
        .ghost()
        .xsmall()
        .icon(Icon::new(IconName::Plus).size_3p5())
        .dropdown_menu(move |mut menu, _, _| {
            let st_proj = plus_state.clone();
            menu = menu.item(PopupMenuItem::new("Switch project (⌘P)").on_click(move |_, _, cx| {
                st_proj.update(cx, |s, cx| s.set_screen(Screen::Projects, cx));
            }));
            if let Some(web) = web_for_issue.clone() {
                if perms.can_run_pipelines() {
                    let st_run = plus_state.clone();
                    menu = menu.item(PopupMenuItem::new("Run pipeline").on_click(move |_, _, cx| {
                        st_run.update(cx, |s, cx| {
                            s.set_screen(Screen::Pipelines, cx);
                            s.dispatch_command(ScreenCommand::RunPipeline, cx);
                        });
                    }));
                }
                if perms.can_create_mr() {
                    let mr_url = format!("{}/-/merge_requests/new", web.trim_end_matches('/'));
                    menu = menu.item(PopupMenuItem::new("New merge request (Web)").on_click(move |_, _, _| {
                        let _ = safe_open_url(&mr_url);
                    }));
                }
                if perms.can_create_issue() {
                    let issue_url = format!("{}/-/issues/new", web.trim_end_matches('/'));
                    menu = menu.item(PopupMenuItem::new("New issue (Web)").on_click(move |_, _, _| {
                        let _ = safe_open_url(&issue_url);
                    }));
                }
            }
            menu
        });

    let refresh_state = state.clone();
    let refresh_btn = Button::new("topbar-refresh")
        .ghost()
        .xsmall()
        .icon(Icon::new(IconName::RefreshCw).size_3p5())
        .on_click(move |_, _, cx| {
            refresh_state.update(cx, |s, cx| s.dispatch_command(ScreenCommand::Refresh, cx));
        });

    let account_state = state.clone();
    let account_header = format!("@{user_label} · {active_host_label}");
    let avatar_el = render_avatar(user_avatar.as_deref(), &user_label, px(24.), client.as_ref(), cx);
    let avatar_btn = Button::new("topbar-user-avatar")
        .ghost()
        .xsmall()
        .p_0()
        .rounded_full()
        .child(avatar_el)
        .dropdown_menu(move |menu, _, _| {
            let st_pref = account_state.clone();
            let st_upd = account_state.clone();
            let st_add = account_state.clone();
            let st_out = account_state.clone();
            menu.item(PopupMenuItem::label(account_header.clone()))
                .item(PopupMenuItem::separator())
                .item(PopupMenuItem::new("Preferences (⌘,)").on_click(move |_, _, cx| {
                    st_pref.update(cx, |s, cx| s.set_screen(Screen::Preferences, cx));
                }))
                .item(PopupMenuItem::new("Check for updates").on_click(move |_, _, cx| {
                    st_upd.update(cx, |s, cx| {
                        s.open_update_modal(cx);
                        s.check_for_updates(true, cx);
                    });
                }))
                .item(PopupMenuItem::new("Add / Switch account…").on_click(move |_, _, cx| {
                    st_add.update(cx, |s, cx| s.open_add_account(cx));
                }))
                .item(PopupMenuItem::separator())
                .item(PopupMenuItem::new("Sign out").on_click(move |_, _, cx| {
                    st_out.update(cx, |s, cx| s.sign_out(false, cx));
                }))
        });

    let update_pill = if show_update
        && let Some(label) = update_label
    {
        let upd_state = state.clone();
        let dark = cx.theme().is_dark();
        let accent = status_color(if update_busy { "running" } else { "success" }, dark);
        Some(
            div()
                .id("topbar-update-pill")
                .h(px(24.))
                .px_2()
                .rounded_full()
                .bg(accent.opacity(0.18))
                .border_1()
                .border_color(accent.opacity(0.45))
                .text_xs()
                .text_color(accent)
                .flex()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .hover(move |s| s.bg(accent.opacity(0.28)))
                .on_click(move |_, _, cx| {
                    upd_state.update(cx, |s, cx| s.open_update_modal(cx));
                })
                .children(update_busy.then(|| Spinner::new().xsmall()))
                .children((!update_busy).then(|| {
                    Icon::empty().path("icons/glab-rocket.svg").size_3().text_color(accent)
                }))
                .child(SharedString::from(label)),
        )
    } else {
        None
    };

    div()
        .flex()
        .items_center()
        .gap_1()
        .flex_shrink_0()
        .children(update_pill)
        .child(plus_btn)
        .child(count_pill("top-issues", "icons/glab-issue.svg", issues_count, !has_project, Screen::Issues, state.clone(), cx))
        .child(count_pill("top-mrs", "icons/glab-mr.svg", mrs_count, !has_project, Screen::MergeRequests, state.clone(), cx))
        .child(count_pill("top-todos", "icons/glab-todo.svg", todos_count, false, Screen::Todos, state.clone(), cx))
        .child(refresh_btn)
        .child(avatar_btn)
}
