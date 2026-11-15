//! Export as Markdown: opening the dialog for the open session, loading its whole history, and Copy and Save
//! (`use-export-markdown.ts`, `ExportMarkdownDialog.vue`'s script). The dialog's own state is `conv::export_dialog`; the
//! paging is `conv::export_history`; this file starts the thread that pages and takes its answers.
use super::http;
use crate::State;
use crate::conv::export::{ExportToolMode, find_key_lines};
use crate::conv::export_dialog::{ExportDialog, Load, Pane};
use crate::conv::export_history::{HistoryError, load_whole_history};
use crate::fleet::model::Msg;
use rune::{FileDialogOptions, Tx};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// How long the Copy button says "Copied" (2000 ms).
const COPIED_SECONDS: f64 = 2.0;

/// What a file write came to, from the thread that did it.
pub struct Saved {
    generation: u64,
    name: String,
    result: Result<(), String>,
}

pub struct Export {
    /// The dialog, while it is open.
    pub dialog: Option<ExportDialog>,
    generation: u64,
    send: Sender<Saved>,
    receive: Receiver<Saved>,
    writing: usize,
}

impl Default for Export {
    fn default() -> Self {
        let (send, receive) = channel();
        Export { dialog: None, generation: 0, send, receive, writing: 0 }
    }
}

impl State {
    /// `openExportMarkdown`: the dialog for the open session; the palette's "Export as Markdown" and the session menu's
    /// "Export as Markdown…" come here.
    pub fn export_open(&mut self) {
        if !self.has_session() {
            return;
        }
        let session = self.session();
        let title = if session.title.trim().is_empty() { "Untitled session".to_string() } else { session.title.clone() };
        let harness = session.row.harness_type.clone().filter(|h| !h.is_empty());
        let directory = Some(self.project().path.clone()).filter(|p| !p.is_empty());
        let id = self.open_id().unwrap_or_else(|| match &self.route {
            super::store::Route::Session(id) => id.clone(),
            super::store::Route::None => String::new(),
        });
        self.export.generation += 1;
        self.export.dialog = Some(ExportDialog::new(id, title, harness, directory, self.export.generation));
        self.export_load();
    }

    /// The export's time (the file's header): the real clock, or the one a scene fixed.
    fn export_time(&self) -> i64 {
        self.now_ms.unwrap_or_else(super::link::now_ms)
    }

    /// A dialog with its history not asked for yet, in the state given: a scene's or a test's way in.
    pub fn show_export(&mut self, load: Load) {
        self.export_open();
        self.export_stop();
        if let Some(d) = &mut self.export.dialog {
            d.load = load;
            d.messages.clear();
            d.built = None;
        }
    }

    /// `load`: starts paging the history (again, for Try again). With no Fleet to ask, the invented session's messages
    /// are what there is, and they are there at once.
    pub fn export_load(&mut self) {
        let Some(d) = &mut self.export.dialog else { return };
        d.cancel.store(true, Ordering::Relaxed);
        self.export.generation += 1;
        d.generation = self.export.generation;
        d.cancel = Default::default();
        d.load = Load::Loading(0);
        d.messages.clear();
        d.built = None;
        let (generation, id, cancel) = (d.generation, d.session_id.clone(), d.cancel.clone());
        let Some(link) = self.link.as_ref().filter(|_| self.open_id().is_some()) else {
            self.export_done(generation, Ok(crate::mock::export::session()));
            return;
        };
        let (base, post) = (link.base.clone(), link.post());
        std::thread::spawn(move || {
            let progress_post = post.clone();
            let result = load_whole_history(
                &id,
                &mut |path| http::get_page(&base, path),
                &mut |count| progress_post.update(move |s, _| s.export_progress(generation, count)),
                &|| cancel.load(Ordering::Relaxed),
            );
            post.update(move |s, _| s.export_done(generation, result));
        });
    }

    /// Stops the thread that is paging, if one is.
    fn export_stop(&mut self) {
        if let Some(d) = &self.export.dialog {
            d.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// `onProgress`: the count after a page.
    pub fn export_progress(&mut self, generation: u64, loaded: usize) {
        if let Some(d) = self.export.dialog.as_mut().filter(|d| d.generation == generation && matches!(d.load, Load::Loading(_))) {
            d.load = Load::Loading(loaded);
        }
    }

    /// The whole history arrived, or a page failed. An aborted load shows nothing.
    pub fn export_done(&mut self, generation: u64, result: Result<Vec<Msg>, HistoryError>) {
        let now = self.export_time();
        let Some(d) = self.export.dialog.as_mut().filter(|d| d.generation == generation) else { return };
        match result {
            Ok(messages) => {
                d.messages = messages;
                d.load = Load::Ready;
                d.rebuild(now);
            }
            Err(HistoryError::Failed(why)) => d.load = Load::Failed(why),
            Err(HistoryError::Aborted) => {}
        }
    }

    /// The dialog closes (Esc, the X, a press outside): the load stops.
    pub fn export_close(&mut self) {
        self.export_stop();
        self.export.dialog = None;
    }

    /// A tool-calls radio (7.10).
    pub fn export_set_tools(&mut self, mode: ExportToolMode) {
        self.export_change(|o| o.tools = mode);
    }

    pub fn export_toggle_thinking(&mut self) {
        if self.export.dialog.as_ref().is_some_and(|d| d.thinking_available()) {
            self.export_change(|o| o.thinking = !o.thinking);
        }
    }

    pub fn export_toggle_paths(&mut self) {
        self.export_change(|o| o.shorten_paths = !o.shorten_paths);
    }

    pub fn export_toggle_meta(&mut self) {
        self.export_change(|o| o.meta = !o.meta);
    }

    /// Any option changes the file at once (7.14).
    fn export_change(&mut self, change: impl FnOnce(&mut crate::conv::export::ExportOptions)) {
        let now = self.export_time();
        if let Some(d) = &mut self.export.dialog {
            change(&mut d.options);
            d.rebuild(now);
        }
    }

    pub fn export_set_pane(&mut self, pane: Pane) {
        if let Some(d) = &mut self.export.dialog {
            d.pane = pane;
        }
    }

    /// Copy: the whole file to the clipboard, and "Copied" for two seconds. Headless runs never write the system
    /// clipboard, so there is no failure to report here (Fleet's "Fleet could not copy. Use Save instead." is for a browser
    /// that refuses).
    pub fn export_copy(&mut self, tx: &mut Tx) {
        let clock = self.clock;
        let Some(d) = self.export.dialog.as_mut().filter(|d| d.built.is_some()) else { return };
        d.action_error = None;
        tx.copy(d.markdown().to_string());
        d.copied_until = Some(clock + COPIED_SECONDS);
        tx.wake_after(Duration::from_millis(100));
    }

    /// Save .md: the system's save dialog, named as Fleet names the download; the file is written off the UI thread.
    pub fn export_save(&mut self, tx: &mut Tx) {
        let Some(d) = self.export.dialog.as_mut().filter(|d| d.built.is_some()) else { return };
        d.action_error = None;
        let options = FileDialogOptions { title: "Save as Markdown".into(), file_name: Some(d.file_name()), ..Default::default() }
            .filter("Markdown", &["md"]);
        let (generation, name) = (d.generation, d.file_name());
        tx.save_file(options, move |s: &mut State, answer, tx| match answer {
            Ok(Some(path)) => s.export_write(generation, name, path, tx),
            Ok(None) => {}
            Err(_) => s.export_failed(generation, "Fleet could not open the save dialog. Use Copy instead."),
        });
    }

    fn export_failed(&mut self, generation: u64, why: &str) {
        if let Some(d) = self.export.dialog.as_mut().filter(|d| d.generation == generation) {
            d.action_error = Some(why.into());
        }
    }

    /// Writes the file the dialog holds *now* (not what it held when Save was pressed: the options may have moved while the
    /// system's dialog was up, and the name the user sees should match what they asked for).
    fn export_write(&mut self, generation: u64, name: String, path: PathBuf, tx: &mut Tx) {
        let Some(d) = self.export.dialog.as_ref().filter(|d| d.generation == generation && d.built.is_some()) else { return };
        let (markdown, send) = (d.markdown().to_string(), self.export.send.clone());
        self.export.writing += 1;
        std::thread::spawn(move || {
            let result = std::fs::write(&path, markdown).map_err(|e| e.to_string());
            let _ = send.send(Saved { generation, name, result });
        });
        tx.wake_after(Duration::from_millis(50));
    }

    /// From `poll`: the Copied label ends, and a finished write is told. True when something changed.
    pub fn export_poll(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        let clock = self.clock;
        if let Some(d) = &mut self.export.dialog
            && let Some(until) = d.copied_until
        {
            if clock >= until {
                d.copied_until = None;
                changed = true;
            } else {
                tx.wake_after(Duration::from_millis(100));
            }
        }
        while let Ok(saved) = self.export.receive.try_recv() {
            self.export.writing = self.export.writing.saturating_sub(1);
            changed = true;
            match saved.result {
                Ok(()) => self.toast(format!("Saved {}", saved.name), None, 3.0, tx),
                Err(_) => self.export_failed(saved.generation, "Fleet could not save the file. Use Copy instead."),
            }
        }
        if self.export.writing > 0 {
            tx.wake_after(Duration::from_millis(50));
        }
        changed
    }

    /// For tests and the dialog's note: the lines of the file that look like keys.
    pub fn export_key_lines(&self) -> usize {
        self.export.dialog.as_ref().map_or(0, |d| find_key_lines(d.markdown()))
    }
}
