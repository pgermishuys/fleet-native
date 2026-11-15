//! The menus the shell opens: the composer's agent and model pickers, the header's "More
//! actions", and a session row's right-click menu. Entries and wording are Fleet's where
//! Fleet's are known; the agent and model lists are the session's own (`State::lists`).
use super::menu::{Entry, Item, Mark, context, dropdown as drop, dropdown_capped, item};
use crate::icons::*;
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{Align, El, Side};

/// `max-height: min(320px, 60vh)` of Fleet's selector menu; the viewport's height isn't known here, so 320.
const MENU_MAX_H: f32 = 320.0;

/// A picker's rows: Default first (empty id), then the list, the current pick marked.
fn rows(current: &str, default: String, listed: Vec<(String, String)>, pick: fn(&mut State, &str, &mut Tx)) -> Vec<Entry> {
    std::iter::once((String::new(), default))
        .chain(listed)
        .map(|(id, label)| {
            let on = id == current;
            Entry::Item(Item::new(label).mark(Mark::Radio(on)).on(move |s, tx| pick(s, &id, tx)))
        })
        .collect()
}

fn waiting(label: &'static str, icon: &'static rune::Icon, n: u8) -> Entry {
    Entry::Item(Item::new(label).icon(icon).on(move |s: &mut State, tx: &mut Tx| {
        s.toast(format!("{label} isn't in the native app yet (component {n} of the port)."), None, 4.0, tx);
    }))
}

pub fn dropdown(s: &State, t: &Theme, name: &'static str) -> Vec<El> {
    match name {
        "agent" => {
            let listed = s.lists().agents.iter().map(|a| (a.id.clone(), a.name.clone())).collect();
            let entries = rows(&s.choice().agent, s.default_labels().agent_label, listed, State::pick_agent);
            dropdown_capped(t, "agent-pop", "agent", entries, Side::Top, Align::Start, Some(MENU_MAX_H))
        }
        "model" => {
            let listed = s.lists().models.iter().map(|m| (m.selection_key.clone(), m.name.clone())).collect();
            let entries = rows(&s.choice().model, s.default_labels().model_label, listed, State::pick_model);
            dropdown_capped(t, "model-pop", "model", entries, Side::Top, Align::Start, Some(MENU_MAX_H))
        }
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
