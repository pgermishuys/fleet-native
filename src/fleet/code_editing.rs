//! The editor spike in the app (component 11, piece P): the code view of the open file tab takes the keyboard, the pointer, pastes and
//! the input method, and edits the file's lines through the buffer's `code_edit::Editor`. Off unless `FLEET_NATIVE_EDITOR_SPIKE=1` (or a scene
//! turns it on): nothing saves yet (piece R), so the app people run stays a viewer until slice 2 is built on it.
use super::code_edit::{Editor, Move, Pos};
use super::file_buffers::Status;
use crate::State;
use crate::view::code_view::{CODE, LINE_H};
use rune::{Ime, Key, KeyEvent, Tx};

/// The file the editor is over: (machine, session, path).
pub type FileKey = (String, String, String);

#[derive(Debug, Default)]
pub struct CodeEditing {
    /// Editing is on: the spike's switch.
    pub on: bool,
    /// The file the keyboard edits: its caret and history are the buffer's own (`Buffer::editor`).
    pub file: Option<FileKey>,
    /// The code view's height as last laid out, for Page Up and Down.
    pub view_h: f32,
    /// Moves with every change of the caret or selection, so the rows on screen are built again.
    pub stamp: u64,
    /// The code view has the keyboard: the caret (and with it the input method) is drawn only then.
    pub focused: bool,
    /// The width the caret's wrapped line was laid out at, to find which row of it the caret is on.
    pub text_w: f32,
    /// The other way the spike compares: the file in one of Rune's `text_input`s (`view::code_input_spike`), for scenes and the bench.
    pub input: Option<(FileKey, rune::TextEdit)>,
    /// The element a scene or the bench asks to take the keyboard on the next poll, with the caret then brought into view.
    pub take_focus: Option<&'static str>,
}

/// Whether the spike is asked for in the environment.
pub fn asked() -> bool {
    std::env::var("FLEET_NATIVE_EDITOR_SPIKE").is_ok_and(|v| v == "1")
}

impl CodeEditing {
    /// Makes `file` the one being edited.
    pub fn for_file(&mut self, file: &FileKey) {
        if self.file.as_ref() != Some(file) {
            self.file = Some(file.clone());
        }
    }

    /// Whether the editor is on and over this file.
    pub fn is_over(&self, machine: &str, session: &str, path: &str) -> bool {
        self.on && self.file.as_ref().is_some_and(|(m, s, p)| m == machine && s == session && p == path)
    }
}

/// The row key of a line in the code view (as its `virtual_rows` key closure names it).
pub fn row_key(line: usize, wrap: bool) -> String {
    format!("code-line-{}{}", line + 1, if wrap { "w" } else { "" })
}

impl State {
    /// The open file tab's file when it shows its code, ready, in Edit.
    pub fn code_edit_target(&self) -> Option<FileKey> {
        if !self.code_edit.on {
            return None;
        }
        let tabs = self.open_canvases();
        let file = tabs.active().and_then(|c| c.file.clone())?;
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let info = self.file_buffers.info(&machine, &session, &file.path)?;
        let has_changes = self.diffs.by_file(&file.path).is_some() || info.dirty;
        let shown = crate::view::file_tab::shown_view(file.view, has_changes);
        (info.status == Status::Ready && shown == super::canvases::FileView::Edit).then_some((machine, session, file.path))
    }

    /// The editor of the file being edited: the buffer's own.
    pub fn current_editor(&self) -> Option<&Editor> {
        let (m, s, p) = self.code_edit.file.as_ref()?;
        Some(&self.file_buffers.buffer(m, s, p)?.editor)
    }

    /// Runs `f` on the editor and the lines of the file it edits; the buffer's revision moves when `f` says it changed the text.
    pub(crate) fn with_editor<R>(&mut self, f: impl FnOnce(&mut Editor, &mut Vec<String>) -> (R, bool)) -> Option<R> {
        let file = self.code_edit_target()?;
        self.code_edit.for_file(&file);
        let (m, s, p) = &file;
        let out = self.file_buffers.edit(m, s, p, f);
        self.code_edit.stamp += 1;
        out
    }

    /// Brings the caret's line into view, and the caret itself sideways (`EditorView.scrollIntoView` on each selection change).
    fn reveal_caret(&mut self, tx: &mut Tx) {
        let wrap = self.code_edit.file.as_ref().is_some_and(|(_, _, p)| self.sidebar.file_wrap(p));
        let line = self.current_editor().map_or(0, |e| e.caret.line);
        tx.scroll_into_view_in(CODE, row_key(line, wrap));
        tx.scroll_into_view_in(CODE, "code-caret");
    }

    /// A key while the code view has the keyboard. Returns false for keys it leaves to the window (Ctrl+F, Alt+Z, Ctrl+K...).
    pub fn code_edit_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if self.code_edit_target().is_none() {
            return false;
        }
        let m = ev.mods;
        let mac = cfg!(target_os = "macos");
        // Word steps: Option on a Mac, Ctrl elsewhere. Line ends: Cmd+Left and Right on a Mac. Document ends: Cmd+Up and Down there,
        // Ctrl+Home and End elsewhere.
        let word = if mac { m.alt } else { m.ctrl };
        let line_cmd = mac && m.cmd;
        let page = ((self.code_edit.view_h / LINE_H) as usize).saturating_sub(1).max(1);
        let now = self.clock;
        let shift = m.shift;
        let motion = |s: &mut State, mv: Move| s.with_editor(|e, l| (e.motion(l, mv, shift), false));
        let handled = match &ev.key {
            // Alt+letter is the window's off a Mac (Alt+Z wraps); Ctrl+Alt is AltGr there and types.
            Key::Text(_) if m.alt && !m.ctrl && !mac => false,
            Key::Text(t) if m.alt && (t.eq_ignore_ascii_case("z") || t == "\u{3a9}" || t == "\u{b8}") => false,
            Key::Text(t) if !(m.cmd && !m.alt) && !t.is_empty() => {
                let t = t.clone();
                self.with_editor(|e, l| (e.insert(l, &t, now), true));
                true
            }
            Key::Enter if !m.cmd && !m.ctrl => {
                self.with_editor(|e, l| (e.newline(l, now), true));
                true
            }
            Key::Tab if !m.cmd && !m.ctrl && !m.alt => {
                // Shift+Tab would take the indent back (`indentLess`): not in the spike, but the keyboard stays in the editor.
                if !shift {
                    self.with_editor(|e, l| (e.tab(l, now), true));
                }
                true
            }
            Key::Backspace => {
                self.with_editor(|e, l| (e.delete(l, false, word || (mac && m.cmd), now), true));
                true
            }
            Key::Delete => {
                self.with_editor(|e, l| (e.delete(l, true, word, now), true));
                true
            }
            Key::Left => motion(
                self,
                if line_cmd {
                    Move::LineStart
                } else if word {
                    Move::WordLeft
                } else {
                    Move::Left
                },
            )
            .is_some(),
            Key::Right => motion(
                self,
                if line_cmd {
                    Move::LineEnd
                } else if word {
                    Move::WordRight
                } else {
                    Move::Right
                },
            )
            .is_some(),
            Key::Up => motion(self, if line_cmd { Move::DocStart } else { Move::Up(1) }).is_some(),
            Key::Down => motion(self, if line_cmd { Move::DocEnd } else { Move::Down(1) }).is_some(),
            Key::Home => motion(self, if m.ctrl || m.cmd { Move::DocStart } else { Move::LineStart }).is_some(),
            Key::End => motion(self, if m.ctrl || m.cmd { Move::DocEnd } else { Move::LineEnd }).is_some(),
            Key::PageUp => motion(self, Move::Up(page)).is_some(),
            Key::PageDown => motion(self, Move::Down(page)).is_some(),
            Key::Escape if self.current_editor().is_some_and(Editor::has_selection) => {
                let c = self.current_editor().map_or_else(Pos::default, |e| e.caret);
                self.with_editor(|e, _| (e.set_caret(c, false), false));
                true
            }
            Key::Command('a') if !shift => {
                self.with_editor(|e, l| (e.select_all(l), false));
                // Selecting everything doesn't move the view, as CodeMirror's doesn't.
                return true;
            }
            Key::Command('z') if !shift => self
                .with_editor(|e, l| {
                    let u = e.undo(l);
                    (u, u)
                })
                .is_some(),
            Key::Command('z') => self
                .with_editor(|e, l| {
                    let r = e.redo(l);
                    (r, r)
                })
                .is_some(),
            Key::Command('y') if !mac => self
                .with_editor(|e, l| {
                    let r = e.redo(l);
                    (r, r)
                })
                .is_some(),
            Key::Command('c') | Key::Command('x') => {
                let cut = ev.key == Key::Command('x');
                let text = self.with_editor(|e, l| {
                    let text = e.selected_text(l);
                    if cut && e.has_selection() {
                        e.delete(l, false, false, now);
                    }
                    (text, cut)
                });
                match text {
                    Some(t) if !t.is_empty() => tx.copy(t),
                    _ => {}
                }
                true
            }
            // Rune hands a paste to the root's `on_paste` only, so the editor reads the clipboard itself, as the terminal does.
            Key::Command('v') if !shift => {
                tx.read_clipboard(|s: &mut State, content, tx| {
                    if !content.text().is_empty() {
                        s.code_edit_paste(content.text(), tx);
                    }
                });
                return true;
            }
            _ => false,
        };
        if handled {
            self.reveal_caret(tx);
        }
        handled
    }

    /// A paste while the code view has the keyboard.
    pub fn code_edit_paste(&mut self, text: &str, tx: &mut Tx) {
        let now = self.clock;
        // The file's own breaks are put back when it is saved (piece Q's `to_disk_text`); in the buffer every break is `\n`.
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.with_editor(|e, l| (e.paste(l, &text, now), true)).is_some() {
            self.reveal_caret(tx);
        }
    }

    /// What an input method composed (`App::on_ime`): the preedit is drawn at the caret, a commit is typed.
    pub fn code_edit_ime(&mut self, ime: &Ime, tx: &mut Tx) -> bool {
        if self.code_edit_target().is_none() || tx.focused() != Some(CODE) {
            return false;
        }
        let now = self.clock;
        match ime {
            Ime::Preedit(text, cursor, _) => {
                let preedit = (!text.is_empty()).then(|| (text.clone(), *cursor));
                self.with_editor(|e, _| {
                    e.preedit = preedit;
                    ((), false)
                });
            }
            Ime::Commit(text) => {
                let text = text.clone();
                self.with_editor(|e, l| {
                    e.preedit = None;
                    if !text.is_empty() {
                        e.insert(l, &text, now);
                    }
                    ((), true)
                });
            }
            _ => return false,
        }
        self.reveal_caret(tx);
        true
    }

    /// Keeps `focused` with where the keyboard is: a blur is no event in Rune (docs/RUNE.md), so the poll looks.
    pub fn keep_code_edit_focus(&mut self, tx: &mut Tx) -> bool {
        if let Some(key) = self.code_edit.take_focus.take() {
            tx.focus(key, false);
            if key == CODE {
                self.reveal_caret(tx);
            }
        }
        let focused = self.code_edit.on && tx.focused() == Some(CODE);
        if focused == self.code_edit.focused {
            return false;
        }
        self.code_edit.focused = focused;
        // Focus from a click on the numbers or Go to file starts the caret where nothing was yet.
        if focused && let Some(file) = self.code_edit_target() {
            self.code_edit.for_file(&file);
        }
        true
    }

    /// A press (and a drag) on line `line` of the code at byte `byte`: the caret goes there, Shift or a drag extends the selection.
    pub fn code_edit_press(&mut self, at: Pos, extend: bool, tx: &mut Tx) {
        tx.focus(CODE, false);
        self.with_editor(|e, _| (e.set_caret(at, extend), false));
    }
}
