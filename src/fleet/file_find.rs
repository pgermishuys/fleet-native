//! Find in a file, the state behind the panel (CodeMirror's search panel as `search({ top: true })` opens it, rows 12.1 and 12.2 of
//! docs/parity/11-files.md): whether it is open, the field, the three boxes, every match in the open file, the match stepping has
//! made current, and the keys. CodeMirror keeps the panel per editor; here one panel follows the active file tab and shuts when the
//! tab changes. The matching is `conv::file_find`; the panel and the marks are `view::file_find`.
//!
//! Like CodeMirror, typing marks every match and moves nothing: the first Enter makes the first match current and scrolls to it. There
//! is no match count (Fleet's panel has none) and no caret yet, so stepping starts from the top, or from the bottom going back.
use super::canvases::FileView;
use super::file_buffers::Status;
use crate::State;
use crate::conv::changes_find::step_shortcut;
use crate::conv::file_find::{Match, Options, find_matches, step};
use crate::view::code_view::CODE;
use rune::{Key, KeyEvent, Tx};
use std::time::Duration;

/// The panel's field, and the element the sideways scroll to the current match aims at.
pub const FIELD: &str = "file-find-field";
pub const MARK: &str = "file-find-current";

#[derive(Debug, Default)]
pub struct FileFind {
    pub open: bool,
    /// What the field holds, with its caret and selection.
    pub field: rune::TextEdit,
    pub options: Options,
    session: String,
    path: String,
    /// The revision of the buffer the matches were worked out from.
    rev: u64,
    matches: Vec<Match>,
    current: Option<usize>,
    /// A sideways scroll to the current match is due (its line is drawn first, so it waits a frame).
    scroll_due: Option<f64>,
}

impl FileFind {
    /// The file the panel is searching.
    pub fn target(&self) -> (&str, &str) {
        (&self.session, &self.path)
    }

    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// What the marks are drawn from, for the rows that cache them: it changes when any mark would.
    pub fn stamp(&self, session: &str, path: &str) -> (bool, String, Options, Option<usize>) {
        let mine = self.open && self.target() == (session, path);
        (mine, if mine { self.field.text.clone() } else { String::new() }, self.options, if mine { self.current } else { None })
    }

    /// The matches on one line (0-based) and the index of the first of them among all, for telling which one is current.
    pub fn on_line(&self, line: usize) -> (&[Match], usize) {
        let from = self.matches.partition_point(|m| m.line < line);
        let to = from + self.matches[from..].partition_point(|m| m.line == line);
        (&self.matches[from..to], from)
    }

    /// Works the matches out again for `lines`. The current match stays when it is still a match, as the selection does in CodeMirror.
    pub fn search(&mut self, lines: &[String], rev: u64) {
        let before = self.current.and_then(|i| self.matches.get(i)).cloned();
        self.matches = find_matches(lines, &self.field.text, self.options);
        self.rev = rev;
        self.current = before.and_then(|m| self.matches.iter().position(|n| *n == m));
    }

    fn step(&mut self, direction: i32) -> Option<&Match> {
        self.current = step(self.matches.len(), self.current, direction);
        self.scroll_due = self.current.map(|_| 0.0);
        self.current.and_then(|i| self.matches.get(i))
    }
}

impl State {
    /// A scene: the panel open on the file in view with `query` typed, every match marked and `steps` Enters pressed.
    pub fn show_file_find(&mut self, query: &str, options: Options, steps: usize) {
        let Some((session, path, _)) = self.code_in_view() else { return };
        self.file_find = FileFind { open: true, session, path, field: rune::TextEdit::new(query), options, ..Default::default() };
        self.file_find_lines();
        for _ in 0..steps {
            self.file_find.step(1);
        }
    }

    /// The open file tab whose code is showing: (session, path, machine).
    fn code_in_view(&self) -> Option<(String, String, String)> {
        let tabs = self.open_canvases();
        let file = tabs.active().and_then(|c| c.file.clone())?;
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let ready = self.file_buffers.info(&machine, &session, &file.path).is_some_and(|i| i.status == Status::Ready);
        (ready && file.view != FileView::Rendered && !super::canvases::is_image_path(&file.path)).then_some((session, file.path, machine))
    }

    /// Whether the panel is drawn over the file in view.
    pub fn file_find_shows(&self) -> bool {
        self.file_find.open && self.code_in_view().is_some_and(|(s, p, _)| self.file_find.target() == (s.as_str(), p.as_str()))
    }

    fn file_find_lines(&mut self) {
        let Some((session, path, machine)) = self.code_in_view() else { return };
        let Some(buffer) = self.file_buffers.buffer(&machine, &session, &path) else { return };
        let rev = buffer.rev();
        let lines = buffer.lines().to_vec();
        self.file_find.search(&lines, rev);
    }

    /// Ctrl/Cmd+F: the panel opens over the file in view and the field takes focus with its text selected.
    pub fn file_find_open_ui(&mut self, tx: &mut Tx) {
        let Some((session, path, _)) = self.code_in_view() else { return };
        if !self.file_find_shows() {
            self.file_find.session = session;
            self.file_find.path = path;
            self.file_find.open = true;
            self.file_find.current = None;
            self.file_find_lines();
        }
        let end = self.file_find.field.text.len();
        self.file_find.field.set_selection(0, end);
        tx.focus(FIELD, false);
    }

    /// Esc or the close button: the panel and its marks go, and the keyboard goes back to the code.
    pub fn file_find_close_ui(&mut self, tx: &mut Tx) {
        if !self.file_find.open {
            return;
        }
        self.file_find.open = false;
        self.file_find.current = None;
        self.file_find.scroll_due = None;
        tx.focus(CODE, false);
    }

    /// The field's text or a box changed.
    pub fn file_find_typed(&mut self) {
        self.file_find_lines();
    }

    pub fn file_find_toggle(&mut self, which: fn(&mut Options) -> &mut bool) {
        let o = &mut self.file_find.options;
        *which(o) = !*which(o);
        self.file_find_lines();
    }

    /// Enter, Shift+Enter, F3, Ctrl/Cmd+G and the buttons: the next or previous match becomes current, wrapping, and scrolls into view.
    pub fn file_find_step(&mut self, direction: i32, tx: &mut Tx) {
        let Some(line) = self.file_find.step(direction).map(|m| m.line + 1) else { return };
        let wrapped = self.sidebar.file_wrap(&self.file_find.path);
        tx.scroll_center_in_instant(CODE, format!("code-line-{line}{}", if wrapped { "w" } else { "" }));
        tx.wake_after(Duration::from_millis(16));
    }

    /// Whether focus is in the file's code or its panel (an unfocused window is in neither).
    fn in_file_code(focused: Option<&str>) -> bool {
        focused.is_some_and(|k| k == CODE || k.starts_with("code-") || k.starts_with("file-find"))
    }

    /// The window's key, before the app's and the conversation's: Ctrl/Cmd+F opens the panel (or selects its field), F3 and Ctrl/Cmd+G
    /// step while it is open and Esc closes it, all only while focus is in the file's code, so the conversation's find and the agent's
    /// Esc are left alone there.
    pub fn file_find_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !Self::in_file_code(tx.focused()) || self.code_in_view().is_none() {
            return false;
        }
        let apple = cfg!(target_os = "macos");
        if crate::conv::find::is_find_shortcut(ev, apple) {
            self.file_find_open_ui(tx);
            return true;
        }
        if !self.file_find_shows() {
            return false;
        }
        let direction = step_shortcut(ev, apple);
        if direction != 0 {
            self.file_find_step(direction, tx);
            true
        } else if ev.key == Key::Escape {
            self.file_find_close_ui(tx);
            true
        } else {
            false
        }
    }

    /// From `poll`: the panel shuts when its file is no longer the one in view, the matches are worked out again when the file was read
    /// again, and the current match is centred sideways once its line is drawn. True when the view must redraw.
    pub fn keep_file_find(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.file_find.open {
            return false;
        }
        if !self.file_find_shows() {
            self.file_find.open = false;
            self.file_find.current = None;
            return true;
        }
        let mut changed = false;
        let (session, path) = (self.file_find.session.clone(), self.file_find.path.clone());
        let machine = self.machine_key_of(&session).to_string();
        if self.file_buffers.buffer(&machine, &session, &path).is_some_and(|b| b.rev() != self.file_find.rev) {
            self.file_find_lines();
            changed = true;
        }
        match self.file_find.scroll_due {
            Some(0.0) => {
                self.file_find.scroll_due = Some(now + 0.016);
                tx.wake_after(Duration::from_millis(16));
            }
            Some(due) if now >= due => {
                self.file_find.scroll_due = None;
                tx.scroll_center(MARK);
                changed = true;
            }
            Some(_) => tx.wake_after(Duration::from_millis(16)),
            None => {}
        }
        changed
    }
}
