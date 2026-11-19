//! What the window remembers between runs: `stores/sidebar.ts`, whose browser `localStorage` keys become one small
//! file in the app's own data folder (`composer::drafts::app_dir`, never Fleet's `.weave`; the format is not shared
//! with the Electron app). Which groups of the session list are folded, whether the list is hidden, and whether the
//! right panel is: per session, with the last choice anywhere as the default for a session never toggled.
//!
//! The data and its rules are plain Rust. Reading and writing the file is blocking: a thread does it
//! (`State::adopt_settings`, `State::save_settings`), never a view or a handler.
pub mod anchor;
pub mod appearance;
pub mod browser;
pub mod code_themes;
pub mod dream_nightly;
pub mod dream_nightly_actions;
pub mod dreaming;
pub mod dreaming_actions;
pub mod harness_install;
pub mod harness_path;
pub mod harness_profiles;
pub mod harness_sign_in;
pub mod harness_update;
pub mod harness_weave;
pub mod harnesses;
pub mod install_target;
pub mod locations;
pub mod locations_actions;
pub mod machine_scope;
pub mod memory;
pub mod memory_actions;
pub mod memory_events;
pub mod memory_notes;
pub mod naming_actions;
pub mod naming_draft;
pub mod permissions;
pub mod permissions_more;
pub mod plugins;
pub mod skills;
pub mod skills_actions;
pub mod skills_catalog;
pub mod skills_install;
pub mod storage;
pub mod storage_actions;
pub mod switches;
pub mod updates_page;

use super::new_session::defaults::{DEFAULTS_KEY, NewSessionDefaults, defaults_key};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub const FILE: &str = "settings.json";

/// The machine a Fleet is on when it is this one (`HOME_MACHINE_KEY`). Component 17 gives the others a key.
pub const HOME_MACHINE_KEY: &str = "home";

/// Oldest per-session choices drop off past this, so the map doesn't grow with every session ever opened.
const RIGHT_BY_SESSION_LIMIT: usize = 200;

/// A machine's group in the session list, as the folded-groups map keys it (`machineGroupKey`).
pub fn machine_group_key(machine: &str) -> String {
    format!("machine:{machine}")
}

/// A project's group under a machine; project ids are only unique within their machine (`projectGroupKey`).
/// The Pinned group uses the id `pinned`.
pub fn project_group_key(machine: &str, project: &str) -> String {
    format!("project:{machine}:{project}")
}

#[derive(Debug, Default, Clone)]
pub struct Sidebar {
    /// Only folded groups are kept: an open group has no entry.
    collapsed_groups: BTreeSet<String>,
    left_collapsed: bool,
    /// The last open/close choice of the right panel anywhere; `None` until one is made (Fleet reads that as open).
    last_right: Option<bool>,
    /// Each session's own choice, oldest first.
    right_by_session: Vec<(String, bool)>,
    /// Sessions seen with nothing to show, and those that then got their first change (not kept across runs).
    empty_sessions: HashSet<String>,
    opened_by_change: HashSet<String>,
    lists: HashSet<u64>,
    next_list: u64,
    /// The New session page's remembered choices, by machine key (component 7; `weave:new-session:defaults[:key]`).
    new_session: BTreeMap<String, NewSessionDefaults>,
    /// The default harness last seen on each other machine, shown at once on the next visit (`saved-per-machine`).
    default_harness: BTreeMap<String, String>,
    /// The last repository scan of each machine, so the box has repositories before the machine answers
    /// (`weave:saved:repositories:{machineKey}`).
    saved_repositories: BTreeMap<String, super::new_session::feeds::Scan>,
    /// Ids of the notices that have been a card, oldest first (`weave:notices-shown`).
    notices_shown: Vec<String>,
    /// The Appearance page's choices (`weave:theme`, `weave:code-theme`, `weave:font-size`, `weave:thinking`).
    appearance: appearance::Appearance,
    /// The harness picked for "Dream now", one for the install, not per machine (`weave:dreaming-harness`).
    dreaming_harness: Option<String>,
    /// Widen, for every session (`weave:canvas-widened`).
    canvas_widened: bool,
    /// Wrap long lines in the Changes review (`weave:changes-wrap`).
    changes_wrap: bool,
    /// Files whose code view doesn't wrap, the latest 200 (`fleet.editor.wrap`).
    file_wrap: super::wrap_pref::WrapPrefs,
    /// Sessions whose terminal drawer is open, and the drawer's height (`weave:terminal-open`, `weave:terminal-height`).
    terminal_open: Vec<String>,
    terminal_height: Option<u32>,
    /// The layout picked in the Changes review, `None` until one is (`weave:changes-layout`).
    changes_layout: Option<super::review::Layout>,
    /// The version each machine last reported to this device (`fleet.machineVersions`), for one that stops answering.
    machine_versions: super::updates::LastSeen,
    /// The Fleet version this device last ran, by source (`weave:last-version:<source>`), so an update says what's new once.
    last_versions: super::updates::LastSeen,
    dir: Option<PathBuf>,
    dirty: bool,
}

/// What `register_list` gives back; pass it to `release_list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListToken(u64);

impl Sidebar {
    /// Reads what `to_json` wrote. A file that isn't JSON, or isn't an object, is an empty store, and a value of the
    /// wrong kind is dropped on its own (`readStoredBooleanMap`).
    pub fn parse(json: &str) -> Sidebar {
        let mut s = Sidebar::default();
        let Ok(Value::Object(root)) = serde_json::from_str::<Value>(json) else { return s };
        if let Some(Value::Object(groups)) = root.get("collapsedGroups") {
            s.collapsed_groups = groups.iter().filter(|(_, v)| **v == Value::Bool(true)).map(|(k, _)| k.clone()).collect();
        }
        s.left_collapsed = root.get("leftCollapsed") == Some(&Value::Bool(true));
        s.last_right = root.get("rightCollapsed").and_then(Value::as_bool);
        if let Some(Value::Array(pairs)) = root.get("rightCollapsedBySession") {
            for pair in pairs {
                if let Some([Value::String(id), Value::Bool(c)]) = pair.as_array().map(Vec::as_slice) {
                    s.remember_right(id, *c);
                }
            }
        }
        let prefix = format!("{DEFAULTS_KEY}:");
        if let Some(Value::Object(by_key)) = root.get("newSessionDefaults") {
            for (key, v) in by_key {
                let machine = if key == DEFAULTS_KEY { Some(HOME_MACHINE_KEY) } else { key.strip_prefix(&prefix) };
                if let Some(machine) = machine.filter(|m| !m.is_empty()) {
                    s.new_session.insert(machine.to_string(), NewSessionDefaults::parse(v));
                }
            }
        }
        if let Some(Value::Object(by_machine)) = root.get("defaultHarness") {
            s.default_harness = by_machine.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
        }
        if let Some(Value::Object(by_machine)) = root.get("savedRepositories") {
            s.saved_repositories = by_machine.iter().map(|(k, v)| (k.clone(), super::new_session::feeds::Scan::from_json(v))).collect();
        }
        if let Some(Value::Array(ids)) = root.get("noticesShown") {
            s.notices_shown = ids.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        }
        s.appearance = appearance::Appearance::parse(&root);
        if let Some(map) = root.get("machineVersions") {
            s.machine_versions = super::updates::LastSeen::parse(&map.to_string());
        }
        if let Some(map) = root.get("lastVersions") {
            s.last_versions = super::updates::LastSeen::parse(&map.to_string());
        }
        s.canvas_widened = root.get("canvasWidened") == Some(&Value::Bool(true));
        s.changes_wrap = root.get("changesWrap") == Some(&Value::Bool(true));
        s.file_wrap = super::wrap_pref::WrapPrefs::parse(root.get("fileWrapOff"));
        if let Some(Value::Array(ids)) = root.get("terminalOpen") {
            s.terminal_open = ids.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        }
        s.terminal_height = root.get("terminalHeight").and_then(Value::as_u64).map(|h| h as u32);
        s.changes_layout = match root.get("changesLayout").and_then(Value::as_str) {
            Some("split") => Some(super::review::Layout::Split),
            Some("unified") => Some(super::review::Layout::Unified),
            _ => None,
        };
        s.dreaming_harness = root.get("dreamingHarness").and_then(Value::as_str).filter(|h| !h.is_empty()).map(str::to_string);
        s
    }

    pub fn to_json(&self) -> String {
        let groups: serde_json::Map<String, Value> = self.collapsed_groups.iter().map(|k| (k.clone(), Value::Bool(true))).collect();
        let by_session: Vec<Value> = self.right_by_session.iter().map(|(id, c)| json!([id, c])).collect();
        let mut root = json!({ "collapsedGroups": groups, "leftCollapsed": self.left_collapsed, "rightCollapsedBySession": by_session });
        if let Some(c) = self.last_right {
            root["rightCollapsed"] = Value::Bool(c);
        }
        if !self.new_session.is_empty() {
            root["newSessionDefaults"] = self.new_session.iter().map(|(m, d)| (defaults_key(m), d.to_json())).collect();
        }
        if !self.default_harness.is_empty() {
            root["defaultHarness"] = json!(self.default_harness);
        }
        if !self.saved_repositories.is_empty() {
            root["savedRepositories"] = self.saved_repositories.iter().map(|(m, scan)| (m.clone(), scan.to_json())).collect();
        }
        if !self.notices_shown.is_empty() {
            root["noticesShown"] = json!(self.notices_shown);
        }
        self.appearance.write(&mut root);
        if self.canvas_widened {
            root["canvasWidened"] = Value::Bool(true);
        }
        if self.changes_wrap {
            root["changesWrap"] = Value::Bool(true);
        }
        if let Some(v) = self.file_wrap.to_json() {
            root["fileWrapOff"] = v;
        }
        if !self.terminal_open.is_empty() {
            root["terminalOpen"] = json!(self.terminal_open);
        }
        if let Some(h) = self.terminal_height {
            root["terminalHeight"] = json!(h);
        }
        if let Some(l) = self.changes_layout {
            root["changesLayout"] = json!(if l == super::review::Layout::Split { "split" } else { "unified" });
        }
        if let Some(h) = &self.dreaming_harness {
            root["dreamingHarness"] = json!(h);
        }
        if !self.machine_versions.0.is_empty() {
            root["machineVersions"] = serde_json::from_str(&self.machine_versions.to_json()).unwrap_or_default();
        }
        if !self.last_versions.0.is_empty() {
            root["lastVersions"] = serde_json::from_str(&self.last_versions.to_json()).unwrap_or_default();
        }
        serde_json::to_string_pretty(&root).unwrap_or_default()
    }

    // ── folded groups ─────────────────────────────────────────────────────────

    pub fn is_group_collapsed(&self, key: &str) -> bool {
        self.collapsed_groups.contains(key)
    }

    pub fn set_group_collapsed(&mut self, key: &str, collapsed: bool) {
        if self.is_group_collapsed(key) == collapsed {
            return;
        }
        if collapsed {
            self.collapsed_groups.insert(key.to_string());
        } else {
            self.collapsed_groups.remove(key);
        }
        self.dirty = true;
    }

    pub fn toggle_group_collapsed(&mut self, key: &str) {
        self.set_group_collapsed(key, !self.is_group_collapsed(key));
    }

    // ── the list and the right panel ──────────────────────────────────────────

    pub fn panel_collapsed(&self) -> bool {
        self.left_collapsed
    }

    pub fn set_panel_collapsed(&mut self, collapsed: bool) {
        self.left_collapsed = collapsed;
        self.dirty = true;
    }

    /// Whether the right panel is folded away for `session` (`None` is the board's, which has no session): the session's
    /// own choice, else folded while it has nothing to show, else open once it first changed, else the last choice
    /// anywhere (open when none was made).
    pub fn right_collapsed(&self, session: Option<&str>) -> bool {
        if let Some(id) = session {
            if let Some(chosen) = self.right_choice(id) {
                return chosen;
            }
            if self.empty_sessions.contains(id) {
                return true;
            }
            if self.opened_by_change.contains(id) {
                return false;
            }
        }
        self.last_right.unwrap_or(false)
    }

    /// What was chosen for `session` itself, or else the last choice anywhere, if there ever was one. The native panel
    /// starts closed, so it only follows a choice that was made (component 11 decides the rest).
    pub fn right_choice(&self, session: &str) -> Option<bool> {
        self.right_by_session.iter().find(|(id, _)| id == session).map(|(_, c)| *c)
    }

    pub fn remembered_right(&self, session: &str) -> Option<bool> {
        let by_content = if self.empty_sessions.contains(session) {
            Some(true)
        } else if self.opened_by_change.contains(session) {
            Some(false)
        } else {
            None
        };
        self.right_choice(session).or(by_content).or(self.last_right)
    }

    /// The user opened or closed the right panel. The last choice anywhere always changes; the session's own only when a
    /// session is open (the board's panel keeps out of the session choices).
    pub fn set_right_collapsed(&mut self, session: Option<&str>, collapsed: bool) {
        self.last_right = Some(collapsed);
        if let Some(id) = session {
            self.remember_right(id, collapsed);
        }
        self.dirty = true;
    }

    /// Re-inserting moves the session to the newest end, so the limit drops the least recently toggled.
    fn remember_right(&mut self, id: &str, collapsed: bool) {
        self.right_by_session.retain(|(s, _)| s != id);
        self.right_by_session.push((id.to_string(), collapsed));
        let extra = self.right_by_session.len().saturating_sub(RIGHT_BY_SESSION_LIMIT);
        self.right_by_session.drain(..extra);
    }

    /// What the open session has to show in the panel: nothing yet (it stays folded), or its first change (`noteSessionContent`).
    pub fn note_session_content(&mut self, session: &str, has_content: bool) {
        let empty = self.empty_sessions.contains(session);
        if !has_content && !empty {
            self.empty_sessions.insert(session.to_string());
        } else if has_content && empty {
            self.empty_sessions.remove(session);
            self.opened_by_change.insert(session.to_string());
        }
    }

    // ── the New session page ──────────────────────────────────────────────────

    /// What the page remembers for `machine` (a machine key); nothing the first time.
    pub fn new_session_defaults(&self, machine: &str) -> NewSessionDefaults {
        self.new_session.get(machine).cloned().unwrap_or_default()
    }

    /// Changes `machine`'s remembered choices; the file is written at the next save.
    pub fn update_new_session_defaults(&mut self, machine: &str, change: impl FnOnce(&mut NewSessionDefaults)) {
        change(self.new_session.entry(machine.to_string()).or_default());
        self.dirty = true;
    }

    /// The notice ids shown as cards before, oldest first.
    pub fn notices_shown(&self) -> &[String] {
        &self.notices_shown
    }

    /// Keeps the notice ids shown so far (the last [`super::notices::SHOWN_LIMIT`]).
    pub fn save_notices_shown(&mut self, ids: &[String]) {
        if self.notices_shown != ids {
            self.notices_shown = ids.to_vec();
            self.dirty = true;
        }
    }

    /// The versions machines last reported to this device.
    pub fn machine_versions(&self) -> &super::updates::LastSeen {
        &self.machine_versions
    }

    /// Keeps the versions machines reported.
    pub fn save_machine_versions(&mut self, seen: &super::updates::LastSeen) {
        if self.machine_versions != *seen {
            self.machine_versions = seen.clone();
            self.dirty = true;
        }
    }

    /// The version this device last ran under `source` (`readLastVersion`).
    pub fn last_run_version(&self, source: &str) -> Option<&str> {
        self.last_versions.get(source)
    }

    /// Keeps the version this device runs now (`writeLastVersion`); true when it changed and the file should be written.
    pub fn save_last_run_version(&mut self, source: &str, version: &str) -> bool {
        let changed = self.last_versions.note([(source, version)]);
        self.dirty |= changed;
        changed
    }

    /// The default harness another machine last said it has (`readSaved("default-harness", key)`).
    pub fn saved_default_harness(&self, machine: &str) -> Option<&str> {
        self.default_harness.get(machine).map(String::as_str)
    }

    /// Keeps what a machine said (`writeSaved`); `None` forgets it.
    pub fn save_default_harness(&mut self, machine: &str, harness: Option<&str>) {
        let changed = match harness {
            Some(h) => self.default_harness.insert(machine.to_string(), h.to_string()).as_deref() != Some(h),
            None => self.default_harness.remove(machine).is_some(),
        };
        self.dirty |= changed;
    }

    /// Whether the right panel was left widened (`weave:canvas-widened`).
    pub fn canvas_widened(&self) -> bool {
        self.canvas_widened
    }

    /// Whether a file's code view wraps (`wrapFor`).
    pub fn file_wrap(&self, path: &str) -> bool {
        self.file_wrap.wrap_for(path)
    }

    pub fn save_file_wrap(&mut self, path: &str, wrap: bool) {
        if self.file_wrap.set_wrap_for(path, wrap) {
            self.dirty = true;
        }
    }

    pub fn changes_wrap(&self) -> bool {
        self.changes_wrap
    }

    pub fn terminal_open(&self) -> &[String] {
        &self.terminal_open
    }

    pub fn terminal_height(&self) -> Option<u32> {
        self.terminal_height
    }

    pub fn save_terminals(&mut self, open: Vec<String>, height: u32) {
        let height = Some(height);
        if self.terminal_open != open || self.terminal_height != height {
            self.terminal_open = open;
            self.terminal_height = height;
            self.dirty = true;
        }
    }

    pub fn changes_layout(&self) -> Option<super::review::Layout> {
        self.changes_layout
    }

    pub fn save_changes_layout(&mut self, layout: super::review::Layout) {
        if self.changes_layout != Some(layout) {
            self.changes_layout = Some(layout);
            self.dirty = true;
        }
    }

    pub fn save_changes_wrap(&mut self, wrap: bool) {
        if self.changes_wrap != wrap {
            self.changes_wrap = wrap;
            self.dirty = true;
        }
    }

    pub fn save_canvas_widened(&mut self, widened: bool) {
        if self.canvas_widened != widened {
            self.canvas_widened = widened;
            self.dirty = true;
        }
    }

    /// The harness last picked for "Dream now".
    pub fn dreaming_harness(&self) -> Option<&str> {
        self.dreaming_harness.as_deref()
    }

    pub fn save_dreaming_harness(&mut self, harness: &str) {
        if self.dreaming_harness.as_deref() != Some(harness) {
            self.dreaming_harness = Some(harness.to_string());
            self.dirty = true;
        }
    }

    /// The repositories a machine last listed (`readSaved("repositories", key)`); none the first time.
    pub fn saved_repositories(&self) -> impl Iterator<Item = (&String, &super::new_session::feeds::Scan)> {
        self.saved_repositories.iter()
    }

    /// Keeps a machine's scan (`writeSaved`).
    pub fn save_repositories(&mut self, machine: &str, scan: &super::new_session::feeds::Scan) {
        if self.saved_repositories.get(machine) != Some(scan) {
            self.saved_repositories.insert(machine.to_string(), scan.clone());
            self.dirty = true;
        }
    }

    // ── Appearance ────────────────────────────────────────────────────────────

    pub fn appearance(&self) -> appearance::Appearance {
        self.appearance
    }

    pub fn update_appearance(&mut self, change: impl FnOnce(&mut appearance::Appearance)) {
        let before = self.appearance;
        change(&mut self.appearance);
        self.dirty |= self.appearance != before;
    }

    // ── lists on screen ───────────────────────────────────────────────────────

    /// A session list was put on screen (`registerSessionList`).
    pub fn register_list(&mut self) -> ListToken {
        self.next_list += 1;
        self.lists.insert(self.next_list);
        ListToken(self.next_list)
    }

    /// Takes the list off again; a second release of the same one does nothing.
    pub fn release_list(&mut self, token: ListToken) {
        self.lists.remove(&token.0);
    }

    pub fn list_shown(&self) -> bool {
        !self.lists.is_empty()
    }

    // ── the file ──────────────────────────────────────────────────────────────

    /// The file was read: what it says replaces what is here, unless something was changed before it arrived.
    pub fn adopt(&mut self, dir: PathBuf, stored: Sidebar) {
        if self.dirty {
            self.dir = Some(dir);
            return;
        }
        *self = Sidebar { dir: Some(dir), lists: std::mem::take(&mut self.lists), next_list: self.next_list, ..stored };
    }

    /// Where the file goes and what it holds, if something changed since the last write.
    pub fn take_save(&mut self) -> Option<(PathBuf, String)> {
        let dir = self.dir.clone()?;
        std::mem::take(&mut self.dirty).then(|| (dir, self.to_json()))
    }
}

/// Reads the file in `dir`; no file, or one that can't be read, is an empty store. Blocking.
pub fn load(dir: &Path) -> Sidebar {
    std::fs::read_to_string(dir.join(FILE)).map(|json| Sidebar::parse(&json)).unwrap_or_default()
}

/// Writes the file to a temporary name first, then renames it over, so a crash leaves the old one whole. Blocking.
pub fn write_file(dir: &Path, json: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, dir.join(FILE))
}

static ASKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The last write asked for, per folder: a newer write to another folder must not make an older one to this folder look stale.
static WRITTEN: std::sync::Mutex<Option<std::collections::HashMap<PathBuf, u64>>> = std::sync::Mutex::new(None);

pub(crate) fn write_in_order(mine: u64, dir: &Path, json: &str) {
    let mut guard = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    let written = guard.get_or_insert_with(Default::default).entry(dir.to_path_buf()).or_insert(0);
    if mine > *written {
        *written = mine;
        let _ = write_file(dir, json);
    }
}

/// Writes on a thread. A write asked for before one already written is dropped, so a slow thread never puts an older
/// state over a newer one. A failure is dropped too: the choices stay for this run.
pub fn spawn_save(dir: PathBuf, json: String) {
    let mine = ASKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    std::thread::spawn(move || write_in_order(mine, &dir, &json));
}

/// Writes now, in the same order, for the moment the app restarts itself (the updater).
pub fn save_now(dir: &Path, json: &str) {
    write_in_order(ASKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1, dir, json);
}

#[cfg(test)]
impl Sidebar {
    pub(crate) fn collapsed_is_empty(&self) -> bool {
        self.collapsed_groups.is_empty()
    }
}
