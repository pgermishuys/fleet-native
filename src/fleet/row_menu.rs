//! The session row's right-click menu (`SessionItem.vue`, `ContextMenuContent`): which rows it has for this session and
//! machine, in Fleet's order, each with its hint. One menu serves a row of this machine and a row of another machine; what the
//! machine allows (nothing but Copy session ID when it isn't answering, Copy path for Open in, no Repeat on a schedule)
//! decides the rows. The look is `ui::menu`'s; the work is `row_menu_actions.rs` and the row's own handlers.
use super::groups::{ProjectKind, UNGROUPED_ID};
use super::lineage_moves::{movable_back_under, movable_out_of};
use super::settings::HOME_MACHINE_KEY as HOME_KEY;
use crate::icons::*;
use crate::mock::Session;
use crate::ui::menu::{Entry, Item};
use crate::{State, Tx};

/// The rows of the menu and the footer it shows while no row is highlighted.
pub struct RowMenu {
    pub entries: Vec<Entry>,
    pub footer: Option<String>,
}

/// `Ungrouped` for the scratch project; the rest by name. `Ungrouped` (no project) is first when the list has none.
pub fn project_targets(s: &State, machine: &str) -> Vec<(Option<String>, String)> {
    let known: Vec<(String, String, ProjectKind)> = if machine == HOME_KEY {
        s.projects.iter().map(|p| (p.id.clone(), p.name.clone(), p.kind)).collect()
    } else {
        s.machines
            .others
            .get(machine)
            .map(|m| m.projects.iter().map(|p| (p.id.clone(), p.name.clone(), p.kind)).collect())
            .unwrap_or_default()
    };
    let mut out: Vec<(Option<String>, String)> = Vec::new();
    for (id, name, kind) in known {
        if id == UNGROUPED_ID {
            out.insert(0, (None, "Ungrouped".into()));
        } else if kind == ProjectKind::Scratch {
            out.push((Some(id), "Ungrouped".into()));
        } else {
            out.push((Some(id), name));
        }
    }
    if !out.iter().any(|(id, _)| id.is_none()) {
        out.insert(0, (None, "Ungrouped".into()));
    }
    out
}

/// `<project> · <branch> · <harness>`, what is missing dropped.
fn footer_of(s: &State, session: &Session) -> Option<String> {
    let l = &session.listing;
    let folder = l.workspace_directory.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("").to_string();
    let project = l.project_name.clone().filter(|n| !n.is_empty()).or(Some(folder).filter(|f| !f.is_empty()));
    let harness = session
        .row
        .harness_type
        .as_deref()
        .and_then(|t| s.pickers.harnesses.iter().find(|h| h.kind == t))
        .map(|h| h.display_name.clone().unwrap_or_else(|| h.kind.clone()));
    let parts: Vec<String> = [project, Some(session.branch.clone()).filter(|b| !b.is_empty()), harness].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
}

/// The menu of `session`, which is on `machine` (`None`: this machine's list).
pub fn build(s: &State, session: &Session, machine: Option<&str>) -> RowMenu {
    let id = session.id.clone();
    let key = machine.unwrap_or(HOME_KEY).to_string();
    let elsewhere = machine.is_some_and(|m| m != HOME_KEY);
    let read_only = elsewhere && !s.machines.is_answering(&key);
    let archived = session.row.archived;
    let caps = &session.row.capabilities;
    let busy = s.is_restoring(&id);
    let mut entries: Vec<Entry> = Vec::new();
    let mut add = |it: Item| entries.push(Entry::Item(it));
    let copy_id = {
        let id = id.clone();
        Item::new("Copy session ID")
            .icon(&COPY)
            .id("session-context-copy-id")
            .hint(format!("Copies {id}."))
            .on(move |_: &mut State, tx: &mut Tx| tx.copy(id.clone()))
    };
    if read_only {
        add(copy_id);
        return RowMenu { entries, footer: None };
    }
    let at = |s: &State, id: &str| s.row_of(id);
    {
        let id = id.clone();
        let m = key.clone();
        add(Item::new("Rename").icon(&PENCIL).shortcut("F2").id("session-context-rename").on(move |s: &mut State, tx: &mut Tx| {
            match at(s, &id) {
                Some((p, i)) if !elsewhere => s.start_row_rename(p, i, tx),
                _ => s.start_machine_row_rename(&m, &id, tx),
            }
        }));
    }
    let remember = move |s: &mut State, id: &str, m: &str| {
        if elsewhere {
            s.machines.remember_sessions(m, &[id.to_string()]);
        }
    };
    let pinned = session.listing.pin_order.is_some();
    if !archived {
        let (id, m) = (id.clone(), key.clone());
        let (label, icon, hint) = if pinned {
            ("Unpin", &PIN_OFF, "Puts it back in its project, newest first.")
        } else {
            ("Pin to top", &PIN, "Keeps it in Pinned, above your projects. Drag pinned sessions to put them in order.")
        };
        add(Item::new(label).icon(icon).id("session-context-pin").hint(hint).on(move |s: &mut State, _: &mut Tx| {
            remember(s, &id, &m);
            s.toggle_pin(&id);
        }));
    }
    if pinned {
        for (label, icon, chip, up) in [("Move up", &ARROW_UP, "Alt+\u{2191}", true), ("Move down", &ARROW_DOWN, "Alt+\u{2193}", false)] {
            let (id, m) = (id.clone(), key.clone());
            let delta = if up { -1 } else { 1 };
            add(Item::new(label)
                .icon(icon)
                .shortcut(chip)
                .id(if up { "session-context-pin-up" } else { "session-context-pin-down" })
                .hint(format!("Or press {chip} on the row."))
                .disabled(!s.can_move_pin(&id, delta))
                .on(move |s: &mut State, _: &mut Tx| {
                    remember(s, &id, &m);
                    s.move_pin(&id, delta);
                }));
        }
    }
    if !archived && caps.can_archive.unwrap_or(true) {
        let (id, m) = (id.clone(), key.clone());
        add(Item::new("Archive")
            .icon(&ARCHIVE)
            .id("session-context-archive")
            .hint("Hides it from the list. You can undo it for a few seconds, then find it under Archived.")
            .disabled(busy)
            .on(move |s: &mut State, tx: &mut Tx| s.archive_sessions(std::slice::from_ref(&id), elsewhere.then_some(m.as_str()), tx)));
    }
    if archived {
        let (id, m) = (id.clone(), key.clone());
        add(Item::new("Restore")
            .icon(&ARCHIVE_RESTORE)
            .id("session-context-restore")
            .hint("Puts it back in the list.")
            .disabled(busy)
            .on(move |s: &mut State, _: &mut Tx| s.restore_session(&id, elsewhere.then_some(m.as_str()))));
    }
    if !archived {
        entries.push(Entry::Separator);
        let can_fork = caps.can_fork.unwrap_or(true);
        let id1 = id.clone();
        let mut fork = Item::new("Fork").icon(&GIT_FORK).id("session-context-fork").disabled(!can_fork);
        fork = if can_fork {
            fork.hint("A new session with a copy of this conversation.").on(move |s: &mut State, tx: &mut Tx| s.fork_row(&id1, tx))
        } else {
            fork.note(caps.fork_disabled_reason.clone().unwrap_or_else(|| "This session can't be forked.".into()))
        };
        entries.push(Entry::Item(fork));
        let id2 = id.clone();
        entries.push(Entry::Item(
            Item::new("New session in this folder")
                .icon(&PLUS)
                .id("session-context-new-in-folder")
                .hint("Same folder and harness, an empty conversation.")
                .disabled(busy)
                .on(move |s: &mut State, tx: &mut Tx| s.new_in_folder(&id2, tx)),
        ));
    }
    if !elsewhere {
        entries.push(Entry::Separator);
        entries.push(Entry::Item(
            Item::new("Repeat on a schedule\u{2026}")
                .icon(&REPEAT)
                .id("session-repeat-on-schedule")
                .hint("Opens Automations with this session's first message and folder filled in.")
                .on(|s: &mut State, tx: &mut Tx| {
                    s.menu_waiting("Repeat on a schedule", "Automations is its own component of the port", tx)
                }),
        ));
    }
    entries.push(Entry::Separator);
    if elsewhere {
        let dir = session.listing.workspace_directory.clone();
        let name = s.machines.entries().into_iter().find(|e| e.key == key).map(|e| e.name).unwrap_or_default();
        entries.push(Entry::Item(
            Item::new("Copy path")
                .icon(&COPY)
                .id("session-context-copy-path")
                .hint(format!("Copies {dir}. It's a folder on {name}, so it doesn't open here."))
                .disabled(dir.is_empty())
                .on(move |_: &mut State, tx: &mut Tx| tx.copy(dir.clone())),
        ));
    } else {
        entries.push(Entry::Item(
            Item::new("Open in")
                .icon(&EXTERNAL_LINK)
                .id("session-context-open-in")
                .hint(crate::view::open_in::HINT)
                .submenu_fit(crate::view::open_in::entries(s, &session.listing.workspace_directory)),
        ));
    }
    {
        let current = session.listing.project_id.clone();
        let now = project_targets(s, &key).into_iter().find(|(p, _)| *p == current).map(|(_, n)| n);
        let rows: Vec<Entry> = project_targets(s, &key)
            .into_iter()
            .map(|(project, name)| {
                let here = project == current;
                let (id, label) = (id.clone(), name.clone());
                let mut it = Item::new(name).on(move |s: &mut State, _: &mut Tx| s.move_to_project(&id, project.as_deref(), &label));
                if here {
                    it = it.mark(crate::ui::menu::Mark::Check(true));
                }
                Entry::Item(it)
            })
            .collect();
        let mut it = Item::new("Move to project").icon(&FOLDER_OPEN).id("session-context-move").disabled(busy).submenu(rows);
        if let Some(n) = now {
            it = it.hint(format!("Now in {n}."));
        }
        entries.push(Entry::Item(it));
    }
    if !archived && let Some(parent) = movable_out_of(&session.listing) {
        let id = id.clone();
        let label = parent_label(s, &parent).map_or("Move out of its parent".to_string(), |t| format!("Move out of \"{t}\""));
        entries.push(Entry::Item(
            Item::new(label)
                .icon(&CORNER_LEFT_UP)
                .id("session-context-move-out")
                .hint("Lists it on its own instead of under the session it came from.")
                .disabled(busy)
                .on(move |s: &mut State, tx: &mut Tx| s.move_lineage(&id, true, tx)),
        ));
    }
    if !archived && let Some(parent) = movable_back_under(&session.listing) {
        let id = id.clone();
        let label = parent_label(s, &parent).map_or("Move back under its parent".to_string(), |t| format!("Move back under \"{t}\""));
        entries.push(Entry::Item(
            Item::new(label)
                .icon(&CORNER_DOWN_RIGHT)
                .id("session-context-move-back")
                .hint("Lists it under the session it came from again.")
                .disabled(busy)
                .on(move |s: &mut State, tx: &mut Tx| s.move_lineage(&id, false, tx)),
        ));
    }
    entries.push(Entry::Item(copy_id));
    {
        let id = id.clone();
        entries.push(Entry::Item(
            Item::new("Clear attachments\u{2026}")
                .icon(&PAPERCLIP)
                .id("session-context-clear-attachments")
                .hint("Deletes the files you attached to this session's prompts. Fleet asks first.")
                .on(move |s: &mut State, tx: &mut Tx| match s.row_of(&id) {
                    Some((p, i)) if !elsewhere => s.ask_clear_attachments(p, i, tx),
                    _ => s.ask_machine_clear_attachments(&key_of(s, &id), &id, tx),
                }),
        ));
    }
    if caps.can_delete.unwrap_or(true) {
        let id = id.clone();
        entries.push(Entry::Separator);
        entries.push(Entry::Item(
            Item::new("Delete permanently\u{2026}")
                .icon(&TRASH_2)
                .destructive()
                .id("session-context-delete")
                .hint("Deletes the session and its history. Fleet asks first; it can't be undone.")
                .on(move |s: &mut State, tx: &mut Tx| match s.row_of(&id) {
                    Some((p, i)) if !elsewhere => s.ask_delete_session(p, i, tx),
                    _ => s.ask_machine_delete_session(&key_of(s, &id), &id, tx),
                }),
        ));
    }
    RowMenu { entries, footer: footer_of(s, session) }
}

fn key_of(s: &State, id: &str) -> String {
    s.machine_key_of(id).to_string()
}

/// The parent's title as the menu names it, when the list holds it.
fn parent_label(s: &State, id: &str) -> Option<String> {
    s.parent_title(id).filter(|t| !t.trim().is_empty())
}

// ---- scenes -----------------------------------------------------------------------------------------------------------

/// Which state of the menu a scene shows.
#[derive(Clone, Copy)]
pub enum Scene {
    /// A plain session, nothing highlighted: the footer says where it is.
    Plain,
    /// The pointer on Fork: its hint in the footer.
    Hint,
    /// An archived session: Restore, no Fork.
    Archived,
    /// A fork of the first session: Move out of it.
    Fork,
    /// A harness that can't fork: Fork off with its reason.
    ForkOff,
    /// Move to project open.
    Submenu,
    /// Open in open: the tools in their groups, an editor with key files (piece N).
    OpenIn,
    /// Open in open with that editor's key files open beside it.
    OpenInFiles,
    /// Open in while the tools are still being found.
    OpenInLoading,
    /// Open in on a machine with no tools.
    OpenInEmpty,
}

impl State {
    /// The menu on the second row of the first project, in `scene`.
    pub fn show_row_menu(&mut self, scene: Scene, dark: bool) {
        self.dark = Some(dark);
        let parent = self.projects[0].sessions[0].id.clone();
        let row = &mut self.projects[0].sessions[1];
        match scene {
            Scene::Plain
            | Scene::Hint
            | Scene::Submenu
            | Scene::OpenIn
            | Scene::OpenInFiles
            | Scene::OpenInLoading
            | Scene::OpenInEmpty => {}
            Scene::Archived => row.row.archived = true,
            Scene::Fork => {
                row.listing.forked_from = Some(parent);
                row.listing.spawn_kind = Some("fork".into());
            }
            Scene::ForkOff => {
                row.row.capabilities = super::rows::Capabilities {
                    can_fork: Some(false),
                    fork_disabled_reason: Some("This harness can't copy a conversation, so its sessions can't be forked.".into()),
                    ..Default::default()
                };
            }
        }
        row.branch = "main".into();
        if matches!(scene, Scene::Archived) {
            self.store.retention = super::store::Retention::Archived;
        }
        self.ui.show_context(crate::ui::Open::Context(0, 1), (210.0, 150.0));
        match scene {
            Scene::Hint => self.ui.hint = Some("A new session with a copy of this conversation.".into()),
            Scene::Submenu => {
                self.ui.sub = Some("session-context-move".into());
                self.ui.hint = Some("Now in Orchard.".into());
            }
            Scene::OpenIn | Scene::OpenInFiles | Scene::OpenInLoading | Scene::OpenInEmpty => {
                let o = &mut self.open_in;
                match scene {
                    Scene::OpenInLoading => o.tools = None,
                    Scene::OpenInEmpty => o.tools = Some(Vec::new()),
                    _ => {
                        o.tools = Some(super::open_in::mock_tools());
                        o.files = super::open_in::mock_key_files();
                    }
                }
                self.ui.sub = Some("session-context-open-in".into());
                self.ui.hint = Some(crate::view::open_in::HINT.into());
                if matches!(scene, Scene::OpenInFiles) {
                    self.ui.sub2 = Some("open-in-code".into());
                }
            }
            _ => {}
        }
    }
}
