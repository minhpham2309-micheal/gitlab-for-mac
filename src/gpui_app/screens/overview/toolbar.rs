use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, ClipboardItem, Context, IntoElement, ParentElement as _, SharedString, Styled as _,
};

use crate::gitlab::{Project, ProjectDetail};
use crate::text_util::ellipsis;

use super::super::super::safe_url::safe_open_url;
use super::super::super::state::ScreenCommand;
use super::ProjectOverviewView;

pub fn render_repo_toolbar(
    view: &ProjectOverviewView,
    project: &Project,
    detail: Option<&ProjectDetail>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let current_ref = view.current_ref.clone();
    let branches: Vec<String> = view.branches.iter().map(|b| b.name.clone()).collect();
    let tags: Vec<String> = view.tags.iter().map(|t| t.name.clone()).collect();
    let view_weak = cx.entity().downgrade();

    let ref_dropdown = Button::new("ov-ref-select")
        .outline()
        .small()
        .icon(Icon::empty().path("icons/glab-branch.svg").size_3p5())
        .label(ellipsis(&current_ref, 22))
        .dropdown_menu(move |mut menu, _, _| {
            for br in branches.iter().take(40) {
                let name = br.clone();
                let active = *br == current_ref;
                let weak = view_weak.clone();
                menu = menu.item(PopupMenuItem::new(name.clone()).checked(active).on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.select_ref(name.clone(), cx));
                }));
            }
            if !tags.is_empty() {
                menu = menu.separator();
                for tag in tags.iter().take(25) {
                    let name = tag.clone();
                    let active = *tag == current_ref;
                    let weak = view_weak.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("tag: {name}")).checked(active).on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.select_ref(name.clone(), cx));
                        }),
                    );
                }
            }
            menu
        });

    let repo_slug = detail
        .map(|d| d.path.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| project.path_with_namespace.rsplit('/').next().unwrap_or(&project.name))
        .to_string();

    let mut path_crumb = div()
        .flex()
        .items_center()
        .gap_1()
        .min_w_0()
        .overflow_hidden()
        .child(
            Button::new("ov-path-root")
                .ghost()
                .xsmall()
                .label(repo_slug)
                .on_click(cx.listener(|this, _, _, cx| this.navigate_dir(String::new(), cx))),
        );

    if !view.current_path.is_empty() {
        let mut acc = String::new();
        for (ix, seg) in view.current_path.split('/').enumerate() {
            if !acc.is_empty() { acc.push('/'); }
            acc.push_str(seg);
            let target = acc.clone();
            path_crumb = path_crumb
                .child(div().text_xs().text_color(theme.muted_foreground).child("/"))
                .child(
                    Button::new(SharedString::from(format!("ov-path-{ix}")))
                        .ghost()
                        .xsmall()
                        .label(seg.to_string())
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate_dir(target.clone(), cx))),
                );
        }
    }

    let web_base = project.web_url.trim_end_matches('/').to_string();
    let ref_enc = urlencoding::encode(&view.current_ref).into_owned();
    let ssh_url = detail.and_then(|d| d.ssh_url_to_repo.clone());
    let http_url = detail
        .and_then(|d| d.http_url_to_repo.clone())
        .or_else(|| Some(format!("{web_base}.git")));
    let zip_url = format!("{web_base}/-/archive/{ref_enc}/archive.zip");
    let tar_url = format!("{web_base}/-/archive/{ref_enc}/archive.tar.gz");
    let state_copy = view.state.clone();
    let web_new = web_base.clone();
    let ref_new = ref_enc.clone();

    let plus_btn = Button::new("ov-repo-plus")
        .outline()
        .small()
        .icon(Icon::new(IconName::Plus).size_3p5())
        .dropdown_menu(move |menu, _, _| {
            let new_file = format!("{web_new}/-/new/{ref_new}");
            let new_branch = format!("{web_new}/-/branches/new");
            let new_tag = format!("{web_new}/-/tags/new");
            menu.item(PopupMenuItem::new("New file (Web)").on_click(move |_, _, _| { let _ = safe_open_url(&new_file); }))
                .item(PopupMenuItem::new("New branch (Web)").on_click(move |_, _, _| { let _ = safe_open_url(&new_branch); }))
                .item(PopupMenuItem::new("New tag (Web)").on_click(move |_, _, _| { let _ = safe_open_url(&new_tag); }))
        });

    let code_btn = Button::new("ov-code-btn")
        .primary()
        .small()
        .label("Code")
        .icon(Icon::new(IconName::ChevronDown).size_3())
        .dropdown_menu(move |mut menu, _, _| {
            if let Some(ssh) = ssh_url.clone() {
                let st = state_copy.clone();
                menu = menu.item(PopupMenuItem::new("Copy Clone with SSH").on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(ssh.clone()));
                    st.update(cx, |s, cx| s.set_status("Copied SSH clone URL", cx));
                }));
            }
            if let Some(http) = http_url.clone() {
                let st = state_copy.clone();
                menu = menu.item(PopupMenuItem::new("Copy Clone with HTTPS").on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(http.clone()));
                    st.update(cx, |s, cx| s.set_status("Copied HTTPS clone URL", cx));
                }));
            }
            let z = zip_url.clone();
            let t = tar_url.clone();
            let w = web_base.clone();
            menu.separator()
                .item(PopupMenuItem::new("Download .zip").on_click(move |_, _, _| { let _ = safe_open_url(&z); }))
                .item(PopupMenuItem::new("Download .tar.gz").on_click(move |_, _, _| { let _ = safe_open_url(&t); }))
                .separator()
                .item(PopupMenuItem::new("Open repository in Web").on_click(move |_, _, _| { let _ = safe_open_url(&w); }))
        });

    let find_state = view.state.clone();
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .pb_3()
        .child(div().flex().items_center().gap_2().min_w_0().child(ref_dropdown).child(path_crumb))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .flex_shrink_0()
                .child(plus_btn)
                .child(
                    Button::new("ov-find-file")
                        .outline()
                        .small()
                        .label("Find file")
                        .on_click(move |_, _, cx| {
                            find_state.update(cx, |s, cx| s.dispatch_command(ScreenCommand::OpenFindFile, cx));
                        }),
                )
                .child(code_btn),
        )
}
