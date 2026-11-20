//! Closing a file with unsaved changes, and quitting with some (component 11, piece T): `CanvasHost.vue`'s `close`, `discardAndClose` and
//! `saveAndClose` with `UnsavedFileDialog.vue`, and `use-unsaved-files-guard.ts`. A tab with text that differs from the disk asks before it
//! goes; the app asks before it quits while any file has such text.
use super::canvases::CanvasInstance;
use super::file_buffers::SaveOutcome;
use crate::{State, Tx};

/// The file tab the question is about (`closingFile`) and whether its save is on its way (`closingSaving`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closing {
    pub tab: String,
    pub session: String,
    pub path: String,
    pub saving: bool,
}

#[derive(Debug, Default)]
pub struct Unsaved {
    pub closing: Option<Closing>,
    /// The app was asked to quit while these files (paths, in the order of the buffers) had unsaved changes.
    pub quitting: Option<Vec<String>>,
}

/// The dialog's words (`UnsavedFileDialog.vue`).
pub const TITLE: &str = "Save changes before closing?";
pub const DETAIL: &str = "has changes that aren't saved. If you close it without saving, they're lost.";
pub const CANCEL: &str = "Cancel";
pub const DISCARD: &str = "Close without saving";
pub const SAVE: &str = "Save and close";
pub const SAVING: &str = "Saving\u{2026}";

/// The question before the app quits with unsaved files. Fleet leaves this to the browser's own prompt (`beforeunload`); the native
/// app has no such prompt, so it asks in the same words as the close: what is lost, and Cancel.
pub const QUIT_TITLE: &str = "Quit with unsaved changes?";
pub const QUIT_BUTTON: &str = "Quit without saving";

/// The sentence under the quit question: one file or several.
pub fn quit_detail(n: usize) -> String {
    if n == 1 {
        "1 file has changes that aren't saved. If you quit, they're lost.".into()
    } else {
        format!("{n} files have changes that aren't saved. If you quit, they're lost.")
    }
}

impl State {
    /// `isUnsaved`: a file tab whose buffer's text differs from the disk's.
    pub fn is_tab_unsaved(&self, canvas: &CanvasInstance) -> bool {
        let Some(file) = &canvas.file else { return false };
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session);
        self.file_buffers.is_dirty(machine, &session, &file.path)
    }

    /// `close`: a file with unsaved changes asks first; every other tab goes at once.
    pub fn request_close_tab(&mut self, id: &str, tx: &mut Tx) {
        let tabs = self.open_canvases();
        let Some(canvas) = tabs.canvases.iter().find(|c| c.id == id) else { return };
        let asked = match &canvas.file {
            Some(file) if canvas.server.is_none() && self.is_tab_unsaved(canvas) => Some(file.path.clone()),
            _ => None,
        };
        match asked {
            Some(path) => {
                self.unsaved.closing = Some(Closing { tab: id.to_string(), session: self.canvas_session(), path, saving: false });
            }
            None => self.close_tab(id, tx),
        }
    }

    /// Cancel, Escape: nothing changes (`@update:open`).
    pub fn close_cancel(&mut self) {
        if self.unsaved.closing.as_ref().is_some_and(|c| !c.saving) {
            self.unsaved.closing = None;
        }
    }

    /// Close without saving (`discardAndClose`): the tab and its buffer go, the changes with them.
    pub fn close_discard(&mut self, tx: &mut Tx) {
        let Some(closing) = self.unsaved.closing.take().filter(|c| !c.saving) else { return };
        self.close_asked_tab(&closing, tx);
    }

    /// Save and close (`saveAndClose`). Saved: the tab closes. Not saved (a conflict, a refusal, nothing sent): the dialog closes and
    /// the tab is shown, with its bar saying why.
    pub fn close_save(&mut self, tx: &mut Tx) {
        let Some(closing) = self.unsaved.closing.as_mut().filter(|c| !c.saving) else { return };
        closing.saving = true;
        let asked = closing.clone();
        self.save_file_then(&asked.session, &asked.path, false, {
            let asked = asked.clone();
            move |s, tx, outcome| {
                s.unsaved.closing = None;
                match outcome {
                    Some(SaveOutcome::Saved) => s.close_asked_tab(&asked, tx),
                    _ => s.show_asked_tab(&asked, tx),
                }
            }
        });
        // Nothing was sent (no link, a save already on its way, no hash): there will be no answer to wait for.
        let machine = self.machine_key_of(&asked.session).to_string();
        if !self.file_buffers.info(&machine, &asked.session, &asked.path).is_some_and(|i| i.saving) && self.unsaved.closing.is_some() {
            self.unsaved.closing = None;
            self.show_asked_tab(&asked, tx);
        }
    }

    fn close_asked_tab(&mut self, closing: &Closing, tx: &mut Tx) {
        if closing.session == self.canvas_session() {
            self.close_tab(&closing.tab, tx);
        }
    }

    fn show_asked_tab(&mut self, closing: &Closing, tx: &mut Tx) {
        if closing.session == self.canvas_session() {
            self.activate_tab(&closing.tab, tx);
        }
    }

    /// `useUnsavedFilesGuard`: the paths of the files with unsaved changes, across sessions.
    pub fn unsaved_files(&self) -> Vec<String> {
        self.file_buffers.unsaved().into_iter().map(|(_, _, path)| path).collect()
    }

    /// The app is about to quit: with unsaved files it asks first and returns `true` (the caller doesn't quit); with none it returns
    /// `false` and the quit goes on.
    pub fn ask_before_quitting(&mut self) -> bool {
        let files = self.unsaved_files();
        if files.is_empty() {
            return false;
        }
        self.unsaved.quitting = Some(files);
        true
    }

    /// Cancel on the quit question.
    pub fn quit_unsaved_cancel(&mut self) {
        self.unsaved.quitting = None;
    }

    /// Quit without saving: the question is answered and the app quits.
    pub fn quit_unsaved_confirm(&mut self, tx: &mut Tx) {
        if self.unsaved.quitting.take().is_some() {
            tx.quit();
        }
    }

    /// The scenes' pictures of the close question, on the open file tab with unsaved text.
    pub fn show_close_question(&mut self, dark: bool) {
        self.show_file(crate::mock::file_tab::Case::Unsaved, dark);
        let tab = self.active_canvas();
        let session = self.canvas_session();
        self.unsaved.closing = Some(Closing { tab, session, path: crate::mock::file_tab::PATH.into(), saving: false });
    }
}

impl State {
    /// The strip with an unsaved file tab beside a clean one: the dot where the close button is.
    pub fn show_unsaved_tab(&mut self, dark: bool) {
        self.show_file(crate::mock::file_tab::Case::Unsaved, dark);
        self.edit_canvases(|c, id| {
            c.open_file(id, "README.md", crate::fleet::canvases::OpenFile { keep: true, view: None }, &|_| false);
        });
        let tab = format!("file:{}", crate::mock::file_tab::PATH);
        let id = self.canvas_session();
        self.edit_canvases(|c, _| c.activate(&id, &tab));
    }

    /// The question before the app quits, over the file tab with unsaved text.
    pub fn show_quit_question(&mut self, dark: bool) {
        self.show_file(crate::mock::file_tab::Case::Unsaved, dark);
        self.ask_before_quitting();
    }
}
