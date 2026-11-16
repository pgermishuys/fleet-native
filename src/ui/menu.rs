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
    /// The submenu is as wide as its rows (min 8rem), not the context menu's fixed width (`ContextMenuSubContent`'s default).
    pub sub_fit: bool,
    /// A 14px spinner where the icon goes ("Detecting tools…").
    pub spinner: bool,
    /// The label in the mono face, 12.25px (a key file's path).
    pub mono: bool,
    pub run: Option<Action>,
    /// The label ends in a machine's name, drawn in mono 11.5 px / 600 (`machine-header__menu-name`): where it starts.
    pub name_at: Option<usize>,
    /// The row's own key (a test finds it by this); without one it is `<menu>-item<n>`.
    pub id: Option<String>,
    /// `ContextMenuHint`: what the footer says while the pointer is on this row.
    pub hint: Option<String>,
    /// A second line under the label: why a disabled row is (Fork's reason), 11.5px in the muted colour.
    pub note: Option<String>,
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
            sub_fit: false,
            spinner: false,
            mono: false,
            run: None,
            name_at: None,
            id: None,
            hint: None,
            note: None,
        }
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }
    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.note = Some(n.into());
        self
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
    /// `submenu`, sized to its rows.
    pub fn submenu_fit(mut self, entries: Vec<Entry>) -> Self {
        self.sub = Some(entries);
        self.sub_fit = true;
        self
    }
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    pub fn spinner(mut self) -> Self {
        self.spinner = true;
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
    col()
        .items_stretch()
        .px(5.0)
        .py(2.0)
        .radius(RADIUS_BTN - 3.0)
        .fill(t.wash(6.0))
        .border(1.0, t.border)
        .child(text(label.to_string()).font(sans(10.0, 500.0, 1.0)).color(t.muted))
}

fn row_el(t: &Theme, key: String, it: Item, closes: bool, chevron: bool, top: bool, fit: bool) -> El {
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
    if it.spinner {
        e = e.child(super::confirm::spinner(glyph_ink));
    } else if let Some(g) = it.icon {
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
        None => match &it.note {
            Some(note) => e.child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .child(text(it.label).font(sans(13.0, 400.0, 1.3)).color(ink))
                    .child(text(note.clone()).font(sans(11.5, 400.0, 1.3)).color(t.muted).wrap().w_full()),
            ),
            None => {
                let font = if it.mono { mono(rem(0.75), 400.0, 1.3) } else { sans(13.0, 400.0, 1.3) };
                let label = text(it.label).font(font).color(ink).grow(1.0);
                // A panel sized to its rows must not cut them short.
                e.child(if fit { label } else { label.min_w(0.0).truncate() })
            }
        },
    };
    if let Some(s) = it.shortcut {
        e = e.child(shortcut_chip(t, &s).ml(rem(0.5)));
    }
    if chevron {
        e = e.child(icon(&crate::icons::CHEVRON_RIGHT, rem(0.875)).color(t.muted));
    }
    // Rows of a submenu leave the footer alone; a trigger keeps its hint while its submenu is open.
    if let (Some(h), true, false) = (it.hint.clone(), top, it.disabled) {
        e = e.on_hover_change(move |s: &mut State, inside: bool, _: &mut Tx| {
            if inside {
                s.ui.hint = Some(h.clone());
            } else if !chevron && s.ui.hint.as_deref() == Some(h.as_str()) {
                s.ui.hint = None;
            }
        });
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

/// A width for a panel sized to its rows (Rune's shrink-to-fit leaves the widest row touching the edge): the longest label at
/// a generous per-letter width, with the icon, the chevron and the paddings.
pub fn fit_width(entries: &[Entry]) -> f32 {
    let widest = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Item(it) => {
                let letters = it.label.chars().count() as f32 * if it.mono { 7.4 } else { 6.9 };
                let lead = if it.icon.is_some() || it.spinner { 14.0 + rem(0.5) } else { 0.0 };
                let tail = if it.sub.is_some() { 14.0 + rem(0.5) } else { 0.0 };
                Some(letters + lead + tail + 2.0 * rem(0.5) + 10.0)
            }
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    widest.max(rem(8.0))
}

/// The panel: `rounded-card border bg-card-bg p-[5px] shadow-(--menu-shadow)`, min width 8rem.
pub fn panel(t: &Theme, key: &str, entries: Vec<Entry>) -> El {
    panel_capped(t, key, entries, None)
}

/// `panel`, its rows scrolling inside `max_h` (the panel's padding included).
pub fn panel_capped(t: &Theme, key: &str, entries: Vec<Entry>, max_h: Option<f32>) -> El {
    panel_full(t, key, entries, max_h, None, true).0
}

/// A submenu a row opens: the key of its trigger row, and its rows.
pub type Sub = (String, Vec<Entry>, bool);

/// The key of the row `i` of menu `key` (`Item::id`, else `<menu>-item<i>`).
fn item_key(key: &str, i: usize, it: &Item) -> String {
    it.id.clone().unwrap_or_else(|| format!("{key}-item{i}"))
}

/// `panel_capped` with the hint footer (`footer` is what it says now) and the submenus its rows open. `top` is false for a
/// submenu's own rows, which do not touch the footer.
pub fn panel_full(t: &Theme, key: &str, entries: Vec<Entry>, max_h: Option<f32>, footer: Option<String>, top: bool) -> (El, Vec<Sub>) {
    panel_level(t, key, entries, max_h, footer, top, (0, false))
}

/// `panel_full` for the menu at nesting `level` (with whether its rows fit their panel): 0 is the menu itself, 1 its submenu. A row's submenu opens by setting
/// `Ui::sub` (level 0) or `Ui::sub2` (level 1).
fn panel_level(
    t: &Theme,
    key: &str,
    entries: Vec<Entry>,
    max_h: Option<f32>,
    footer: Option<String>,
    top: bool,
    (level, fit_rows): (u8, bool),
) -> (El, Vec<Sub>) {
    let mut subs = Vec::new();
    let mut list = col().key("rows").items_stretch();
    for (i, entry) in entries.into_iter().enumerate() {
        list = list.child(match entry {
            Entry::Item(mut it) => {
                let k = item_key(key, i, &it);
                let sub = it.sub.take();
                let chevron = sub.is_some();
                if let Some(rows) = sub {
                    let open = k.clone();
                    let fit = it.sub_fit;
                    it.run = Some(Rc::new(move |s: &mut State, _: &mut Tx| {
                        let slot = if level == 0 { &mut s.ui.sub } else { &mut s.ui.sub2 };
                        *slot = if slot.as_deref() == Some(open.as_str()) { None } else { Some(open.clone()) };
                        if level == 0 {
                            s.ui.sub2 = None;
                        }
                    }));
                    subs.push((k.clone(), rows, fit));
                }
                row_el(t, k, it, !chevron, chevron, top, fit_rows)
            }
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
    if let Some(text_) = footer {
        // `ContextMenuHint`: it runs edge to edge under the rows, over the panel's padding.
        p = p.child(
            row()
                .key(format!("{key}-hint"))
                .mx(-5.0)
                .mt(4.0)
                .mb(-5.0)
                .h(50.0)
                .px(13.0)
                .pt(8.0)
                .pb(9.0)
                .fill(t.wash(2.5))
                .clip_corners([0.0, 0.0, RADIUS_CARD - 1.0, RADIUS_CARD - 1.0])
                .child(el().h(1.0).w_full().fill(t.border).absolute(0.0, 0.0))
                .child(text(text_).font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap_anywhere().w_full()),
        );
    }
    // A scrolling menu is Fleet's selector box: 380 wide (row 3.3), not as wide as its widest row.
    if max_h.is_some() {
        p = p.w(380.0);
    }
    for s in t.menu_shadows() {
        p = p.box_shadow(s);
    }
    (p, subs)
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
    context_hint(t, key, at, entries, width, None, None)
}

/// `context_w` with the hint footer (`footer`, else nothing) and the submenu `open` names (`State::ui.sub`).
pub fn context_hint(
    t: &Theme,
    key: &str,
    at: (f32, f32),
    entries: Vec<Entry>,
    width: Option<f32>,
    footer: Option<String>,
    open: Option<&str>,
) -> Vec<El> {
    context_nested(t, key, at, entries, width, footer, (open, None))
}

/// `context_hint` with a second level: `open` is the submenu's row and the submenu's own open row.
pub fn context_nested(
    t: &Theme,
    key: &str,
    at: (f32, f32),
    entries: Vec<Entry>,
    width: Option<f32>,
    footer: Option<String>,
    open: (Option<&str>, Option<&str>),
) -> Vec<El> {
    let point = format!("{key}-point");
    let (mut p, subs) = panel_full(t, key, entries, None, footer, true);
    if let Some(w) = width {
        p = p.w(w);
    }
    let p = p.modal(true, |s: &mut State| s.ui.open = None);
    let mut out = vec![
        scrim(&format!("{key}-scrim"), |s: &mut State| s.ui.open = None),
        el().key(point.clone()).absolute(at.0, at.1).size(0.0, 0.0),
        anchored(p, &point, Side::Bottom, Align::Start, 0.0),
    ];
    // `ContextMenuSubContent`: beside its trigger; Escape closes it before the menu.
    for (trigger, rows, fit) in subs.into_iter().filter(|(k, ..)| Some(k.as_str()) == open.0) {
        let sub_key = format!("{trigger}-sub");
        let width = fit_width(&rows);
        let (panel, inner) = panel_level(t, &sub_key, rows, None, None, false, (1, fit));
        let panel = if fit { panel.w(width) } else { panel.w(rem(13.0)) };
        let panel = panel.modal(true, |s: &mut State| s.ui.sub = None);
        out.push(anchored(panel, &trigger, Side::Right, Align::Start, 4.0));
        for (inner_trigger, inner_rows, _) in inner.into_iter().filter(|(k, ..)| Some(k.as_str()) == open.1) {
            let width = fit_width(&inner_rows);
            let (panel, _) = panel_level(t, &format!("{inner_trigger}-sub"), inner_rows, None, None, false, (2, true));
            let panel = panel.w(width);
            let panel = panel.modal(true, |s: &mut State| s.ui.sub2 = None);
            out.push(anchored(panel, &inner_trigger, Side::Right, Align::Start, 4.0));
        }
    }
    out
}
