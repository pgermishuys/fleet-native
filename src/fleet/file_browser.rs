//! The Files tab's tree (`composables/use-file-browser.ts`, the tree half of `FileBrowserPanel.vue` and `FileBrowserTreeNode.vue`):
//! the session's root folder, the folders opened under it, what is loading, the one error, the file last clicked and the wait
//! a single click makes for a double-click. `FileBrowser` is pure (it says what to ask and takes the answer); the requests, the
//! 500 ms reload on `files.changed` and the clock are in the `State` half at the bottom.
use super::canvases::OpenFile;
use super::events::{Event, Kind};
use super::http;
use super::state::Topic;
use crate::State;
use rune::Tx;
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;

/// One double-click interval, shared by every row: a single click opens its preview tab only after this, so a second click can
/// open a kept tab instead (`DOUBLE_CLICK_MS`).
pub const DOUBLE_CLICK: f64 = 0.25;

/// The agent edits several files in one go; the root is read again once the burst has settled (`setTimeout(loadRoot, 500)`).
pub const RELOAD_WAIT: Duration = Duration::from_millis(500);

pub const NO_SESSION: &str = "No session ID provided";

/// One row of the server's answer (`BrowseDirectoryEntry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
}

impl Entry {
    pub fn file(name: &str, path: &str) -> Entry {
        Entry { name: name.into(), path: path.into(), is_directory: false }
    }

    pub fn dir(name: &str, path: &str) -> Entry {
        Entry { name: name.into(), path: path.into(), is_directory: true }
    }
}

/// `{entries: [{name, relativePath, isDirectory}]}`; no `entries` is an empty folder (`response.entries || []`).
pub fn parse_entries(body: &str) -> Vec<Entry> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    v.get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let text = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_string);
            Some(Entry {
                name: text("name")?,
                path: text("relativePath")?,
                is_directory: e.get("isDirectory").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// What an answer from the browse endpoint is: the entries, or the words `browseSessionDirectory` throws with.
pub fn browse_result(answer: Result<(u16, String), String>) -> Result<Vec<Entry>, String> {
    match answer {
        Ok((status, body)) if (200..300).contains(&status) => Ok(parse_entries(&body)),
        Ok((status, _)) => Err(format!("Failed to browse directory: {}", super::revert::reason_of(status))),
        Err(why) => Err(why),
    }
}

/// What the "Search in" switch searches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchIn {
    #[default]
    Names,
    Contents,
}

/// One drawn row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Dir {
        entry: Entry,
        depth: usize,
        open: bool,
        loading: bool,
    },
    File {
        entry: Entry,
        depth: usize,
        selected: bool,
    },
    /// "Loading…" under an open folder that is being read.
    Loading {
        depth: usize,
    },
    /// "No files" under an open folder with nothing in it.
    Empty {
        depth: usize,
    },
}

#[derive(Debug, Default)]
pub struct FileBrowser {
    /// The session the tree is of; another session starts it over.
    session: Option<String>,
    root: Vec<Entry>,
    /// The folders that are open, in the order they were opened, with what they held when read (`expandedDirs`).
    open: Vec<(String, Vec<Entry>)>,
    loading: HashSet<String>,
    root_loading: bool,
    error: Option<String>,
    /// The folders a refresh re-opens, one after another once the root is back.
    refill: Vec<String>,
    /// The file last clicked (`filesContext.selectedFilePath`).
    pub selected: Option<String>,
    /// A single click waiting out the double-click interval: when, and for which file. One for every row.
    click: Option<(f64, String)>,
    /// The reload after `files.changed`: a later event makes an earlier wait stale.
    reload: u64,
    pub search_in: SearchIn,
    /// What the search box holds (the names search is piece E's).
    pub field: rune::TextEdit,
}

impl FileBrowser {
    /// Tests make a folder that is open and being read at once, which a refresh does for a moment.
    #[cfg(test)]
    pub(crate) fn loading_for_test(&mut self, path: &str) {
        self.loading.insert(path.to_string());
    }

    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub fn root(&self) -> &[Entry] {
        &self.root
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn root_loading(&self) -> bool {
        self.root_loading
    }

    pub fn is_open(&self, path: &str) -> bool {
        self.open.iter().any(|(p, _)| p == path)
    }

    pub fn is_loading(&self, path: &str) -> bool {
        self.loading.contains(path)
    }

    /// The open folders, in the order they were opened.
    pub fn open_paths(&self) -> Vec<String> {
        self.open.iter().map(|(p, _)| p.clone()).collect()
    }

    /// The tree is for `session` (the watch on `sessionId`): another session clears everything, entries, open folders, loading and
    /// the error, and drops the waits. `true` when the root must be read.
    pub fn follow(&mut self, session: &str) -> bool {
        if self.session.as_deref() == Some(session) {
            return false;
        }
        self.session = Some(session.to_string());
        self.clear();
        self.selected = None;
        self.click = None;
        self.reload += 1;
        true
    }

    fn clear(&mut self) {
        self.root.clear();
        self.open.clear();
        self.loading.clear();
        self.root_loading = false;
        self.error = None;
        self.refill.clear();
    }

    /// `loadRoot`: the root is being read. `false` (with the error set) when there is no session.
    pub fn start_root(&mut self) -> bool {
        if self.session.is_none() {
            self.error = Some(NO_SESSION.into());
            return false;
        }
        self.root_loading = true;
        self.error = None;
        true
    }

    /// The root answered. An answer for a session that has gone is dropped.
    pub fn root_done(&mut self, session: &str, answer: Result<Vec<Entry>, String>) {
        if self.session.as_deref() != Some(session) {
            return;
        }
        match answer {
            Ok(entries) => self.root = entries,
            Err(why) => self.error = Some(why),
        }
        self.root_loading = false;
    }

    /// `expandDirectory`: whether to read the folder. A folder that is open is cached and read no more.
    pub fn expand(&mut self, path: &str) -> bool {
        if self.session.is_none() {
            self.error = Some(NO_SESSION.into());
            return false;
        }
        if self.is_open(path) {
            return false;
        }
        self.loading.insert(path.to_string());
        self.error = None;
        true
    }

    pub fn dir_done(&mut self, session: &str, path: &str, answer: Result<Vec<Entry>, String>) {
        if self.session.as_deref() != Some(session) {
            return;
        }
        self.loading.remove(path);
        match answer {
            Ok(entries) => {
                self.open.retain(|(p, _)| p != path);
                self.open.push((path.to_string(), entries));
            }
            Err(why) => self.error = Some(why),
        }
    }

    /// `collapseDirectory`: the entries are forgotten, so opening the folder again reads it again.
    pub fn collapse(&mut self, path: &str) {
        self.open.retain(|(p, _)| p != path);
    }

    /// `refresh`: everything is cleared and the root is read; the folders that were open are re-opened by [`next_refill`].
    pub fn refresh(&mut self) -> bool {
        let reopen = self.open_paths();
        self.clear();
        self.refill = reopen;
        self.start_root()
    }

    /// The next folder a refresh re-opens, once the one before it has answered.
    pub fn next_refill(&mut self) -> Option<String> {
        (!self.refill.is_empty()).then(|| self.refill.remove(0))
    }

    /// The rows of the tree: every root entry in the server's order, each open folder followed by its entries.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.rows_of(&self.root, 0, &mut rows);
        rows
    }

    fn rows_of(&self, entries: &[Entry], depth: usize, rows: &mut Vec<Row>) {
        for entry in entries {
            if !entry.is_directory {
                let selected = self.selected.as_deref() == Some(entry.path.as_str());
                rows.push(Row::File { entry: entry.clone(), depth, selected });
                continue;
            }
            let children = self.open.iter().find(|(p, _)| *p == entry.path).map(|(_, c)| c);
            let loading = self.is_loading(&entry.path);
            rows.push(Row::Dir { entry: entry.clone(), depth, open: children.is_some(), loading });
            if let Some(children) = children {
                if loading {
                    rows.push(Row::Loading { depth: depth + 1 });
                } else if children.is_empty() {
                    rows.push(Row::Empty { depth: depth + 1 });
                } else {
                    self.rows_of(children, depth + 1, rows);
                }
            }
        }
    }

    /// A click on a file: it is selected, and opens as a preview once `now + DOUBLE_CLICK` passes with no other click anywhere in
    /// the tree (the wait is shared, so a click elsewhere replaces it).
    pub fn click(&mut self, path: &str, now: f64) {
        self.selected = Some(path.to_string());
        self.click = Some((now + DOUBLE_CLICK, path.to_string()));
    }

    /// The second click of a double-click: the wait is dropped; the caller opens a kept tab.
    pub fn double_click(&mut self, path: &str) {
        self.selected = Some(path.to_string());
        self.click = None;
    }

    /// The file whose wait has ended, once.
    pub fn due_click(&mut self, now: f64) -> Option<String> {
        match &self.click {
            Some((due, _)) if now >= *due => self.click.take().map(|(_, path)| path),
            _ => None,
        }
    }

    /// When the wait for a click ends, to wake for.
    pub fn click_due(&self) -> Option<f64> {
        self.click.as_ref().map(|(due, _)| *due)
    }

    /// `files.changed` came: a wait to start, and the number that tells it from an earlier one.
    pub fn queue_reload(&mut self) -> u64 {
        self.reload += 1;
        self.reload
    }

    /// The wait ended: whether to read the root (no later event replaced it, and the session is still this one).
    pub fn reload_due(&self, session: &str, generation: u64) -> bool {
        self.session.as_deref() == Some(session) && self.reload == generation
    }
}

impl State {
    /// The session whose files the Files tab shows: the open one, or the invented one a scene draws.
    pub fn files_session(&self) -> String {
        self.canvas_session()
    }

    /// Called every frame. The tab showing for a session reads its root, and another session starts over; a click that waited out
    /// the double-click interval opens its preview.
    pub fn keep_files_tab(&mut self, now: f64, tx: &mut Tx) {
        let tabs = self.open_canvases();
        if tabs.active().is_some_and(|c| c.kind == super::canvases::CanvasKind::Files) {
            let session = self.files_session();
            if self.file_browser.follow(&session) {
                self.load_files_root(&session);
            }
        }
        if let Some(path) = self.file_browser.due_click(now) {
            let session = self.files_session();
            self.open_file(&session, &path, OpenFile::default(), tx);
        }
        if let Some(due) = self.file_browser.click_due() {
            tx.wake_after(Duration::from_secs_f64((due - now).max(0.0) + 0.005));
        }
    }

    /// Reads the root (`loadRoot`), then the folders a refresh is re-opening.
    pub fn load_files_root(&mut self, session: &str) {
        if !self.file_browser.start_root() {
            return;
        }
        self.browse(session, None);
    }

    /// A folder was pressed: open it (reading it), or close it.
    pub fn toggle_files_dir(&mut self, path: &str) {
        let session = self.files_session();
        if self.file_browser.is_open(path) {
            self.file_browser.collapse(path);
        } else if self.file_browser.expand(path) {
            self.browse(&session, Some(path.to_string()));
        }
    }

    /// The box's clear button, or Esc in it.
    pub fn clear_files_search(&mut self) {
        self.file_browser.field = rune::TextEdit::new("");
    }

    /// The toolbar's Refresh.
    pub fn refresh_files(&mut self) {
        let session = self.files_session();
        if self.file_browser.refresh() {
            self.browse(&session, None);
        }
    }

    /// A press on a file row. The second press of a double-click is `double_press_file`'s.
    pub fn press_file(&mut self, path: &str, tx: &mut Tx) {
        self.file_browser.click(path, self.clock);
        tx.wake_after(Duration::from_secs_f64(DOUBLE_CLICK + 0.005));
    }

    /// A double-click on a file: a kept tab, at once, and nothing else.
    pub fn double_press_file(&mut self, path: &str, tx: &mut Tx) {
        self.file_browser.double_click(path);
        let session = self.files_session();
        self.open_file(&session, path, OpenFile { keep: true, view: None }, tx);
    }

    /// `files.changed` on the open session's topic: the root is read again 500 ms after the last one. Only the root; the folders
    /// that are open keep what they had.
    pub fn on_file_browser_event(&mut self, topic: &Topic, event: &Event) {
        let Topic::Session(session) = topic else { return };
        if !matches!(event, Event::Other(Kind::FilesChanged, _)) || self.file_browser.session() != Some(session.as_str()) {
            return;
        }
        let generation = self.file_browser.queue_reload();
        let Some(link) = self.link_for(session) else { return };
        let id = session.clone();
        link.off_thread(
            move |_| std::thread::sleep(RELOAD_WAIT),
            move |s, _, ()| {
                if s.file_browser.reload_due(&id, generation) {
                    s.load_files_root(&id);
                }
            },
        );
    }

    /// One read of the browse endpoint. With no server (an invented session) the invented tree answers at once.
    fn browse(&mut self, session: &str, path: Option<String>) {
        let Some(link) = self.link_for(session) else {
            let answer = crate::mock::files_tab::browse(path.as_deref());
            self.browsed(session, path, answer);
            return;
        };
        let (id, asked) = (session.to_string(), path.clone());
        link.off_thread(move |base| browse_result(http::files_browse(base, &id, asked.as_deref())), {
            let id = session.to_string();
            move |s, _, answer| s.browsed(&id, path, answer)
        });
    }

    fn browsed(&mut self, session: &str, path: Option<String>, answer: Result<Vec<Entry>, String>) {
        match path {
            None => self.file_browser.root_done(session, answer),
            Some(path) => self.file_browser.dir_done(session, &path, answer),
        }
        // A refresh opens its folders one after another.
        if let Some(next) = self.file_browser.next_refill()
            && self.file_browser.expand(&next)
        {
            self.browse(session, Some(next));
        }
    }
}
