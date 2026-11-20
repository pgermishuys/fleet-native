//! `ui/DirectoryPickerPopover.vue`: a 32rem popover with a breadcrumb bar (or a "Locations"
//! header at the roots), a search field, and a list of folders. The model here is a fixed tree of
//! invented folders; listing a real server's folders belongs to component 7 (New session).
use super::overlay::anchored;
use super::rem;
use crate::State;
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use crate::ui::text_menu::text_input;
use rune::{Align, Cursor, El, Side, TextEdit, col, icon, row, text};

pub struct Picker {
    /// Path segments below the root; empty is the roots list.
    pub at: Vec<&'static str>,
    pub search: TextEdit,
}

impl Default for Picker {
    fn default() -> Self {
        Picker { at: Vec::new(), search: TextEdit::new("") }
    }
}

const ROOTS: [&str; 2] = ["code", "notes"];

fn children(at: &[&'static str]) -> Vec<(&'static str, bool)> {
    match at {
        [] => ROOTS.iter().map(|r| (*r, false)).collect(),
        ["code"] => vec![("orchard", true), ("lighthouse", true), ("scratch", false)],
        ["notes"] => vec![("field", false), ("archive", false)],
        _ => vec![("src", false), ("docs", false)],
    }
}

pub fn view(s: &State, t: &Theme, anchor: &str) -> Vec<El> {
    let pk = &s.ui.dir;
    let crumbs: El = if pk.at.is_empty() {
        row()
            .w_full()
            .px(rem(0.5))
            .py(rem(0.5))
            .center_y()
            .justify_between()
            .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
            .child(text("Locations").font(mono(rem(0.75), 400.0, 1.3)).color(t.text))
            .child(
                super::button::icon_button(t, super::button::Variant::Ghost, super::button::Size::IconSm, &REFRESH_CW, "Refresh")
                    .key("dir-refresh"),
            )
    } else {
        let mut r = row().w_full().gap(4.0).px(rem(0.75)).py(rem(0.5)).center_y().border_sides([0.0, 0.0, 1.0, 0.0], t.border).child(
            text("Roots")
                .font(sans(rem(0.75), 400.0, 1.3))
                .color(t.muted)
                .key("dir-roots")
                .cursor(Cursor::Pointer)
                .on_press(|s: &mut State| s.ui.dir.at.clear()),
        );
        let last = pk.at.len() - 1;
        for (i, seg) in pk.at.iter().enumerate() {
            r = r.child(icon(&CHEVRON_RIGHT, rem(0.75)).color(t.muted)).child(
                text(seg.to_string())
                    .key(format!("dir-crumb{i}"))
                    .font(sans(rem(0.75), if i == last { 500.0 } else { 400.0 }, 1.3))
                    .color(if i == last { t.text } else { t.muted })
                    .cursor(Cursor::Pointer)
                    .on_press(move |s: &mut State| s.ui.dir.at.truncate(i + 1)),
            );
        }
        r
    };
    let search = row().w_full().p_search(t).child(
        text_input(&pk.search, sans(rem(0.875), 400.0, 1.3))
            .key("dir-search")
            .label("Search folders")
            .single_line()
            .placeholder("Search folders", t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w_full()
            .h(rem(2.0))
            .px(rem(0.5))
            .py(7.0)
            .on_edit(|s: &mut State| &mut s.ui.dir.search),
    );
    let needle = pk.search.text.trim().to_lowercase();
    let mut list = col().w_full().padding(rem(0.25));
    for (name, repo) in children(&pk.at).into_iter().filter(|(n, _)| n.contains(&needle)) {
        let at = pk.at.clone();
        list = list.child(
            row()
                .key(format!("dir-{name}"))
                .role("button")
                .label(name)
                .w_full()
                .px(rem(0.5))
                .py(rem(0.375))
                .gap(rem(0.5))
                .center_y()
                .radius(4.0)
                .hover(t.wash(7.0))
                .cursor(Cursor::Pointer)
                .child(icon(if repo { &FOLDER_GIT_2 } else { &FOLDER }, rem(1.0)).color(t.muted))
                .child(text(name).font(sans(rem(0.875), 400.0, 1.3)).color(t.text).grow(1.0))
                .child(icon(&CHEVRON_RIGHT, rem(0.75)).color(t.muted))
                .on_press(move |s: &mut State| {
                    let mut next = at.clone();
                    next.push(name);
                    s.ui.dir.at = next;
                    s.ui.dir.search.clear();
                }),
        );
    }
    let footer = row().w_full().justify_end().p_search(t).child(
        super::button::button(t, super::button::Variant::Default, super::button::Size::Sm, None, "Use this directory")
            .key("dir-use")
            .on_press(|s: &mut State| s.ui.open = None),
    );
    let mut p = col()
        .key("dir-popover")
        .role("dialog")
        .label("Choose a folder")
        .w(rem(32.0))
        .fill(t.card_bg)
        .border(1.0, t.border)
        .child(crumbs)
        .child(search)
        .child(list.scroll(rune::ScrollSpec { stick_bottom: false }).key("dir-list").max_h(240.0))
        .modal(true, |s: &mut State| s.ui.open = None)
        .focus_return(anchor.to_string());
    if !pk.at.is_empty() {
        p = p.child(footer);
    }
    for sh in t.menu_shadows() {
        p = p.box_shadow(sh);
    }
    vec![super::overlay::scrim("dir-scrim", |s: &mut State| s.ui.open = None), anchored(p, anchor, Side::Bottom, Align::End, 4.0)]
}

trait SearchPad {
    fn p_search(self, t: &Theme) -> Self;
}

impl SearchPad for El {
    fn p_search(self, t: &Theme) -> Self {
        let _ = t;
        self.padding(rem(0.5))
    }
}
