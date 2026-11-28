use std::sync::Arc;

use gpui_kit::{
    base::StyledExt as _,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputState},
        popover::{Popover, PopoverState},
        scroll::{Scrollbar, ScrollbarMode},
        ActiveTheme as _, Icon, IconName, Sizable as _,
    },
    div, point, px, rgb, Anchor, AnyElement, App, AppContext as _, ClipboardItem, Entity,
    Focusable as _, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window,
};

use crate::gitlab::{Branch, RepoTag};
use crate::text_util::ellipsis;

pub struct RefSwitcherState {
    pub search_input: Entity<InputState>,
    pub scroll_handle: ScrollHandle,
}

impl RefSwitcherState {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search by Git revision"));
        Self { search_input, scroll_handle: ScrollHandle::new() }
    }

    pub fn reset(&self, window: &mut Window, cx: &mut App) {
        self.search_input.update(cx, |inp, cx| inp.set_value("", window, cx));
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
    }
}

pub type OnSelectRef = Arc<dyn Fn(String, &mut Window, &mut App) + 'static>;
pub type OnCopyRef = Arc<dyn Fn(String, &mut App) + 'static>;

pub struct RefSwitcherProps<'a> {
    pub id_prefix: &'static str,
    pub trigger_id: &'static str,
    pub current_ref: &'a str,
    pub default_branch: Option<&'a str>,
    pub branches: &'a [Branch],
    pub tags: &'a [RepoTag],
    pub state: &'a RefSwitcherState,
    pub on_select: OnSelectRef,
    pub on_copy: Option<OnCopyRef>,
}

#[derive(Clone)]
struct RefItem {
    name: String,
    is_default: bool,
    is_protected: bool,
}

#[derive(Clone, Copy)]
struct RefPalette {
    fg: Hsla,
    muted: Hsla,
    border: Hsla,
    bg: Hsla,
    input_b: Hsla,
    hover_bg: Hsla,
    active_bg: Hsla,
    def_bg: Hsla,
    def_fg: Hsla,
    prot_bg: Hsla,
    prot_fg: Hsla,
    cnt_bg: Hsla,
    cnt_fg: Hsla,
}

impl RefPalette {
    fn from_app(cx: &App) -> Self {
        let t = cx.theme();
        let dark = t.is_dark();
        Self {
            fg: t.foreground, muted: t.muted_foreground, border: t.border, bg: t.background,
            input_b: t.input, hover_bg: t.list_hover, active_bg: t.list_active,
            def_bg: if dark { rgb(0x63A6E9).into() } else { rgb(0xCBE2F9).into() },
            def_fg: if dark { rgb(0x0B192C).into() } else { rgb(0x0B5CAD).into() },
            prot_bg: if dark { rgb(0x89888D).into() } else { rgb(0xE4E4E7).into() },
            prot_fg: if dark { rgb(0x18171C).into() } else { rgb(0x3F3F46).into() },
            cnt_bg: if dark { rgb(0x525158).into() } else { rgb(0xE4E4E7).into() },
            cnt_fg: if dark { rgb(0xECECEF).into() } else { rgb(0x3F3F46).into() },
        }
    }
}

fn ref_badge(label: &'static str, bg: Hsla, fg: Hsla) -> impl IntoElement {
    div().px_1p5().py(px(1.)).rounded_full().bg(bg).text_size(px(11.)).font_medium().text_color(fg).flex_shrink_0().child(label)
}

fn section_header(title: &'static str, count: usize, p: RefPalette) -> impl IntoElement {
    div().px_3().pb_1p5().flex().items_center().gap_1p5()
        .child(div().text_xs().font_semibold().text_color(p.fg).child(title))
        .child(div().px_1p5().py(px(0.5)).rounded_full().bg(p.cnt_bg).text_size(px(10.5)).font_medium().text_color(p.cnt_fg).child(count.to_string()))
}

fn build_rows(
    id_prefix: &'static str,
    kind: &'static str,
    items: Vec<RefItem>,
    p: RefPalette,
    pop: Entity<PopoverState>,
    on_select: OnSelectRef,
) -> Vec<AnyElement> {
    items.into_iter().enumerate().map(|(ix, item)| {
        let (name, sel_cb, pop_item) = (item.name.clone(), on_select.clone(), pop.clone());
        div().id(SharedString::from(format!("{id_prefix}-{kind}-{ix}")))
            .mx_1p5().px_2p5().py_1p5().rounded_md().cursor_pointer().hover(move |s| s.bg(p.hover_bg))
            .flex().items_center().gap_1p5()
            .on_click(move |_, window, cx| {
                pop_item.update(cx, |st, cx| st.dismiss(window, cx));
                sel_cb(name.clone(), window, cx);
            })
            .child(div().text_xs().text_color(p.fg).truncate().child(item.name))
            .children(item.is_default.then(|| ref_badge("default", p.def_bg, p.def_fg)))
            .children(item.is_protected.then(|| ref_badge("protected", p.prot_bg, p.prot_fg)))
            .test_support().into_any_element()
    }).collect()
}

pub fn render_ref_switcher(props: RefSwitcherProps<'_>, cx: &App) -> impl IntoElement + use<> {
    let mono = cx.theme().mono_font_family.clone();
    let trigger = Button::new(props.trigger_id).outline().small().font_family(mono)
        .icon(Icon::empty().path("icons/glab-branch.svg").size_3p5())
        .label(format!("{} ▾", ellipsis(props.current_ref, 22)));

    let (id_prefix, current_ref) = (props.id_prefix, props.current_ref.to_string());
    let default_branch = props.default_branch.map(str::to_string);
    let branches: Vec<RefItem> = props.branches.iter().map(|b| RefItem {
        name: b.name.clone(),
        is_default: b.default || default_branch.as_deref() == Some(b.name.as_str()),
        is_protected: b.protected,
    }).collect();
    let tags: Vec<RefItem> = props.tags.iter()
        .map(|t| RefItem { name: t.name.clone(), is_default: false, is_protected: t.protected })
        .collect();
    let (search_input, scroll_handle) = (props.state.search_input.clone(), props.state.scroll_handle.clone());
    let (search_focus, on_select, on_copy) = (search_input.focus_handle(cx), props.on_select, props.on_copy);
    let (reset_input, reset_scroll) = (search_input.clone(), scroll_handle.clone());

    Popover::new(SharedString::from(format!("{id_prefix}-popover")))
        .anchor(Anchor::TopLeft).arrow(true).p_0().w(px(380.)).track_focus(&search_focus).trigger(trigger)
        .on_open_change(move |open, window, cx| {
            if !*open {
                reset_input.update(cx, |inp, cx| inp.set_value("", window, cx));
                reset_scroll.set_offset(point(px(0.), px(0.)));
            }
        })
        .content(move |_: &mut PopoverState, _window, cx| {
            let pop = cx.entity();
            let q = search_input.read(cx).value().trim().to_ascii_lowercase();
            let p = RefPalette::from_app(cx);
            let show_sel = !current_ref.is_empty() && (q.is_empty() || current_ref.to_ascii_lowercase().contains(&q));
            let sel_def = branches.iter().find(|b| b.name == current_ref).map(|b| b.is_default)
                .unwrap_or_else(|| default_branch.as_deref() == Some(current_ref.as_str()));
            let sel_prot = branches.iter().find(|b| b.name == current_ref).map(|b| b.is_protected)
                .or_else(|| tags.iter().find(|t| t.name == current_ref).map(|t| t.is_protected)).unwrap_or(false);

            let filt_b: Vec<RefItem> = branches.iter().filter(|b| !(show_sel && b.name == current_ref) && (q.is_empty() || b.name.to_ascii_lowercase().contains(&q))).cloned().collect();
            let filt_t: Vec<RefItem> = tags.iter().filter(|t| !(show_sel && t.name == current_ref) && (q.is_empty() || t.name.to_ascii_lowercase().contains(&q))).cloned().collect();

            let mut sections: Vec<AnyElement> = Vec::new();
            if show_sel {
                let (copy_ref, copy_cb, pop_sel) = (current_ref.clone(), on_copy.clone(), pop.clone());
                sections.push(
                    div().w_full().flex().flex_col()
                        .child(div().px_3().pt_1().pb_1p5().flex().items_center().justify_between()
                            .child(div().text_xs().font_semibold().text_color(p.fg).child("Selected"))
                            .child(Button::new(SharedString::from(format!("{id_prefix}-copy"))).ghost().xsmall().icon(Icon::new(IconName::Copy).size_3p5())
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copy_ref.clone()));
                                    if let Some(cb) = copy_cb.as_ref() { cb(copy_ref.clone(), cx); }
                                })))
                        .child(div().id(SharedString::from(format!("{id_prefix}-selected-row")))
                            .mx_1p5().px_2p5().py_1p5().rounded_md().bg(p.active_bg).cursor_pointer().flex().items_center().gap_2()
                            .on_click(move |_, window, cx| { pop_sel.update(cx, |st, cx| st.dismiss(window, cx)); })
                            .child(Icon::new(IconName::Check).size_3p5().text_color(p.fg).flex_shrink_0())
                            .child(div().text_xs().text_color(p.fg).truncate().child(current_ref.clone()))
                            .children(sel_def.then(|| ref_badge("default", p.def_bg, p.def_fg)))
                            .children(sel_prot.then(|| ref_badge("protected", p.prot_bg, p.prot_fg)))
                            .test_support())
                        .into_any_element(),
                );
            }
            for (title, kind, list) in [("Branches", "branch", filt_b), ("Tags", "tag", filt_t)] {
                if list.is_empty() { continue; }
                let mut wrap = div().w_full().flex().flex_col();
                if !sections.is_empty() { wrap = wrap.mt_2().pt_2().border_t_1().border_color(p.border); }
                sections.push(wrap.child(section_header(title, list.len(), p)).children(build_rows(id_prefix, kind, list, p, pop.clone(), on_select.clone())).into_any_element());
            }
            if sections.is_empty() {
                sections.push(div().px_3().py_6().flex().items_center().justify_center().text_xs().text_color(p.muted).child("No matching Git revisions").into_any_element());
            }

            let list_scroll = div().id(SharedString::from(format!("{id_prefix}-scroll")))
                .w_full().max_h(px(280.)).overflow_y_scroll().track_scroll(&scroll_handle)
                .pr_3().py_1p5().flex().flex_col().children(sections).test_support();

            div().id(SharedString::from(format!("{id_prefix}-panel"))).w_full().flex().flex_col()
                .child(div().px_3p5().py_2p5().border_b_1().border_color(p.border).text_sm().font_semibold().text_color(p.fg).child("Select Git revision"))
                .child(div().px_2p5().py_2().border_b_1().border_color(p.border).child(
                    div().h(px(32.)).px_2().rounded_md().bg(p.bg).border_1().border_color(p.input_b).flex().items_center().gap_1p5()
                        .child(Icon::new(IconName::Search).size_3p5().text_color(p.muted))
                        .child(div().flex_1().min_w_0().child(Input::new(&search_input).appearance(false).cleanable(true).small())),
                ))
                .child(div().relative().w_full().child(list_scroll).child(
                    div().absolute().inset_0().child(
                        Scrollbar::vertical(&scroll_handle)
                            .id(SharedString::from(format!("{id_prefix}-scrollbar")))
                            .mode(ScrollbarMode::Always)
                            .viewport_from_layout(),
                    ),
                ))
                .test_support()
        })
}
