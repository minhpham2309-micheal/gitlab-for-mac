use gpui_kit::{
    component::{ActiveTheme as _, Icon, IconName},
    div, px, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
};

use crate::gitlab::time_util::format_long_date;
use crate::gitlab::{
    LanguageSlice, Project, ProjectDetail, ProjectOverviewCounts, RepoTreeEntry, SpecialRepoFiles,
};
use crate::nav::Screen;

use super::super::super::safe_url::safe_open_url;
use super::super::super::state::AppState;
use super::file_icons::language_color;
use super::ProjectOverviewView;

fn metric_row(
    id: &'static str,
    icon: Icon,
    label: String,
    target: Screen,
    app_state: &Entity<AppState>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement {
    let theme = cx.theme();
    let state = app_state.clone();
    div()
        .id(id)
        .py_1()
        .px_1p5()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme.list_hover))
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(theme.foreground)
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.set_screen(target, cx)))
        .child(icon.size_3p5().text_color(theme.muted_foreground))
        .child(label)
}

fn special_link(
    id: &'static str,
    label: &'static str,
    add_label: &'static str,
    entry: Option<&RepoTreeEntry>,
    add_url: String,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement {
    let theme = cx.theme();
    if let Some(file) = entry.cloned() {
        div()
            .id(id)
            .py_1()
            .px_1p5()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(theme.list_hover))
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(theme.foreground)
            .on_click(cx.listener(move |this, _, _, cx| this.open_file(&file, cx)))
            .child(Icon::new(IconName::FileText).size_3p5().text_color(theme.muted_foreground))
            .child(label)
    } else {
        div()
            .id(id)
            .py_1()
            .px_1p5()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(theme.list_hover))
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(theme.link)
            .on_click(move |_, _, _| { let _ = safe_open_url(&add_url); })
            .child(Icon::new(IconName::Plus).size_3().text_color(theme.link))
            .child(add_label)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_info_panel(
    project: &Project,
    detail: Option<&ProjectDetail>,
    languages: &[LanguageSlice],
    counts: &ProjectOverviewCounts,
    special: &SpecialRepoFiles,
    current_ref: &str,
    app_state: &Entity<AppState>,
    cx: &mut Context<ProjectOverviewView>,
) -> impl IntoElement {
    let (fg, muted, border, link, hover, secondary) = {
        let t = cx.theme();
        (t.foreground, t.muted_foreground, t.border, t.link, t.list_hover, t.secondary)
    };
    let total_pct: f32 = languages.iter().map(|l| l.percent).sum::<f32>().max(1.0);
    let lang_bar = (!languages.is_empty()).then(|| {
        let mut bar = div()
            .w_full()
            .h(px(8.))
            .rounded_full()
            .overflow_hidden()
            .bg(secondary)
            .flex()
            .flex_row();
        for slice in languages {
            let weight = (slice.percent / total_pct).max(0.015);
            bar = bar.child(div().h_full().flex_grow(weight).flex_basis(px(0.)).bg(language_color(&slice.name)));
        }
        let legend = div()
            .flex()
            .flex_wrap()
            .gap_x_2p5()
            .gap_y_1()
            .children(languages.iter().take(4).map(|s| {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .text_color(muted)
                    .child(div().size(px(7.)).rounded_full().bg(language_color(&s.name)))
                    .child(SharedString::from(format!("{} {:.1}%", s.name, s.percent)))
            }));
        div().flex().flex_col().gap_1p5().pb_3().child(bar).child(legend)
    });

    let commits_n = detail.and_then(|d| d.statistics.as_ref()).map(|s| s.commit_count);
    let commits_lbl = commits_n.map(|n| format!("{n} Commits")).unwrap_or_else(|| "Commits".into());
    let branches_lbl = counts.branches.map(|n| format!("{n} Branches")).unwrap_or_else(|| "Branches".into());
    let tags_lbl = counts.tags.map(|n| format!("{n} Tags")).unwrap_or_else(|| "Tags".into());
    let rel_lbl = counts.releases.map(|n| format!("{n} Releases")).unwrap_or_else(|| "Releases".into());

    let web = project.web_url.trim_end_matches('/');
    let r = urlencoding::encode(current_ref);
    let created = detail
        .and_then(|d| d.created_at.as_deref())
        .map(format_long_date);
    let auto_devops = detail.and_then(|d| d.auto_devops_enabled).unwrap_or(false);
    let show_wiki = detail.map(|d| d.wiki_enabled).unwrap_or(true);
    let wiki_url = format!("{web}/-/wikis/home");
    let commits_state = app_state.clone();
    let commits_ref = current_ref.to_string();

    div()
        .w(px(250.))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().text_color(fg).child("Project information"))
        .children(lang_bar)
        .child(
            div().flex().flex_col().gap_0p5().pb_2p5().border_b_1().border_color(border.opacity(0.6))
                .child(
                    div().id("info-commits").py_1().px_1p5().rounded_md().cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .flex().items_center().gap_2().text_xs().text_color(fg)
                        .on_click(move |_, _, cx| commits_state.update(cx, |s, cx| s.open_commit_history(Some(commits_ref.clone()), None, cx)))
                        .child(Icon::new(IconName::FileText).size_3p5().text_color(muted))
                        .child(commits_lbl),
                )
                .child(metric_row("info-branches", Icon::empty().path("icons/glab-branch.svg"), branches_lbl, Screen::Branches, app_state, cx))
                .child(metric_row("info-tags", Icon::empty().path("icons/glab-tag.svg"), tags_lbl, Screen::Tags, app_state, cx))
                .child(metric_row("info-releases", Icon::empty().path("icons/glab-rocket.svg"), rel_lbl, Screen::Releases, app_state, cx)),
        )
        .child(
            div().flex().flex_col().gap_0p5().pb_2p5().border_b_1().border_color(border.opacity(0.6))
                .child(special_link("info-readme", "README", "Add README", special.readme.as_ref(), format!("{web}/-/new/{r}?file_name=README.md"), cx))
                .child(special_link("info-ci", "CI/CD configuration", "Set up CI/CD", special.ci_config.as_ref(), format!("{web}/-/ci/editor"), cx))
                .child(special_link("info-license", "LICENSE", "Add LICENSE", special.license.as_ref(), format!("{web}/-/new/{r}?file_name=LICENSE"), cx))
                .child(special_link("info-changelog", "CHANGELOG", "Add CHANGELOG", special.changelog.as_ref(), format!("{web}/-/new/{r}?file_name=CHANGELOG.md"), cx))
                .child(special_link("info-contrib", "CONTRIBUTING", "Add CONTRIBUTING", special.contributing.as_ref(), format!("{web}/-/new/{r}?file_name=CONTRIBUTING.md"), cx))
                .children(auto_devops.then(|| {
                    div().px_1p5().py_1().text_xs().text_color(muted).child("Auto DevOps enabled")
                }))
                .children(show_wiki.then(|| {
                    div().id("info-wiki").py_1().px_1p5().rounded_md().cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .flex().items_center().gap_2().text_xs().text_color(link)
                        .on_click(move |_, _, _| { let _ = safe_open_url(&wiki_url); })
                        .child(Icon::new(IconName::BookOpen).size_3p5().text_color(link))
                        .child("Wiki")
                })),
        )
        .children(created.map(|date| {
            div().flex().flex_col().gap_0p5().pt_1()
                .child(div().text_xs().text_color(fg).child("Created on"))
                .child(div().text_xs().text_color(muted).child(date))
        }))
}
