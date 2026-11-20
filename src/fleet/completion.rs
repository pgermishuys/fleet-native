//! The completion popup's state and keys (component 11, piece U; row 13.11 of docs/parity/11-files.md; CodeMirror's `autocompletion()`
//! with Fleet's sources in `lib/code-editor/completion.ts`). While you type, words of the file and, in an import path, the folder's
//! files are offered under the caret; Down and Up choose, Enter or Tab take one, Esc shuts it, Ctrl/Cmd+Space opens it by hand.
//! What is offered is `conv::completion`; the popup is `view::completion`.
//!
//! A folder's listing is the session's `find/files?q=<folder>`, asked on a thread and kept for 10 seconds (`folderLister`).
use super::code_edit::Pos;
use super::code_editing::FileKey;
use super::finder::parse_files;
use super::http;
use crate::State;
use crate::conv::completion::{ImportAt, Ranked, import_at, import_items, rank, word_start, words};
use rune::{Key, KeyEvent, Tx};
use std::collections::HashMap;

/// How long a folder's listing is kept (`FOLDER_CACHE_MS`), in seconds.
const FOLDER_CACHE: f64 = 10.0;

#[derive(Debug, Default)]
pub struct Completion {
    /// The popup is showing.
    pub open: bool,
    /// Where the text it replaces starts (it ends at the caret).
    pub from: Pos,
    /// The suggestions, best first, and the chosen one.
    pub items: Vec<Ranked>,
    pub selected: usize,
    /// Folder listings by (machine, session, folder): when they were read, and what they hold.
    cache: HashMap<(String, String, String), (f64, Vec<String>)>,
    /// Listings asked for and not yet answered.
    pending: Vec<(String, String, String)>,
    /// How many times a folder was asked of the server (for tests).
    pub asked: usize,
}

impl Completion {
    pub fn close(&mut self) {
        self.open = false;
        self.items.clear();
        self.selected = 0;
    }

    /// The chosen suggestion's text.
    pub fn chosen(&self) -> Option<&Ranked> {
        self.items.get(self.selected)
    }
}

impl State {
    /// The file under the editor: (key, path).
    fn completion_file(&self) -> Option<FileKey> {
        self.code_edit.file.clone().filter(|_| self.code_edit_target().is_some())
    }

    /// Works out what to offer at the caret and opens, updates or shuts the popup. `explicit` is Ctrl/Cmd+Space: it opens with nothing
    /// typed yet.
    pub fn complete_refresh(&mut self, explicit: bool) {
        let Some((machine, session, path)) = self.completion_file() else { return self.completion.close() };
        let Some(buffer) = self.file_buffers.buffer(&machine, &session, &path) else { return self.completion.close() };
        let editor = &buffer.editor;
        let lines = buffer.lines();
        let caret = editor.caret;
        if editor.has_selection() || editor.preedit.is_some() || lines.get(caret.line).is_none() {
            return self.completion.close();
        }
        let line = &lines[caret.line];
        let before = &line[..caret.byte.min(line.len())];
        let (from, typed, items) = if let Some(ImportAt { folder, segment }) = import_at(&path, before) {
            let key = (machine.clone(), session.clone(), folder.clone());
            match self.completion.cache.get(&key) {
                Some((at, entries)) if self.clock - at < FOLDER_CACHE => {
                    let mut items = import_items(&path, &folder, entries);
                    let typed = before[before.len() - segment..].to_string();
                    // The words of the file are offered here too, as CodeMirror merges its sources.
                    if !typed.is_empty() {
                        items.extend(words(lines, caret.line, &typed));
                    }
                    (Pos::new(caret.line, caret.byte - segment), typed, items)
                }
                _ => {
                    self.completion.close();
                    return self.ask_folder(key);
                }
            }
        } else {
            let start = word_start(line, caret.byte);
            let typed = line[start..caret.byte].to_string();
            if typed.is_empty() && !explicit {
                return self.completion.close();
            }
            (Pos::new(caret.line, start), typed.clone(), words(lines, caret.line, &typed))
        };
        let ranked = rank(items, &typed);
        if ranked.is_empty() {
            return self.completion.close();
        }
        // The same list keeps the choice the person made with the arrows while they type on.
        let keep = self.completion.chosen().map(|r| r.item.label.clone());
        self.completion.selected = keep.and_then(|k| ranked.iter().position(|r| r.item.label == k)).unwrap_or(0);
        self.completion.items = ranked;
        self.completion.from = from;
        self.completion.open = true;
    }

    /// Reads a folder's files from the session's Fleet, or from the invented project in a scene.
    fn ask_folder(&mut self, key: (String, String, String)) {
        if self.completion.pending.contains(&key) {
            return;
        }
        let (_, session, folder) = key.clone();
        let Some(link) = self.link_for(&session) else {
            // An invented session: the invented project answers at once.
            let files = crate::mock::completion::folder(&folder);
            self.completion.cache.insert(key, (self.clock, files));
            return self.complete_refresh(false);
        };
        self.completion.pending.push(key.clone());
        self.completion.asked += 1;
        link.off_thread(
            move |base| match http::find_files(base, &session, &folder) {
                Ok((status, body)) if (200..300).contains(&status) => Some(parse_files(&body)),
                _ => None,
            },
            move |s, _tx, files| {
                s.completion.pending.retain(|k| *k != key);
                match files {
                    Some(files) => {
                        s.completion.cache.insert(key, (s.clock, files));
                        // Only while the caret is still in that import path.
                        s.complete_refresh(false);
                    }
                    None => {
                        s.completion.cache.remove(&key);
                    }
                }
            },
        );
    }

    /// Puts the chosen suggestion in place of what was typed.
    pub fn complete_accept(&mut self, tx: &mut Tx) {
        let Some(label) = self.completion.chosen().map(|r| r.item.label.clone()) else { return };
        let (from, now) = (self.completion.from, self.clock);
        self.completion.close();
        self.with_editor(|e, l| {
            let caret = e.caret;
            e.set_caret(from, false);
            e.set_caret(caret, true);
            e.paste(l, &label, now);
            ((), true)
        });
        self.reveal_caret(tx);
    }

    /// A key while the popup is showing: the arrows choose, Enter and Tab take, Esc shuts. False for any other key.
    pub fn complete_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !self.completion.open || ev.mods.alt || ev.mods.cmd || ev.mods.ctrl {
            return false;
        }
        let n = self.completion.items.len();
        match ev.key {
            Key::Down | Key::Up if !ev.mods.shift => {
                let step = if ev.key == Key::Down { 1 } else { n - 1 };
                self.completion.selected = (self.completion.selected + step) % n;
                self.code_edit.stamp += 1;
                tx.scroll_into_view("completion-chosen");
                true
            }
            Key::Enter | Key::Tab if !ev.mods.shift => {
                self.complete_accept(tx);
                true
            }
            Key::Escape => {
                self.completion.close();
                true
            }
            _ => false,
        }
    }
}
