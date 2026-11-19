//! Saving the open file from its tab (`FileCanvas.vue` `save`, rows 13.3 and 13.4 of docs/parity/11-files.md): the Save button and
//! Ctrl/Cmd+S in the editor ask for the same thing, and the answer is said in the tab's toast.
use super::file_buffers::SaveOutcome;
use crate::State;
use rune::Tx;

impl State {
    /// Saves a file of `session`: nothing while the file is in conflict (the notice's buttons are the way out), "No changes to save"
    /// when it is clean, else the request and its toast.
    pub fn save_file(&mut self, session: &str, path: &str, tx: &mut Tx) {
        let machine = self.machine_key_of(session).to_string();
        let Some(info) = self.file_buffers.info(&machine, session, path) else { return };
        if info.conflict.is_some() {
            return;
        }
        if !info.dirty {
            self.file_toast("No changes to save".into(), tx);
            return;
        }
        let name = path.to_string();
        self.save_file_then(session, path, false, move |s, tx, outcome| match outcome {
            Some(SaveOutcome::Saved) => s.file_toast(format!("Saved {name}"), tx),
            Some(SaveOutcome::Error(why)) => s.file_toast(format!("Couldn't save: {why}"), tx),
            _ => {}
        });
    }

    /// The Save button and the editor's Ctrl/Cmd+S: the open file tab's file.
    pub fn save_active_file(&mut self, tx: &mut Tx) {
        let tabs = self.open_canvases();
        let Some(file) = tabs.active().and_then(|c| c.file.clone()) else { return };
        let session = self.canvas_session();
        self.save_file(&session, &file.path, tx);
    }
}
