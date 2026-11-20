//! The right-click menu on text (`desktop/src/menu.ts` `showContextMenu`): on a link, Open Link in Browser and Copy Link; in a
//! field, Cut, Copy, Paste, then Select All; on selected text, Copy. Nothing to offer, no menu, and no trailing separator.
//!
//! macOS and Windows draw the field's and the selection's menu themselves (Rune's `popup_menu`), so those two are drawn here only
//! where Rune has none (Linux, `own_menus`). A link's menu is drawn here everywhere: Rune has none for links.
//! Spelling suggestions are out of reach (no spell checker): 24.109.
use super::menu::{Entry, Item, context_w};
use super::{Open, rem};
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{El, Key, KeyEvent, Mods, PointerPress, TextEdit};
use std::cell::Cell;

thread_local! {
    /// Whether this app draws the field and selection menus: where the platform has no menu of its own. A test turns it on.
    static OWN: Cell<bool> = const { Cell::new(!cfg!(any(target_os = "macos", target_os = "windows"))) };
}

/// The app draws the field and selection menus.
pub fn own_menus() -> bool {
    OWN.with(Cell::get)
}

/// For tests, which run the same on every system.
#[cfg(test)]
pub fn set_own_menus(on: bool) {
    OWN.with(|c| c.set(on));
}

/// What was under the pointer, as `ContextMenuParams` tells it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Params {
    pub link: Option<String>,
    pub editable: bool,
    pub can_cut: bool,
    pub can_copy: bool,
    pub can_paste: bool,
    pub selection: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    OpenLink,
    CopyLink,
    Cut(bool),
    Copy(bool),
    Paste(bool),
    SelectAll,
    Separator,
}

/// `/^(https?|mailto):/i`: the links that get a menu (and the ones `open_url` lets through).
pub fn openable(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// The menu's rows, in Electron's order.
pub fn rows(p: &Params) -> Vec<Row> {
    let mut out = Vec::new();
    if p.link.as_deref().is_some_and(openable) {
        out.extend([Row::OpenLink, Row::CopyLink, Row::Separator]);
    }
    if p.editable {
        out.extend([Row::Cut(p.can_cut), Row::Copy(p.can_copy), Row::Paste(p.can_paste), Row::Separator, Row::SelectAll]);
    } else if !p.selection.trim().is_empty() {
        out.push(Row::Copy(true));
    }
    while out.last() == Some(&Row::Separator) {
        out.pop();
    }
    out
}

/// The menu on screen: where it opened, what it was over, and the field it acts on.
#[derive(Clone, Debug, Default)]
pub struct TextMenu {
    pub params: Params,
    /// The key of the field a Cut, Paste or Select All goes to.
    pub field: Option<String>,
}

fn press(ev_key: char) -> KeyEvent {
    KeyEvent { key: Key::Command(ev_key), mods: Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() } }
}

/// The menu's entries; each pick closes the menu and does its part.
fn entries(m: &TextMenu) -> Vec<Entry> {
    let field = m.field.clone();
    let url = m.params.link.clone().unwrap_or_default();
    let selection = m.params.selection.clone();
    // A key aimed at the field: focus goes back to it as the menu closes, then the key arrives as if typed.
    let to_field = move |c: char| {
        let field = field.clone();
        move |_: &mut State, tx: &mut Tx| {
            if let Some(key) = &field {
                tx.return_focus(key.clone(), false);
            }
            tx.send_key(press(c));
        }
    };
    rows(&m.params)
        .into_iter()
        .map(|row| match row {
            Row::Separator => Entry::Separator,
            Row::OpenLink => {
                let url = url.clone();
                Entry::Item(Item::new("Open Link in Browser").id("text-menu-open-link").on(move |_, tx| {
                    let _ = tx.open_url(&url);
                }))
            }
            Row::CopyLink => {
                let url = url.clone();
                Entry::Item(Item::new("Copy Link").id("text-menu-copy-link").on(move |_, tx| tx.copy(url.clone())))
            }
            Row::Cut(on) => Entry::Item(Item::new("Cut").id("text-menu-cut").disabled(!on).on(to_field.clone()('x'))),
            Row::Copy(on) if m.params.editable => {
                Entry::Item(Item::new("Copy").id("text-menu-copy").disabled(!on).on(to_field.clone()('c')))
            }
            Row::Copy(on) => {
                let text = selection.clone();
                Entry::Item(Item::new("Copy").id("text-menu-copy").disabled(!on).on(move |_, tx| tx.copy(text.clone())))
            }
            Row::Paste(on) => Entry::Item(Item::new("Paste").id("text-menu-paste").disabled(!on).on(to_field.clone()('v'))),
            Row::SelectAll => Entry::Item(Item::new("Select All").id("text-menu-select-all").on(to_field.clone()('a'))),
        })
        .collect()
}

/// The menu to mount while `Open::TextMenu` is open.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let Some(m) = &s.ui.text_menu else { return Vec::new() };
    // As wide as its longest row, as a native menu is: a link's "Open Link in Browser" is the longest.
    let width = if m.params.link.is_some() { Some(rem(12.0)) } else { None };
    context_w(t, "text", s.ui.point, entries(m), width)
}

impl State {
    /// Opens the menu over `params`, unless there is nothing to offer.
    pub fn open_text_menu(&mut self, params: Params, field: Option<String>, at: (f32, f32)) {
        if rows(&params).is_empty() {
            return;
        }
        self.ui.text_menu = Some(TextMenu { params, field });
        self.ui.show_context(Open::TextMenu, at);
    }
}

/// What the pointer is over when it is selected text: the selection the runtime holds.
fn selection(tx: &Tx) -> String {
    tx.text_selection().map(|s| s.text().to_owned()).unwrap_or_default()
}

/// A link's menu (`Markdown::on_link_secondary`, `El::on_link_secondary`): wherever links are drawn.
pub fn on_link(s: &mut State, url: &str, at: PointerPress, tx: &mut Tx) {
    let params = Params { link: Some(url.to_owned()), selection: selection(tx), ..Params::default() };
    s.open_text_menu(params, None, (at.x, at.y));
}

/// Selected text with no menu of its own: Copy, where Rune has none. Put on the element holding the text.
pub fn selectable(e: El) -> El {
    if !own_menus() {
        return e;
    }
    e.on_secondary_press(|s: &mut State, at: PointerPress, tx: &mut Tx| {
        s.open_text_menu(Params { selection: selection(tx), ..Params::default() }, None, (at.x, at.y));
    })
}

/// `rune::text_input`, with this menu where Rune draws none. Every field in the app is made by this.
pub fn text_input(edit: &TextEdit, style: rune::TextStyle) -> El {
    let field = rune::text_input(edit, style);
    if !own_menus() {
        return field;
    }
    let edit = edit.clone();
    field.on_secondary_press(move |s: &mut State, at: PointerPress, tx: &mut Tx| {
        let (a, b) = edit.range();
        let picked = a != b && !edit.is_secure();
        let params = Params {
            editable: true,
            can_cut: picked,
            can_copy: picked,
            can_paste: true,
            selection: if picked { edit.text[a.min(b)..a.max(b)].to_owned() } else { String::new() },
            link: None,
        };
        s.open_text_menu(params, None, (at.x, at.y));
        // The field's key is known only from what is under the pointer; focus it so the menu's keys reach it.
        tx.element_at(at.x, at.y, |s: &mut State, picked, tx: &mut Tx| {
            let Some(key) = picked.and_then(|p| p.key().map(str::to_owned)) else { return };
            if let Some(m) = &mut s.ui.text_menu {
                m.field = Some(key.clone());
            }
            tx.focus(key, false);
        });
    })
}
