//! The menus the shell opens: the composer's agent and model pickers, the header's "More
//! actions", and a session row's right-click menu. Entries and wording are Fleet's where
//! Fleet's are known; the agent and model lists are the session's own (`State::lists`).
use super::menu::{Entry, Item, Mark, context, dropdown as drop, item};
use crate::composer::selector as rules;
use crate::icons::*;
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{Align, El, Side};

fn waiting(label: &'static str, icon: &'static rune::Icon, n: u8) -> Entry {
    Entry::Item(Item::new(label).icon(icon).on(move |s: &mut State, tx: &mut Tx| {
        s.toast(format!("{label} isn't in the native app yet (component {n} of the port)."), None, 4.0, tx);
    }))
}

/// The composer's pickers: Fleet's selector box (`view/selector.rs`) above the chip, over a scrim that closes it.
fn selector(s: &State, t: &Theme, name: &'static str) -> Vec<El> {
    let key = format!("{name}-pop");
    let panel = crate::view::selector::panel(s, t, name).focus_return(name.to_string()).modal(true, |s: &mut State| s.close_selector());
    vec![
        super::overlay::scrim(&format!("{key}-scrim"), |s: &mut State| s.close_selector()),
        super::overlay::anchored(panel, name, Side::Top, Align::Start, rules::GAP),
    ]
}

pub fn dropdown(s: &State, t: &Theme, name: &'static str) -> Vec<El> {
    match name {
        "agent" => selector(s, t, "agent"),
        "model" => selector(s, t, "model"),
        "more" => drop(t, "more-pop", "more", more(), Side::Bottom, Align::End),
        _ => drop(t, "gallery-pop", "g-menu", gallery(s), Side::Bottom, Align::Start),
    }
}

fn more() -> Vec<Entry> {
    vec![
        waiting("Copy folder path", &COPY, 9),
        Entry::Item(Item::new("Rename").icon(&PENCIL).shortcut("Double-click title").on(|s: &mut State, tx: &mut Tx| {
            s.toast("Rename isn't in the native app yet (component 6 of the port).".into(), None, 4.0, tx);
        })),
        waiting("Export as Markdown…", &FILE_DOWN, 3),
        waiting("Archive", &ARCHIVE, 6),
        Entry::Separator,
        Entry::Item(Item::new("Delete…").icon(&TRASH_2).destructive().on(|s: &mut State, tx: &mut Tx| {
            s.toast("Delete isn't in the native app yet (component 6 of the port).".into(), None, 4.0, tx);
        })),
    ]
}

/// A session row's right-click menu (`SessionContextMenu`).
pub fn session_context(s: &State, t: &Theme, p: usize, i: usize) -> Vec<El> {
    let at = s.ui.point;
    let open = Entry::Item(Item::new("Open").icon(&MESSAGE_SQUARE).on(move |s: &mut State, tx: &mut Tx| s.open_session(p, i, tx)));
    context(
        t,
        "ctx",
        at,
        vec![
            open,
            waiting("Rename", &PENCIL, 6),
            waiting("Archive", &ARCHIVE, 6),
            Entry::Separator,
            Entry::Item(Item::new("Delete").icon(&X).destructive().on(|s: &mut State, tx: &mut Tx| {
                s.toast("Delete isn't in the native app yet (component 6 of the port).".into(), None, 4.0, tx);
            })),
        ],
    )
}

/// A project header's right-click menu (`ProjectGroup.vue`).
pub fn project_context(s: &State, t: &Theme, p: usize) -> Vec<El> {
    crate::view::project_menu::context(s, t, p)
}

/// The gallery's own menu: every row kind once.
fn gallery(s: &State) -> Vec<Entry> {
    let check = s.ui.g_check;
    vec![
        Entry::Label("Session".into()),
        Entry::Item(Item::new("Rename").icon(&PENCIL).shortcut("F2")),
        Entry::Item(Item::new("Fork session").icon(&GIT_FORK)),
        Entry::Item(Item::new("Copy session ID").icon(&COPY).disabled(true)),
        Entry::Separator,
        Entry::Label("View".into()),
        Entry::Item(Item::new("Show tool output").mark(Mark::Check(check)).on(|s, _| s.ui.g_check = !s.ui.g_check)),
        Entry::Item(Item::new("Compact").mark(Mark::Check(!check)).on(|s, _| s.ui.g_check = !s.ui.g_check)),
        Entry::Item(Item::new("Move to").icon(&FOLDER).submenu(vec![item("Orchard"), item("Lighthouse")])),
        Entry::Separator,
        Entry::Item(Item::new("Delete").icon(&X).destructive()),
    ]
}
