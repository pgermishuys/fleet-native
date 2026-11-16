//! A draft per session (`draft-storage.ts`, `use-draft-state.ts`): the text each session's composer holds, kept in
//! memory and in a small JSON file in the app's own data folder, so a restart doesn't lose what was typed. The
//! agent, model and effort picked for the next prompt are in memory only, as in Fleet. The message history
//! (`use-input-history.ts`) goes in the same file.
//!
//! Reading and writing the file is blocking: `load` and `spawn_save` run it on a thread, never the UI thread.
use super::history::History;
use super::refs::{Picks, SessionReference};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// The file inside the data folder.
pub const FILE: &str = "drafts.json";
/// How long a change waits before it is written, so typing writes once.
pub const SAVE_AFTER: f64 = 0.3;

/// Fleet's keys, without its `weave-fleet.draft.` prefix: the composer's own text, a side conversation's draft
/// while the composer talks to the session, and the session's draft while it talks to the side conversation.
pub fn session_draft_key(session: &str) -> String {
    format!("session.{session}")
}
pub fn side_draft_key(session: &str, side: &str) -> String {
    format!("side.{session}.{side}")
}
pub fn side_main_draft_key(session: &str) -> String {
    format!("side-main.{session}")
}

/// `appendDraftReference`: the reference and one space at the end, a space before it unless the text is empty or already ends
/// in whitespace.
pub fn with_reference(text: &str, reference: &str) -> String {
    let gap = if text.is_empty() || text.ends_with(char::is_whitespace) { "" } else { " " };
    format!("{text}{gap}{reference} ")
}

/// `appendDraftText`: a blank draft becomes `add`; otherwise `add` follows the old text after a blank line.
pub fn with_text(text: &str, add: &str) -> String {
    if text.trim().is_empty() { add.to_string() } else { format!("{}\n\n{add}", text.trim_end()) }
}

/// What is picked for the next prompt in a session; kept in memory only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    pub agent: String,
    pub model: String,
    pub effort: String,
}

/// The app's own data folder, for what it keeps (never Fleet's `.weave`, which belongs to the server):
/// `FLEET_NATIVE_DATA_DIR`, else the platform's usual place under the home folder. `get` reads the environment
/// so a test can give it one. With no home folder there is no folder, so nothing is kept, never a fixed path.
pub fn app_dir(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |k: &str| get(k).filter(|v| !v.is_empty());
    if let Some(dir) = set("FLEET_NATIVE_DATA_DIR") {
        return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
        return Some(
            Path::new(&set("APPDATA").or_else(|| set("USERPROFILE").map(|h| format!("{h}\\AppData\\Roaming")))?).join("fleet-native"),
        );
    }
    let home = set("HOME")?;
    if cfg!(target_os = "macos") {
        return Some(Path::new(&home).join("Library/Application Support/fleet-native"));
    }
    let base = set("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| Path::new(&home).join(".local/share"));
    Some(base.join("fleet-native"))
}

/// What the file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stored {
    pub drafts: BTreeMap<String, String>,
    pub history: BTreeMap<String, Vec<String>>,
    /// The files, folders and sessions picked from the `@` list into each draft, by the draft's owner.
    pub picks: BTreeMap<String, Picks>,
}

#[derive(Debug, Default)]
pub struct Drafts {
    texts: BTreeMap<String, String>,
    history: HashMap<String, History>,
    choices: HashMap<String, Choice>,
    picks: BTreeMap<String, Picks>,
    /// The data folder, once the app knows it; `None` keeps everything in memory (tests, scenes, pictures).
    dir: Option<PathBuf>,
    /// When the oldest unwritten change was made, on the app's clock.
    dirty_since: Option<f64>,
}

impl Drafts {
    /// `readStoredDraft`.
    pub fn read(&self, key: &str) -> Option<&str> {
        self.texts.get(key).map(String::as_str)
    }

    /// `writeStoredDraft`: an empty draft removes the key. Returns whether anything changed.
    pub fn write(&mut self, key: &str, text: &str, now: f64) -> bool {
        let changed = if text.is_empty() {
            self.texts.remove(key).is_some()
        } else {
            self.texts.insert(key.to_string(), text.to_string()).as_deref() != Some(text)
        };
        if changed {
            self.touch(now);
        }
        changed
    }

    /// `storedDraftKeys`: the keys that start with `start`.
    pub fn keys(&self, start: &str) -> Vec<String> {
        self.texts.keys().filter(|k| k.starts_with(start)).cloned().collect()
    }

    /// `clearStoredDraftsOf`: everything kept for a session, once it is deleted.
    pub fn clear_of(&mut self, session: &str, now: f64) {
        for start in [session_draft_key(session), format!("side.{session}."), side_main_draft_key(session)] {
            for key in self.keys(&start) {
                self.write(&key, "", now);
            }
        }
        self.history.remove(session);
        self.choices.remove(session);
        self.picks.remove(session);
        self.touch(now);
    }

    /// The text of a session's own draft.
    pub fn text_of(&self, session: &str) -> &str {
        self.read(&session_draft_key(session)).unwrap_or("")
    }

    pub fn choice(&self, session: &str) -> Choice {
        self.choices.get(session).cloned().unwrap_or_default()
    }

    pub fn set_choice(&mut self, session: &str, choice: Choice) {
        if choice == Choice::default() {
            self.choices.remove(session);
        } else {
            self.choices.insert(session.to_string(), choice);
        }
    }

    /// The picks kept with a session's draft (`file-refs.` and the remembered sessions of Fleet's draft storage).
    pub fn picks_of(&self, owner: &str) -> Option<&Picks> {
        self.picks.get(owner)
    }

    /// Keeps a draft's picks with it; nothing picked removes the entry. Returns whether anything changed.
    pub fn set_picks(&mut self, owner: &str, picks: &Picks, now: f64) -> bool {
        let empty = picks.files().is_empty() && picks.sessions().is_empty();
        let changed = if empty { self.picks.remove(owner).is_some() } else { self.picks.get(owner) != Some(picks) };
        if changed && !empty {
            self.picks.insert(owner.to_string(), picks.clone());
        }
        if changed {
            self.touch(now);
        }
        changed
    }

    pub fn history(&self, session: &str) -> Option<&History> {
        self.history.get(session)
    }

    pub fn history_mut(&mut self, session: &str) -> &mut History {
        self.history.entry(session.to_string()).or_default()
    }

    /// Records a sent text in the session's history (row 8.1).
    pub fn push_history(&mut self, session: &str, text: &str, now: f64) {
        if self.history_mut(session).push(text) {
            self.touch(now);
        }
    }

    /// Takes what the file held. A session typed in since the app started keeps what it has.
    pub fn merge(&mut self, stored: Stored) {
        for (key, text) in stored.drafts {
            if !text.is_empty() {
                self.texts.entry(key).or_insert(text);
            }
        }
        for (owner, picks) in stored.picks {
            self.picks.entry(owner).or_insert(picks);
        }
        for (session, entries) in stored.history {
            self.history.entry(session).or_insert_with(|| History::from_entries(entries));
        }
    }

    pub fn set_dir(&mut self, dir: Option<PathBuf>) {
        self.dir = dir;
    }

    fn touch(&mut self, now: f64) {
        self.dirty_since.get_or_insert(now);
    }

    /// Whether a write is waiting and has waited long enough.
    pub fn save_due(&self, now: f64) -> bool {
        self.dir.is_some() && self.dirty_since.is_some_and(|since| now - since >= SAVE_AFTER)
    }

    /// Whether a write is waiting (so the app asks for a later `poll`).
    pub fn unsaved(&self) -> bool {
        self.dir.is_some() && self.dirty_since.is_some()
    }

    /// The folder and the file's content, taking the pending change: hand both to `spawn_save`.
    pub fn take_save(&mut self) -> Option<(PathBuf, String)> {
        let dir = self.dir.clone()?;
        self.dirty_since = None;
        Some((dir, self.to_json()))
    }

    fn to_json(&self) -> String {
        // The invented sessions of the mock data (`mock.<project>.<session>`) are never written down.
        let drafts: Map<String, Value> =
            self.texts.iter().filter(|(k, _)| !k.contains(".mock.")).map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
        let history: Map<String, Value> = self
            .history
            .iter()
            .filter(|(k, h)| !h.entries.is_empty() && !k.starts_with("mock."))
            .map(|(k, h)| (k.clone(), Value::Array(h.entries.iter().cloned().map(Value::String).collect())))
            .collect();
        let picks: Map<String, Value> =
            self.picks.iter().filter(|(k, _)| !k.starts_with("mock.")).map(|(k, p)| (k.clone(), picks_json(p))).collect();
        let mut root = Map::new();
        root.insert("drafts".into(), Value::Object(drafts));
        root.insert("picks".into(), Value::Object(picks));
        root.insert("history".into(), Value::Object(history));
        Value::Object(root).to_string()
    }
}

fn picks_json(picks: &Picks) -> Value {
    let sessions: Vec<Value> =
        picks.sessions().iter().map(|r| serde_json::json!({ "token": r.token, "sessionId": r.session_id, "title": r.title })).collect();
    serde_json::json!({ "files": picks.files(), "sessions": sessions })
}

fn parse_picks(value: &Value) -> Picks {
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect());
    let mut sessions: Vec<SessionReference> = value.get("sessions").and_then(Value::as_array).map_or_else(Vec::new, |a| {
        a.iter()
            .filter_map(|r| {
                Some(SessionReference {
                    token: text(r, "token")?,
                    session_id: text(r, "sessionId")?,
                    title: text(r, "title").unwrap_or_default(),
                })
            })
            .collect()
    });
    let extra = sessions.len().saturating_sub(super::refs::MAX_REMEMBERED);
    sessions.drain(..extra);
    Picks::from_parts(files, sessions)
}

/// Parses the file; anything that isn't one is an empty store (a damaged file loses its drafts, nothing worse).
pub fn parse(json: &str) -> Stored {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(json) else { return Stored::default() };
    let mut stored = Stored::default();
    if let Some(Value::Object(drafts)) = root.get("drafts") {
        for (key, text) in drafts {
            if let Some(text) = text.as_str().filter(|t| !t.is_empty()) {
                stored.drafts.insert(key.clone(), text.to_string());
            }
        }
    }
    if let Some(Value::Object(picks)) = root.get("picks") {
        for (owner, value) in picks {
            let parsed = parse_picks(value);
            if !parsed.files().is_empty() || !parsed.sessions().is_empty() {
                stored.picks.insert(owner.clone(), parsed);
            }
        }
    }
    if let Some(Value::Object(history)) = root.get("history") {
        for (session, entries) in history {
            if let Value::Array(entries) = entries {
                let entries: Vec<String> = entries.iter().filter_map(|e| e.as_str().map(str::to_string)).collect();
                stored.history.insert(session.clone(), entries);
            }
        }
    }
    stored
}

/// Reads the file in `dir`; no file, or one that can't be read, is an empty store ("works in memory when storage
/// throws"). Blocking.
pub fn load(dir: &Path) -> Stored {
    std::fs::read_to_string(dir.join(FILE)).map(|json| parse(&json)).unwrap_or_default()
}

/// Writes the file: to a temporary name first, then renamed over it, so a crash leaves the old one whole.
/// Blocking. A failure is dropped: the drafts stay in memory.
pub fn write_file(dir: &Path, json: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, dir.join(FILE))
}

static ASKED: AtomicU64 = AtomicU64::new(0);
static WRITTEN: Mutex<u64> = Mutex::new(0);

/// Writes unless a later write already went out, so a slow thread never puts older text over newer.
fn write_in_order(mine: u64, dir: &Path, json: &str) {
    let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    if mine > *written {
        *written = mine;
        let _ = write_file(dir, json);
    }
}

/// Writes on a thread. A write that was asked for before one already written is dropped, so a slow thread
/// never puts older text over newer.
pub fn spawn_save(dir: PathBuf, json: String) {
    let mine = ASKED.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || write_in_order(mine, &dir, &json));
}

/// Writes now and returns when it is on disk, for the one moment a thread is too late: the app is about to restart itself
/// (the updater). It goes through the same ordering, so a thread still in flight can't write over it afterwards.
pub fn save_now(dir: &Path, json: &str) {
    write_in_order(ASKED.fetch_add(1, Ordering::SeqCst) + 1, dir, json);
}
