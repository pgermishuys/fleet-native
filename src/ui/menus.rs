//! The menus the shell opens: the composer's agent and model pickers, the header's "More
//! actions", and a session row's right-click menu. Entries and wording are Fleet's where
//! Fleet's are known; the agent and model lists are the session's own (`State::lists`).
use super::menu::{Entry, Item, Mark, dropdown as drop, item};
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
        "more" => super::menu::dropdown_w(t, "more-pop", "more", more(s), Side::Bottom, Align::End, super::rem(14.0)),
        _ => drop(t, "gallery-pop", "g-menu", gallery(s), Side::Bottom, Align::Start),
    }
}

/// The header's ⋯ menu (`SessionActionToolbar.vue`, `w-56`): what the open session's capabilities allow, in Fleet's order.
/// A phone's extra rows (usage, Show terminal, Fork) are not here.
pub fn more(s: &State) -> Vec<Entry> {
    let m = s.header_model();
    let pending = s.toolbar_pending();
    let mut entries = Vec::new();
    if !m.directory.is_empty() {
        let dir = m.directory.clone();
        // No "Copied" and no toast: the menu closing is the answer.
        entries.push(Entry::Item(Item::new("Copy folder path").icon(&COPY).on(move |_: &mut State, tx: &mut Tx| tx.copy(dir.clone()))));
    }
    if !m.archived {
        entries.push(Entry::Item(
            Item::new("Rename")
                .icon(&PENCIL)
                .shortcut("Double-click title")
                .disabled(pending)
                .on(|s: &mut State, tx: &mut Tx| s.start_header_rename(tx)),
        ));
    }
    entries.push(Entry::Item(Item::new("Export as Markdown…").icon(&FILE_DOWN).on(|s: &mut State, _: &mut Tx| s.export_open())));
    if m.actions.can_archive {
        entries.push(Entry::Item(
            Item::new("Archive").icon(&ARCHIVE).disabled(pending).on(|s: &mut State, tx: &mut Tx| s.toolbar_archive(tx)),
        ));
    }
    if m.actions.can_restore {
        entries.push(Entry::Item(
            Item::new("Restore").icon(&ARCHIVE_RESTORE).disabled(pending).on(|s: &mut State, tx: &mut Tx| s.toolbar_restore(tx)),
        ));
    }
    if m.actions.can_delete {
        entries.push(Entry::Separator);
        entries.push(Entry::Item(
            Item::new("Delete…").icon(&TRASH_2).destructive().disabled(pending).on(|s: &mut State, tx: &mut Tx| s.toolbar_delete(tx)),
        ));
    }
    entries
}

/// A session row's right-click menu (`SessionContextMenu`).
pub fn session_context(s: &State, t: &Theme, p: usize, i: usize) -> Vec<El> {
    let at = s.ui.point;
    let open = Entry::Item(Item::new("Open").icon(&MESSAGE_SQUARE).on(move |s: &mut State, tx: &mut Tx| s.open_session(p, i, tx)));
    // `ContextMenuContent class="w-66"`.
    super::menu::context_w(
        t,
        "ctx",
        at,
        vec![
            open,
            Entry::Item(
                Item::new("Rename").icon(&PENCIL).shortcut("F2").on(move |s: &mut State, tx: &mut Tx| s.start_row_rename(p, i, tx)),
            ),
            waiting("Archive", &ARCHIVE, 6),
            Entry::Item(
                Item::new("Clear attachments…").icon(&PAPERCLIP).on(move |s: &mut State, tx: &mut Tx| s.ask_clear_attachments(p, i, tx)),
            ),
            Entry::Separator,
            Entry::Item(
                Item::new("Delete permanently…")
                    .icon(&TRASH_2)
                    .destructive()
                    .on(move |s: &mut State, tx: &mut Tx| s.ask_delete_session(p, i, tx)),
            ),
        ],
        Some(super::rem(16.5)),
    )
}

/// The right-click menu of a row on another machine: this machine's menu, with Copy path where Open in is (the folder is on
/// that machine) and no Repeat on a schedule (Automations is this machine's page).
pub fn machine_session_context(s: &State, t: &Theme) -> Vec<El> {
    let Some((machine, id)) = s.machine_menu.clone() else { return Vec::new() };
    let (m1, i1, m2, i2, m3, i3, m4, i4) =
        (machine.clone(), id.clone(), machine.clone(), id.clone(), machine.clone(), id.clone(), machine.clone(), id.clone());
    super::menu::context_w(
        t,
        "ctx",
        s.ui.point,
        vec![
            Entry::Item(
                Item::new("Open").icon(&MESSAGE_SQUARE).on(move |s: &mut State, tx: &mut Tx| s.open_machine_session(&machine, &id, tx)),
            ),
            Entry::Item(Item::new("Rename").icon(&PENCIL).shortcut("F2").on(move |s: &mut State, tx: &mut Tx| {
                s.ui.open = None;
                s.start_machine_row_rename(&m1, &i1, tx);
            })),
            waiting("Archive", &ARCHIVE, 6),
            Entry::Item(Item::new("Copy path").icon(&COPY).on(move |s: &mut State, tx: &mut Tx| {
                s.ui.open = None;
                s.copy_machine_path(&m2, &i2, tx);
            })),
            Entry::Item(
                Item::new("Clear attachments…")
                    .icon(&PAPERCLIP)
                    .on(move |s: &mut State, tx: &mut Tx| s.ask_machine_clear_attachments(&m3, &i3, tx)),
            ),
            Entry::Separator,
            Entry::Item(
                Item::new("Delete permanently…")
                    .icon(&TRASH_2)
                    .destructive()
                    .on(move |s: &mut State, tx: &mut Tx| s.ask_machine_delete_session(&m4, &i4, tx)),
            ),
        ],
        Some(super::rem(16.5)),
    )
}

/// The right-click menu of a project on another machine: Rename, which goes to that machine. New session, Move and Delete
/// wait for component 7 and for the project actions to take a machine (docs/parity/17-machines.md, piece F).
pub fn machine_project_context(s: &State, t: &Theme) -> Vec<El> {
    let Some((machine, id)) = s.machine_menu.clone() else { return Vec::new() };
    let busy = s.proj.pending;
    super::menu::context_w(
        t,
        "pctx",
        s.ui.point,
        vec![Entry::Item(Item::new("Rename").icon(&PENCIL).shortcut("F2").disabled(busy).on(move |s: &mut State, tx: &mut Tx| {
            s.ui.open = None;
            s.start_machine_rename(&machine, &id, tx);
        }))],
        Some(196.0),
    )
}

/// A machine heading's right-click menu (`MachineHeader.vue`, `w-64`). Home's tree is the one the app works in ("live"):
/// New session and New project plain, no Work here. Another machine's items say "on <name>", are off while it isn't
/// answering, and Work here (always on) makes it the machine you're on.
pub fn machine_heading_context(s: &State, t: &Theme) -> Vec<El> {
    let Some((key, _)) = s.machine_menu.clone() else { return Vec::new() };
    super::menu::context_w(t, "mctx", s.ui.point, machine_heading_entries(s, key), Some(super::rem(16.0)))
}

/// The rows of `machine_heading_context` for machine `key`.
pub fn machine_heading_entries(s: &State, key: String) -> Vec<Entry> {
    let live = key == crate::fleet::settings::HOME_MACHINE_KEY;
    let name = s.machines.entries().into_iter().find(|e| e.key == key).map(|e| e.name).unwrap_or_default();
    let off = !live && !s.machines.is_answering(&key);
    let named = |item: Item, before: &str| if live { item } else { item.with_name(before, &name) };
    let (k1, k2, k3) = (key.clone(), key.clone(), key);
    let mut entries = vec![
        Entry::Item(
            named(Item::new("New session"), "New session on")
                .icon(&PLUS)
                .disabled(off)
                .on(move |s: &mut State, _: &mut Tx| s.new_session_on(&k1)),
        ),
        // The dialog is component 6's piece O.
        Entry::Item(
            named(Item::new("New project"), "New project on")
                .icon(&FOLDER_PLUS)
                .disabled(true)
                .on(move |s: &mut State, _: &mut Tx| s.new_project_on(&k2)),
        ),
    ];
    if !live {
        entries.push(Entry::Separator);
        entries.push(Entry::Item(Item::new("Work here").icon(&MONITOR).on(move |s: &mut State, _: &mut Tx| s.work_here(&k3))));
    }
    entries
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
