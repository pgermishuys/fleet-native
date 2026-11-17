//! Agent memory, as plain data (`lib/agent-memory.ts`): the notes agents keep about a repository and about the machine, the
//! calls Settings → Memory makes under `/api/memory`, how their answers are read, and the words the page uses for a note.
//! No Rune and no I/O here but [`send`], which a thread calls. The page's state is [`MemoryPage`]; what touches the app is
//! `memory_actions.rs`.
use crate::fleet::http;
use crate::fleet::row_status::{format_date, to_timestamp};
use serde_json::{Value, json};

/// The server's preference for Memory itself (`MEMORY_PREFERENCE_KEY`).
pub const MEMORY_PREFERENCE_KEY: &str = "Memory";
/// The server's preference for "Search older notes" (`MEMORY_SEARCH_PREFERENCE_KEY`); Labs owns its switch.
pub const MEMORY_SEARCH_PREFERENCE_KEY: &str = "MemorySearch";

/// Which of the two lists a note is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    Repository,
    Machine,
}

impl List {
    pub fn word(self) -> &'static str {
        match self {
            List::Repository => "repository",
            List::Machine => "machine",
        }
    }
}

/// One note (`MemoryNote`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub list: List,
    pub text: String,
    /// `learned`: the agent found it out; `from-you`: the user asked, corrected or stated a preference; `added`: written in
    /// Settings. A kind the server adds later reads as learned, as Fleet's label does.
    pub kind: String,
    pub repository: Option<String>,
    pub session_id: Option<String>,
    pub session_title: Option<String>,
    pub created: String,
    pub updated: String,
    /// Days of use the note lasts; none when it never expires.
    pub lifetime: Option<i64>,
    pub days_left: Option<i64>,
    pub expired: bool,
    pub relearned: i64,
    /// How the agent knew a learned note holds; none when it hadn't checked.
    pub how: Option<String>,
    pub in_search: bool,
    pub pinned: bool,
    pub found: i64,
    pub last_found: Option<String>,
}

/// A repository the notes know (`MemoryRepository`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    pub path: String,
    pub name: String,
    pub count: i64,
}

/// `GET /api/memory` (`MemoryOverview`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overview {
    pub enabled: bool,
    pub repositories: Vec<Repository>,
    pub machine_count: i64,
    pub max_repository_notes: i64,
    pub max_machine_notes: i64,
    pub search_enabled: bool,
    pub max_always_repository_notes: Option<i64>,
    pub max_always_machine_notes: Option<i64>,
    pub max_search_repository_notes: Option<i64>,
    pub max_search_machine_notes: Option<i64>,
}

/// `GET /api/memory/notes` (`MemoryNotes`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notes {
    pub repository: Option<String>,
    pub repository_notes: Vec<Note>,
    pub machine_notes: Vec<Note>,
    /// Roughly what the notes add to each request a session sends.
    pub tokens: i64,
    pub expired_notes: Vec<Note>,
}

// ─── Reading answers ────────────────────────────────────────────────────────

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn whole(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_f64).map(|n| n as i64)
}

pub fn parse_note(v: &Value) -> Option<Note> {
    Some(Note {
        id: text(v, "id").filter(|id| !id.is_empty())?,
        list: if text(v, "list").as_deref() == Some("repository") { List::Repository } else { List::Machine },
        text: text(v, "text").unwrap_or_default(),
        kind: text(v, "kind").unwrap_or_else(|| "learned".into()),
        repository: text(v, "repository"),
        session_id: text(v, "sessionId"),
        session_title: text(v, "sessionTitle"),
        created: text(v, "created").unwrap_or_default(),
        updated: text(v, "updated").unwrap_or_default(),
        lifetime: whole(v, "lifetime"),
        days_left: whole(v, "daysLeft"),
        expired: v.get("expired").and_then(Value::as_bool) == Some(true),
        relearned: whole(v, "relearned").unwrap_or(0),
        how: text(v, "how"),
        in_search: v.get("inSearch").and_then(Value::as_bool) == Some(true),
        pinned: v.get("pinned").and_then(Value::as_bool) == Some(true),
        found: whole(v, "found").unwrap_or(0),
        last_found: text(v, "lastFound"),
    })
}

fn notes_in(v: &Value, key: &str) -> Vec<Note> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(parse_note).collect()).unwrap_or_default()
}

pub fn parse_overview(v: &Value) -> Option<Overview> {
    if !v.is_object() {
        return None;
    }
    let repositories = v
        .get("repositories")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(Repository {
                        path: text(r, "path")?,
                        name: text(r, "name").unwrap_or_default(),
                        count: whole(r, "count").unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Overview {
        enabled: v.get("enabled").and_then(Value::as_bool) == Some(true),
        repositories,
        machine_count: whole(v, "machineCount").unwrap_or(0),
        max_repository_notes: whole(v, "maxRepositoryNotes").unwrap_or(0),
        max_machine_notes: whole(v, "maxMachineNotes").unwrap_or(0),
        search_enabled: v.get("searchEnabled").and_then(Value::as_bool) == Some(true),
        max_always_repository_notes: whole(v, "maxAlwaysRepositoryNotes"),
        max_always_machine_notes: whole(v, "maxAlwaysMachineNotes"),
        max_search_repository_notes: whole(v, "maxSearchRepositoryNotes"),
        max_search_machine_notes: whole(v, "maxSearchMachineNotes"),
    })
}

pub fn parse_notes(v: &Value) -> Option<Notes> {
    if !v.is_object() {
        return None;
    }
    Some(Notes {
        repository: text(v, "repository"),
        repository_notes: notes_in(v, "repositoryNotes"),
        machine_notes: notes_in(v, "machineNotes"),
        tokens: whole(v, "tokens").unwrap_or(0),
        expired_notes: notes_in(v, "expiredNotes"),
    })
}

// ─── Requests ───────────────────────────────────────────────────────────────

const PATH: &str = "/api/memory";

/// One call, built but not sent: the method, the path, the body, and what to say when the server gives no reason
/// (`send`'s `fallback`).
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
    pub fallback: String,
}

fn request(method: &'static str, path: String, body: Option<Value>, fallback: &str) -> Request {
    Request { method, path, body, fallback: fallback.to_string() }
}

pub fn get_memory() -> Request {
    request("GET", PATH.into(), None, "Couldn't load memory.")
}

pub fn set_enabled(enabled: bool) -> Request {
    let fallback = format!("Couldn't turn memory {}.", if enabled { "on" } else { "off" });
    request("PUT", PATH.into(), Some(json!({ "enabled": enabled })), &fallback)
}

pub fn list_notes(repository: Option<&str>) -> Request {
    let query = repository.map(|r| format!("?repository={}", http::escape(r))).unwrap_or_default();
    request("GET", format!("{PATH}/notes{query}"), None, "Couldn't load the notes.")
}

pub fn add_note(list: List, repository: Option<&str>, text: &str) -> Request {
    request(
        "POST",
        format!("{PATH}/notes"),
        Some(json!({ "list": list.word(), "repository": repository, "text": text })),
        "Couldn't add the note.",
    )
}

pub fn update_note(id: &str, text: &str) -> Request {
    request("PUT", format!("{PATH}/notes/{}", http::escape(id)), Some(json!({ "text": text })), "Couldn't save the note.")
}

/// A learned note that should never expire, or an expired one brought back for good.
pub fn keep_note(id: &str) -> Request {
    request("POST", format!("{PATH}/notes/{}/keep", http::escape(id)), None, "Couldn't keep the note.")
}

/// With "Search older notes" on: moves a note to search (`search`), or pins it to "Always read" (`always`).
pub fn move_note(id: &str, group: &str) -> Request {
    request("POST", format!("{PATH}/notes/{}/place", http::escape(id)), Some(json!({ "group": group })), "Couldn't move the note.")
}

pub fn forget_note(id: &str) -> Request {
    request("DELETE", format!("{PATH}/notes/{}", http::escape(id)), None, "Couldn't forget the note.")
}

/// `scope` is `all`, `machine` or `repository`.
pub fn clear(scope: &str, repository: Option<&str>) -> Request {
    request("POST", format!("{PATH}/clear"), Some(json!({ "scope": scope, "repository": repository })), "Couldn't clear the notes.")
}

/// What a refused call says (`send`, `extractApiError`): the body's `detail`, `error`, `title` or `message` when it is a
/// string, else `fallback`. A body that is not JSON says the fallback, and so does an empty one.
pub fn memory_error(body: &str, fallback: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return fallback.to_string() };
    // `body.error ?? body`: a string error is the message, an object error is read as the body itself.
    let inner = match v.get("error") {
        Some(Value::String(s)) => return s.clone(),
        Some(e @ Value::Object(_)) => e,
        _ => &v,
    };
    ["detail", "error", "title", "message"]
        .iter()
        .find_map(|k| inner.get(k).and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// Sends one call to the machine at `base` and reads the answer: the body (null when it had none), or why not. Blocking: a
/// thread calls it.
pub fn send(base: &str, req: &Request) -> Result<Value, String> {
    let (status, body) = http::text_with(req.method, base, &req.path, req.body.as_ref()).map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(memory_error(&body, &req.fallback));
    }
    if body.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

// ─── Words ──────────────────────────────────────────────────────────────────

/// Roughly what one note adds to each request: its text and id at four characters a token (`estimateNoteTokens`;
/// JavaScript's `length` counts UTF-16 units).
pub fn estimate_note_tokens(text: &str) -> i64 {
    let units = text.encode_utf16().count() as i64;
    (units + 30 + 3) / 4
}

/// The "found" line of a note, or none when search has never returned it (`memoryFoundLabel`).
pub fn found_label(note: &Note, now_ms: i64, offset_secs: i64) -> Option<String> {
    if note.found <= 0 {
        return None;
    }
    let times = format!("found {} {}", note.found, if note.found == 1 { "time" } else { "times" });
    let Some(last) = note.last_found.as_deref().filter(|l| !l.is_empty()) else { return Some(times) };
    let date = to_timestamp(last, offset_secs).map_or_else(|| last.to_string(), |at| format_date(at, now_ms, offset_secs));
    Some(format!("{times} · last {date}"))
}

/// Where a note is kept, as the notice and the conversation say it (`memoryPlace`).
pub fn memory_place(list: List, repository_name: Option<&str>) -> String {
    match list {
        List::Machine => "this machine".into(),
        List::Repository => repository_name.unwrap_or("this repository").into(),
    }
}

/// How long a learned note has left, as Settings says it; none for a note from the user, which never expires
/// (`memoryLifeLabel`).
pub fn life_label(note: &Note) -> Option<String> {
    let again = if note.relearned > 0 { format!(" · learned {}×", note.relearned + 1) } else { String::new() };
    let Some(lifetime) = note.lifetime else {
        return (note.kind == "learned").then(|| format!("Kept for good{again}"));
    };
    if note.expired {
        return Some(format!("Expired after {lifetime} days of use{again}"));
    }
    let left = note.days_left.unwrap_or(lifetime);
    if left == 0 {
        return Some(format!("Last day of use{again}"));
    }
    Some(format!("{left} more {} of use{again}", if left == 1 { "day" } else { "days" }))
}

/// How Settings labels where a note came from (`memoryKindLabel`).
pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "from-you" => "From you",
        "added" => "Added by you",
        _ => "Learned",
    }
}

// ─── The page's state ───────────────────────────────────────────────────────

/// What Settings → Memory holds while it is open (`MemorySection.vue`'s refs). Dropped when the page is left, so coming back
/// reads the machine again, as the page mounting does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryPage {
    pub overview: Option<Overview>,
    pub notes: Option<Notes>,
    /// Overview and notes are on their way (`isLoading`).
    pub loading: bool,
    /// One action at a time (`isSaving`).
    pub saving: bool,
    pub error: Option<String>,
    /// The repository the lists are about (`repository`).
    pub repository: Option<String>,
    /// The machine key the page was asked of; the page asks again when the machine Settings shows changes.
    pub loaded_for: Option<String>,
    /// Numbers each read so one a newer read has overtaken is dropped when it arrives.
    pub request: u64,
}

impl MemoryPage {
    pub fn enabled(&self) -> bool {
        self.overview.as_ref().is_some_and(|o| o.enabled)
    }

    /// The lists show only with Memory on and the notes read.
    pub fn shows_lists(&self) -> bool {
        self.enabled() && self.notes.is_some()
    }
}
