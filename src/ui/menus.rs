//! The menus the shell opens: the composer's agent and model pickers, the header's "More
//! actions", and a session row's right-click menu. Entries and wording are Fleet's where
//! Fleet's are known; the agent and model lists are the session's own (`State::lists`).
use super::menu::{Entry, Item, Mark, dropdown as drop, item};
use crate::composer::selector as rules;
use crate::icons::*;
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{Align, El, Side};

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
        "canvas" => crate::view::canvas_host::picker(s, t),
        "help" => super::menu::dropdown_w(t, "help-pop", "rail-help", help(), Side::Right, Align::End, super::rem(14.0)),
        "more" => super::menu::dropdown_w(t, "more-pop", "more", more(s), Side::Bottom, Align::End, super::rem(14.0)),
        "retention" => super::menu::dropdown_w(t, "retention-pop", "home-filter", retention(s), Side::Bottom, Align::End, super::rem(12.0)),
        _ => drop(t, "gallery-pop", "g-menu", gallery(s), Side::Bottom, Align::Start),
    }
}

/// The home page's "Show:" menu (`RetentionFilter.vue`, `w-48`): the three views of the list, a check after the one shown. The
/// pick is the sidebar's too (`store.retention`).
pub fn retention(s: &State) -> Vec<Entry> {
    use crate::fleet::store::Retention;
    [(Retention::Active, "active"), (Retention::Archived, "archived"), (Retention::All, "all")]
        .into_iter()
        .map(|(r, key)| {
            Entry::Item(
                Item::new(crate::view::dashboard::retention_label(r))
                    .id(format!("retention-filter-option-{key}"))
                    .end_check(s.store.retention == r)
                    .on(move |s: &mut State, _: &mut Tx| s.pick_retention(r)),
            )
        })
        .collect()
}

/// The rail's Help menu (`IconRail.vue`, `w-56`). Report a problem opens the dialog once piece Q builds it; What's new is
/// Settings → Updates & about (`useWhatsNew`).
pub fn help() -> Vec<Entry> {
    vec![
        Entry::Item(Item::new("Report a problem…").icon(&BUG).on(|s: &mut State, _: &mut Tx| {
            s.report_open(crate::fleet::report::Source::Help, None, "");
        })),
        Entry::Separator,
        Entry::Item(Item::new("What's new").icon(&SPARKLES).on(|s: &mut State, _: &mut Tx| {
            s.open_settings("updates", None);
        })),
    ]
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

/// A session row's right-click menu (`SessionContextMenu`, `w-66`): one menu for a row of this machine and for a row of
/// another machine, its rows decided by what each allows (`fleet::row_menu`).
pub fn session_context(s: &State, t: &Theme, p: usize, i: usize) -> Vec<El> {
    let Some(session) = s.projects.get(p).and_then(|project| project.sessions.get(i)) else { return Vec::new() };
    row_menu(s, t, session, None)
}

/// The right-click menu of a row on another machine: the same menu, with Copy path where Open in is (the folder is on that
/// machine) and no Repeat on a schedule.
pub fn machine_session_context(s: &State, t: &Theme) -> Vec<El> {
    let Some((machine, id)) = s.machine_menu.clone() else { return Vec::new() };
    let Some(session) = s.machines.others.get(&machine).and_then(|m| m.sessions.iter().find(|x| x.id == id)) else { return Vec::new() };
    row_menu(s, t, session, Some(&machine))
}

fn row_menu(s: &State, t: &Theme, session: &crate::mock::Session, machine: Option<&str>) -> Vec<El> {
    let menu = crate::fleet::row_menu::build(s, session, machine);
    // The footer says the hovered row's hint, else where the session is; the read-only menu has none.
    let footer = menu.footer.map(|f| s.ui.hint.clone().unwrap_or(f)).or_else(|| s.ui.hint.clone());
    super::menu::context_nested(
        t,
        "ctx",
        s.ui.point,
        menu.entries,
        Some(super::rem(16.5)),
        footer,
        (s.ui.sub.as_deref(), s.ui.sub2.as_deref()),
    )
}

/// The right-click menu of a project on another machine (`ProjectGroup.vue`'s, the same items as home's): New session on that
/// machine, Rename, Move up and Move down, Delete…. Every call goes to that machine.
pub fn machine_project_context(s: &State, t: &Theme) -> Vec<El> {
    let Some((machine, id)) = s.machine_menu.clone() else { return Vec::new() };
    let busy = s.proj.pending;
    let (m1, i1, m2, i2) = (machine.clone(), id.clone(), machine.clone(), id.clone());
    let mut entries = vec![
        Entry::Item(Item::new("New session").icon(&PLUS).disabled(busy).on(move |s: &mut State, _: &mut Tx| {
            s.ui.open = None;
            s.new_session_in_machine_project(&m1, &i1);
        })),
        Entry::Separator,
        Entry::Item(Item::new("Rename").icon(&PENCIL).shortcut("F2").disabled(busy).on(move |s: &mut State, tx: &mut Tx| {
            s.ui.open = None;
            s.start_machine_rename(&m2, &i2, tx);
        })),
    ];
    for (up, label, icon) in [(true, "Move up", &ARROW_UP), (false, "Move down", &ARROW_DOWN)] {
        if s.can_move_machine_project(&machine, &id, up) {
            let (m, i) = (machine.clone(), id.clone());
            entries.push(Entry::Item(Item::new(label).icon(icon).disabled(busy).on(move |s: &mut State, tx: &mut Tx| {
                s.ui.open = None;
                s.move_machine_project(&m, &i, up, tx);
            })));
        }
    }
    entries.push(Entry::Separator);
    entries.push(Entry::Item(Item::new("Delete\u{2026}").icon(&TRASH_2).destructive().disabled(busy).on(
        move |s: &mut State, _: &mut Tx| {
            s.ui.open = None;
            let count = crate::view::machine_group::group_count(s, &machine, &id);
            s.open_machine_delete(&machine, &id, count);
        },
    )));
    super::menu::context_w(t, "pctx", s.ui.point, entries, Some(196.0))
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
        Entry::Item(
            named(Item::new("New project"), "New project on")
                .icon(&FOLDER_PLUS)
                .disabled(off)
                .on(move |s: &mut State, tx: &mut Tx| s.new_project_on(&k2, tx)),
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
