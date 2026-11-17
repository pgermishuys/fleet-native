//! Settings → Memory's notes (`MemorySection.vue`: the toolbar, the two lists, add, edit, forget, keep, move, clear): which
//! notes a search leaves, how a list splits when "Search older notes" is on, the words over a list, and what each button
//! asks the machine. Every call goes to the machine Settings shows and never blocks: a thread sends, then reads the overview
//! and the notes again, and the answer comes back to the UI thread. One action at a time (`run`).
use super::memory::{self, List, MemoryPage, Note, Notes, Overview, Request};
use crate::composer::format::format_count;
use crate::{State, Tx};
use rune::TextEdit;

/// `maxlength` on both text areas.
pub const MAX_NOTE_CHARS: usize = 400;

/// The forms and folds of the page (`search`, `adding`, `draft`, `editingId`, `editText`, `confirmingClear`, `showExpired`).
/// Dropped with the page when Settings goes to another one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryUi {
    pub search: TextEdit,
    /// The list whose add form is open.
    pub adding: Option<List>,
    pub draft: TextEdit,
    /// The note being edited, one at a time.
    pub editing: Option<String>,
    pub edit: TextEdit,
    pub confirming_clear: bool,
    pub show_expired: bool,
}

// A text edit is equal to itself; `Eq` lets the page hold one.
impl Eq for MemoryUi {}

// ─── What the lists show ────────────────────────────────────────────────────

/// `matches`: case-insensitive, over the note's words and the title of the session that saved it; blank matches all.
pub fn matches(note: &Note, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || note.text.to_lowercase().contains(&query)
        || note.session_title.as_deref().unwrap_or("").to_lowercase().contains(&query)
}

/// How a list's notes group: one box with no heading, or with "Search older notes" on the two groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    All,
    Always,
    Search,
}

/// One box of notes (`NoteSection`): `items` are what the search leaves; `title` and `meta` only with the two groups.
pub struct Section<'a> {
    pub group: Group,
    pub title: Option<&'static str>,
    pub meta: Option<String>,
    pub items: Vec<&'a Note>,
    pub empty: String,
}

/// One list with its heading's numbers (`group` in the template).
pub struct ListView<'a> {
    pub list: List,
    pub title: String,
    pub total: usize,
    pub max: i64,
    pub sections: Vec<Section<'a>>,
}

impl MemoryPage {
    pub fn search_on(&self) -> bool {
        self.overview.as_ref().is_some_and(|o| o.search_enabled)
    }

    /// The name of the repository the lists are about, else "this repository".
    pub fn repository_name(&self) -> &str {
        let wanted = self.repository.as_deref();
        self.overview
            .as_ref()
            .and_then(|o| o.repositories.iter().find(|r| Some(r.path.as_str()) == wanted))
            .map_or("this repository", |r| r.name.as_str())
    }

    /// Notes in both lists, whatever a search says (`noteCount`).
    pub fn note_count(&self) -> usize {
        self.notes.as_ref().map_or(0, |n| n.repository_notes.len() + n.machine_notes.len())
    }

    /// With search on: the notes every session reads, both lists (`readCount`).
    pub fn read_count(&self) -> usize {
        self.notes.as_ref().map_or(0, |n| n.repository_notes.iter().chain(&n.machine_notes).filter(|n| !n.in_search).count())
    }

    /// The line beside the toolbar (`memory-count`).
    pub fn count_line(&self) -> String {
        let n = self.note_count();
        let mut line = format!("{n} {}", if n == 1 { "note" } else { "notes" });
        if self.search_on() {
            line += &format!(" · {} read by every session", self.read_count());
        }
        let tokens = self.notes.as_ref().map_or(0, |n| n.tokens);
        line + &format!(" · about {} tokens per request", format_count(Some(tokens as f64)))
    }

    /// The notes of the expired list a search leaves.
    pub fn expired<'a>(&'a self, query: &str) -> Vec<&'a Note> {
        self.notes.as_ref().map(|n| n.expired_notes.iter().filter(|n| matches(n, query)).collect()).unwrap_or_default()
    }

    /// The two lists, repository first, as the page draws them.
    pub fn lists<'a>(&'a self, query: &str) -> Vec<ListView<'a>> {
        let (Some(overview), Some(notes)) = (&self.overview, &self.notes) else { return vec![] };
        let name = self.repository_name();
        let repository_empty = format!("No notes for {name} yet. Say “remember …” in a session, or add one here.");
        let machine_empty = "No notes about this machine yet. Agents save what a later session here would want to know.".to_string();
        let one = |list: List, title: String, all: &'a [Note], max: i64, empty: String| {
            let items: Vec<&Note> = all.iter().filter(|n| matches(n, query)).collect();
            ListView { list, title, total: all.len(), max, sections: sections_of(overview, list, all, items, empty) }
        };
        vec![
            one(List::Repository, format!("Repository · {name}"), &notes.repository_notes, overview.max_repository_notes, repository_empty),
            one(List::Machine, "This machine · every repository".into(), &notes.machine_notes, overview.max_machine_notes, machine_empty),
        ]
    }
}

/// `sectionsOf`. A missing maximum reads as 0: Fleet prints "undefined" there, and a server that turns search on sends them.
fn sections_of<'a>(overview: &Overview, list: List, everything: &[Note], items: Vec<&'a Note>, empty: String) -> Vec<Section<'a>> {
    if !overview.search_enabled {
        return vec![Section { group: Group::All, title: None, meta: None, items, empty }];
    }
    let machine = list == List::Machine;
    let always: Vec<&Note> = everything.iter().filter(|n| !n.in_search).collect();
    let found = everything.len() - always.len();
    let (always_max, search_max) = if machine {
        (overview.max_always_machine_notes, overview.max_search_machine_notes)
    } else {
        (overview.max_always_repository_notes, overview.max_search_repository_notes)
    };
    let tokens: i64 = always.iter().map(|n| memory::estimate_note_tokens(&n.text)).sum();
    let (read, searched): (Vec<&Note>, Vec<&Note>) = items.into_iter().partition(|n| !n.in_search);
    vec![
        Section {
            group: Group::Always,
            title: Some("Every session reads"),
            meta: Some(format!("{}/{} · about {} tokens", always.len(), always_max.unwrap_or(0), format_count(Some(tokens as f64)))),
            items: read,
            empty: "Nothing is read by every session. A new note goes here first.".into(),
        },
        Section {
            group: Group::Search,
            title: Some("Found by search"),
            meta: Some(format!("{found}/{} · no tokens until found", search_max.unwrap_or(0))),
            items: searched,
            empty: "Nothing here yet. When the notes every session reads are full, the least used one moves here.".into(),
        },
    ]
}

/// Where a note came from (`origin`).
pub fn origin(note: &Note, now_ms: i64, offset_secs: i64) -> String {
    use crate::fleet::row_status::{format_date, to_timestamp};
    let date = to_timestamp(&note.updated, offset_secs).map_or_else(|| note.updated.clone(), |at| format_date(at, now_ms, offset_secs));
    if note.kind == "added" {
        return format!("Added by you on {date}");
    }
    match note.session_title.as_deref().filter(|t| !t.is_empty()) {
        Some(title) => format!("Saved {date} by “{title}”"),
        None => format!("Saved {date}"),
    }
}

// ─── Actions ────────────────────────────────────────────────────────────────

/// What an action reads again once its calls are made.
#[derive(Clone, Copy)]
enum Reread {
    /// The notes only (an edit or a keep changes no count).
    Notes,
    /// The overview too: its counts and repositories.
    Everything,
}

/// What an action closes when it worked.
#[derive(Clone, Copy)]
enum Done {
    Nothing,
    Add,
    Edit,
    Clear,
}

struct Fresh {
    overview: Option<Overview>,
    notes: Notes,
}

fn ask<T>(base: &str, req: &Request, parse: impl Fn(&serde_json::Value) -> Option<T>) -> Result<T, String> {
    let body = memory::send(base, req)?;
    parse(&body).ok_or_else(|| req.fallback.clone())
}

impl State {
    /// `maxlength="400"`: what is typed past it is cut. Called each frame while the page is open; true when it cut.
    pub fn limit_memory_fields(&mut self) -> bool {
        let ui = &mut self.settings.memory_ui;
        let mut cut = false;
        for edit in [&mut ui.draft, &mut ui.edit] {
            if edit.text.chars().count() > MAX_NOTE_CHARS {
                edit.replace_text(edit.text.chars().take(MAX_NOTE_CHARS).collect::<String>());
                cut = true;
            }
        }
        cut
    }

    /// `run`: one action at a time. `calls` go in order, then what `reread` says; with no Fleet to ask (the invented page)
    /// nothing changes but the form that closes.
    fn run_memory(&mut self, calls: Vec<Request>, reread: Reread, done: Done) {
        if !self.begin_memory_action() {
            return;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        let request = self.settings.memory.request;
        let repository = self.settings.memory.repository.clone();
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            self.settings.memory.saving = false;
            self.close_memory_form(done);
            return;
        };
        link.off_thread(
            move |base| -> Result<Fresh, String> {
                for call in &calls {
                    memory::send(base, call)?;
                }
                let overview = match reread {
                    Reread::Everything => Some(ask(base, &memory::get_memory(), memory::parse_overview)?),
                    Reread::Notes => None,
                };
                let notes = ask(base, &memory::list_notes(repository.as_deref()), memory::parse_notes)?;
                Ok(Fresh { overview, notes })
            },
            move |s: &mut State, _tx: &mut Tx, got: Result<Fresh, String>| {
                if s.settings.memory.request != request {
                    return;
                }
                s.settings.memory.saving = false;
                match got {
                    Ok(fresh) => {
                        if let Some(overview) = fresh.overview {
                            s.settings.memory.overview = Some(overview);
                        }
                        s.settings.memory.notes = Some(fresh.notes);
                        s.close_memory_form(done);
                    }
                    Err(e) => s.settings.memory.error = Some(if e.is_empty() { "Something went wrong.".into() } else { e }),
                }
            },
        );
    }

    fn close_memory_form(&mut self, done: Done) {
        let ui = &mut self.settings.memory_ui;
        match done {
            Done::Nothing => {}
            Done::Add => ui.adding = None,
            Done::Edit => ui.editing = None,
            Done::Clear => ui.confirming_clear = false,
        }
    }

    /// The repository select (`watch(repository)`): the notes of the new one are read under the lock.
    pub fn memory_choose_repository(&mut self, path: &str) {
        let page = &mut self.settings.memory;
        if page.saving || page.repository.as_deref() == Some(path) {
            return;
        }
        page.repository = Some(path.to_string());
        self.run_memory(vec![], Reread::Notes, Done::Nothing);
    }

    pub fn memory_start_adding(&mut self, list: List) {
        let ui = &mut self.settings.memory_ui;
        ui.adding = Some(list);
        ui.draft = TextEdit::new("");
    }

    pub fn memory_cancel_adding(&mut self) {
        self.settings.memory_ui.adding = None;
    }

    pub fn memory_save_draft(&mut self) {
        let Some(list) = self.settings.memory_ui.adding else { return };
        let text = self.settings.memory_ui.draft.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let repository = (list == List::Repository).then(|| self.settings.memory.repository.clone()).flatten();
        self.run_memory(vec![memory::add_note(list, repository.as_deref(), &text)], Reread::Everything, Done::Add);
    }

    pub fn memory_start_editing(&mut self, id: &str) {
        let Some(note) =
            self.settings.memory.notes.as_ref().and_then(|n| n.repository_notes.iter().chain(&n.machine_notes).find(|n| n.id == id))
        else {
            return;
        };
        let text = note.text.clone();
        let ui = &mut self.settings.memory_ui;
        ui.editing = Some(id.to_string());
        ui.edit = TextEdit::new(text);
    }

    pub fn memory_cancel_editing(&mut self) {
        self.settings.memory_ui.editing = None;
    }

    pub fn memory_save_edit(&mut self, id: &str) {
        let text = self.settings.memory_ui.edit.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.run_memory(vec![memory::update_note(id, &text)], Reread::Notes, Done::Edit);
    }

    /// Keep for good, and Keep on an expired note.
    pub fn memory_keep(&mut self, id: &str) {
        self.run_memory(vec![memory::keep_note(id)], Reread::Notes, Done::Nothing);
    }

    /// No confirm: Undo exists only in the notice a saved note raises.
    pub fn memory_forget(&mut self, id: &str) {
        self.run_memory(vec![memory::forget_note(id)], Reread::Everything, Done::Nothing);
    }

    /// "Clear these N": no confirm either; Clear all has one.
    pub fn memory_clear_list(&mut self, list: List) {
        let repository = (list == List::Repository).then(|| self.settings.memory.repository.clone()).flatten();
        self.run_memory(vec![memory::clear(list.word(), repository.as_deref())], Reread::Everything, Done::Nothing);
    }

    pub fn memory_clear_all(&mut self) {
        self.run_memory(vec![memory::clear("all", None)], Reread::Everything, Done::Clear);
    }

    /// `search` or `always`.
    pub fn memory_move(&mut self, id: &str, group: &str) {
        self.run_memory(vec![memory::move_note(id, group)], Reread::Everything, Done::Nothing);
    }

    pub fn memory_set_confirming_clear(&mut self, on: bool) {
        self.settings.memory_ui.confirming_clear = on;
    }

    pub fn memory_toggle_expired(&mut self) {
        let ui = &mut self.settings.memory_ui;
        ui.show_expired = !ui.show_expired;
    }
}
