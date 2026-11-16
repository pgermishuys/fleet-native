//! `menu-classes.ts`, `dropdown-menu/` and `context-menu/`: one look for every menu. Keyboard
//! (Up/Down/Home/End, typeahead, Enter/Space, Escape, Tab) comes from Rune's focus group and
//! modal; the look is Fleet's.
use super::overlay::{anchored, scrim};
use super::rem;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::{State, Tx};
use rune::{Align, Cursor, El, Icon, Side, col, el, icon, row, text};
use std::rc::Rc;

pub type Action = Rc<dyn Fn(&mut State, &mut Tx)>;

pub enum Entry {
    Item(Item),
    Separator,
    /// `menuLabelClass`: 11px, semibold, uppercase.
    Label(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    None,
    /// A check in front (`DropdownMenuCheckboxItem`).
    Check(bool),
    /// A dot in front (`DropdownMenuRadioItem`).
    Radio(bool),
}

pub struct Item {
    pub label: String,
    pub icon: Option<&'static Icon>,
    pub shortcut: Option<String>,
    pub destructive: bool,
    pub disabled: bool,
    pub mark: Mark,
    pub sub: Option<Vec<Entry>>,
    pub run: Option<Action>,
    /// The label ends in a machine's name, drawn in mono 11.5 px / 600 (`machine-header__menu-name`): where it starts.
    pub name_at: Option<usize>,
}

impl Item {
    pub fn new(label: impl Into<String>) -> Item {
        Item {
            label: label.into(),
            icon: None,
            shortcut: None,
            destructive: false,
            disabled: false,
            mark: Mark::None,
            sub: None,
            run: None,
            name_at: None,
        }
    }
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn mark(mut self, m: Mark) -> Self {
        self.mark = m;
        self
    }
    pub fn submenu(mut self, entries: Vec<Entry>) -> Self {
        self.sub = Some(entries);
        self
    }
    /// "`before` `name`" with the name in the machine-name type; one line, a long name cut short.
    pub fn with_name(mut self, before: &str, name: &str) -> Self {
        self.label = format!("{before} {name}");
        self.name_at = Some(before.len() + 1);
        self
    }
    pub fn on(mut self, f: impl Fn(&mut State, &mut Tx) + 'static) -> Self {
        self.run = Some(Rc::new(f));
        self
    }
}

pub fn item(label: impl Into<String>) -> Entry {
    Entry::Item(Item::new(label))
}

/// A key chip, as in the status bar (`menuShortcutClass`).
pub fn shortcut_chip(t: &Theme, label: &str) -> El {
    row()
        .px(5.0)
        .py(2.0)
        .radius(RADIUS_BTN - 3.0)
        .fill(t.wash(6.0))
        .border(1.0, t.border)
        .child(text(label.to_string()).font(sans(10.0, 500.0, 1.0)).color(t.muted))
}

fn row_el(t: &Theme, key: String, it: Item, closes: bool) -> El {
    let ink = if it.destructive { t.destructive } else { t.text };
    let glyph_ink = if it.destructive { t.destructive } else { t.muted };
    let highlight = if it.destructive { t.destructive.alpha(0.12) } else { t.wash(7.0) };
    let mut e = row()
        .key(key)
        .role("menuitem")
        .label(it.label.clone())
        .min_h(30.0)
        .px(rem(0.5))
        .py(5.0)
        .gap(rem(0.5))
        .center_y()
        .radius(RADIUS_BTN - 2.0)
        .cursor(Cursor::Default)
        .hover_group();
    e = e.focusable(!it.disabled).focus_selected(matches!(it.mark, Mark::Check(true) | Mark::Radio(true)));
    if !it.disabled {
        e = e.focus_highlight(highlight).hover(highlight);
    } else {
        e = e.opacity(0.45);
    }
    // `data-[inset]:pl-8` and the check / dot rows keep 28px in front for the indicator.
    match it.mark {
        Mark::None => {}
        Mark::Check(on) | Mark::Radio(on) => {
            let g: &'static Icon = if matches!(it.mark, Mark::Check(_)) { &crate::icons::CHECK } else { &crate::icons::CIRCLE };
            let dot = el().absolute(rem(0.5), 0.0).h_full().w(rem(0.875)).center().child(
                icon(g, if matches!(it.mark, Mark::Check(_)) { rem(0.875) } else { 6.0 }).color(t.text).opacity(if on { 1.0 } else { 0.0 }),
            );
            e = e.pl(rem(2.0)).child(dot);
        }
    }
    if let Some(g) = it.icon {
        e = e.child(icon(g, rem(0.875)).color(glyph_ink).group_color(if it.destructive { t.destructive } else { t.text }));
    }
    e = match it.name_at {
        Some(at) => e.child(
            row()
                .grow(1.0)
                .min_w(0.0)
                .gap(rem(0.25))
                .center_y()
                .child(text(it.label[..at - 1].to_string()).font(sans(13.0, 400.0, 1.3)).color(ink))
                .child(text(it.label[at..].to_string()).font(mono(11.5, 600.0, 1.3)).color(ink).truncate().min_w(0.0)),
        ),
        None => e.child(text(it.label).font(sans(13.0, 400.0, 1.3)).color(ink).grow(1.0)),
    };
    if let Some(s) = it.shortcut {
        e = e.child(shortcut_chip(t, &s).ml(rem(0.5)));
    }
    if it.sub.is_some() {
        e = e.child(icon(&crate::icons::CHEVRON_RIGHT, rem(0.875)).color(t.muted));
    }
    if let (Some(run), false) = (it.run, it.disabled) {
        e = e.on_press_tx(move |s: &mut State, tx: &mut Tx| {
            if closes {
                s.ui.open = None;
            }
            run(s, tx);
        });
    }
    e
}

/// The panel: `rounded-card border bg-card-bg p-[5px] shadow-(--menu-shadow)`, min width 8rem.
pub fn panel(t: &Theme, key: &str, entries: Vec<Entry>) -> El {
    panel_capped(t, key, entries, None)
}

/// `panel`, its rows scrolling inside `max_h` (the panel's padding included).
pub fn panel_capped(t: &Theme, key: &str, entries: Vec<Entry>, max_h: Option<f32>) -> El {
    let mut list = col().key("rows").items_stretch();
    for (i, entry) in entries.into_iter().enumerate() {
        list = list.child(match entry {
            Entry::Item(it) => row_el(t, format!("{key}-item{i}"), it, true),
            Entry::Separator => el().key(format!("{key}-sep{i}")).h(1.0).mx(2.0).my(4.0).fill(t.border),
            Entry::Label(l) => {
                row().px(rem(0.5)).pt(7.0).pb(3.0).child(text(l.to_uppercase()).font(sans(11.0, 600.0, 1.2).tracking(0.55)).color(t.muted))
            }
        });
    }
    if let Some(h) = max_h {
        list = list.w_full().scroll(rune::ScrollSpec { stick_bottom: false }).key(format!("{key}-rows")).max_h(h - 10.0);
    }
    let mut p = col()
        .key(format!("{key}-menu"))
        .role("menu")
        .focus_group()
        .min_w(rem(8.0))
        .padding(5.0)
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .items_stretch()
        .child(list);
    // A scrolling menu is Fleet's selector box: 380 wide (row 3.3), not as wide as its widest row.
    if max_h.is_some() {
        p = p.w(380.0);
    }
    for s in t.menu_shadows() {
        p = p.box_shadow(s);
    }
    p
}

/// A dropdown anchored under (or over) the element keyed `anchor`: the scrim, the modal that
/// closes it on Escape, and the panel. `side` is reka's `side`, `align` its `align`.
pub fn dropdown(t: &Theme, key: &str, anchor: &str, entries: Vec<Entry>, side: Side, align: Align) -> Vec<El> {
    dropdown_capped(t, key, anchor, entries, side, align, None)
}

/// A dropdown whose rows scroll once they pass `max_h` (the composer's pickers hold as many rows as the harness has models).
pub fn dropdown_capped(t: &Theme, key: &str, anchor: &str, entries: Vec<Entry>, side: Side, align: Align, max_h: Option<f32>) -> Vec<El> {
    let p = panel_capped(t, key, entries, max_h).focus_return(anchor.to_string()).modal(true, |s: &mut State| s.ui.open = None);
    vec![scrim(&format!("{key}-scrim"), |s: &mut State| s.ui.open = None), anchored(p, anchor, side, align, 4.0)]
}

/// `dropdown`, `width` wide (`w-56` on the header's ⋯ menu).
pub fn dropdown_w(t: &Theme, key: &str, anchor: &str, entries: Vec<Entry>, side: Side, align: Align, width: f32) -> Vec<El> {
    let p = panel(t, key, entries).w(width).focus_return(anchor.to_string()).modal(true, |s: &mut State| s.ui.open = None);
    vec![scrim(&format!("{key}-scrim"), |s: &mut State| s.ui.open = None), anchored(p, anchor, side, align, 4.0)]
}

/// A right-click menu at a window point (`context-menu/`).
pub fn context(t: &Theme, key: &str, at: (f32, f32), entries: Vec<Entry>) -> Vec<El> {
    context_w(t, key, at, entries, None)
}

/// `context`, `width` wide (`w-56` on the project menu).
pub fn context_w(t: &Theme, key: &str, at: (f32, f32), entries: Vec<Entry>, width: Option<f32>) -> Vec<El> {
    let point = format!("{key}-point");
    let mut p = panel(t, key, entries);
    if let Some(w) = width {
        p = p.w(w);
    }
    let p = p.modal(true, |s: &mut State| s.ui.open = None);
    vec![
        scrim(&format!("{key}-scrim"), |s: &mut State| s.ui.open = None),
        el().key(point.clone()).absolute(at.0, at.1).size(0.0, 0.0),
        anchored(p, &point, Side::Bottom, Align::Start, 0.0),
    ]
}
