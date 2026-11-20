//! The conflict's buttons and Compare (`FileCanvas.vue` `keepMine`, `useAgents`, `compare`, `doneComparing`, rows 13.6 and 13.7 of
//! docs/parity/11-files.md). Compare is a mode of the conflict bar, on top of whichever view the tab is in: the agent's version is the
//! merge view's "original", and per change you take the agent's lines or keep yours. Keeping yours only changes the original that is
//! drawn (CodeMirror's `accept`); what Done rebases on is the version the conflict holds.
use super::file_buffers::SaveOutcome;
use crate::State;
use crate::conv::changed_lines::base_lines;
use crate::conv::file_diff::{chunks, reverted, rows};
use rune::Tx;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct FileCompare {
    /// The agent's lines as Compare draws them, for each file in Compare (by session and path).
    original: HashMap<(String, String), Vec<String>>,
}

impl FileCompare {
    pub fn original(&self, session: &str, path: &str) -> Option<&Vec<String>> {
        self.original.get(&(session.to_string(), path.to_string()))
    }

    pub fn forget(&mut self, session: &str, path: &str) {
        self.original.remove(&(session.to_string(), path.to_string()));
    }
}

impl State {
    fn file_name_of(path: &str) -> &str {
        path.rsplit('/').next().unwrap_or(path)
    }

    /// Whether the file is in Compare: a conflict that is gone (the file was read again and matches, or it was saved) ends it.
    pub fn is_comparing(&self, session: &str, path: &str) -> bool {
        let machine = self.machine_key_of(session);
        self.file_compare.original(session, path).is_some()
            && self.file_buffers.info(machine, session, path).is_some_and(|i| i.conflict.is_some())
    }

    /// Compare: the agent's version against yours, in the tab's editor.
    pub fn compare_file(&mut self, session: &str, path: &str) {
        let machine = self.machine_key_of(session).to_string();
        let Some(disk) =
            self.file_buffers.info(&machine, session, path).and_then(|i| i.conflict.as_ref()).and_then(|c| c.disk_text.clone())
        else {
            return;
        };
        self.file_compare.original.insert((session.to_string(), path.to_string()), base_lines(&disk));
        self.code_edit.stamp += 1;
        self.code_edit.take_focus = Some(self.edit_surface());
    }

    /// Keep mine: saves over exactly the version the conflict showed, still losing to a newer change.
    pub fn keep_mine(&mut self, session: &str, path: &str) {
        self.file_compare.forget(session, path);
        let name = Self::file_name_of(path).to_string();
        self.save_file_then(session, path, true, move |s, tx, outcome| match outcome {
            Some(SaveOutcome::Saved) => s.file_toast(format!("Saved your version of {name}"), tx),
            Some(SaveOutcome::Error(why)) => s.file_toast(format!("Couldn't save: {why}"), tx),
            _ => {}
        });
        self.code_edit.stamp += 1;
    }

    /// Use the agent's: the buffer becomes what is on disk, as one edit undo takes back.
    pub fn use_agents(&mut self, session: &str, path: &str) {
        self.file_compare.forget(session, path);
        self.use_disk_version(session, path);
        self.code_edit.stamp += 1;
        self.code_edit.take_focus = Some(self.edit_surface());
    }

    /// Done: the buffer is based on the agent's version and is unsaved if it still differs.
    pub fn done_comparing(&mut self, session: &str, path: &str, tx: &mut Tx) {
        self.file_compare.forget(session, path);
        self.finish_compare(session, path);
        let machine = self.machine_key_of(session).to_string();
        let dirty = self.file_buffers.info(&machine, session, path).is_some_and(|i| i.dirty);
        self.file_toast(if dirty { "Merged. Save to write it." } else { "You took the agent's version" }.into(), tx);
        self.code_edit.stamp += 1;
        // Back to the editor, so Ctrl S saves the merge.
        self.code_edit.take_focus = Some(self.edit_surface());
    }

    /// "Take the agent's" on change `k`: the agent's lines go into the buffer as one step undo takes back.
    pub fn take_agents_chunk(&mut self, path: &str, k: usize) {
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let Some(original) = self.file_compare.original(&session, path).cloned() else { return };
        let Some(buffer) = self.file_buffers.buffer(&machine, &session, path) else { return };
        let found = chunks(&rows(&original, buffer.lines()));
        let Some(chunk) = found.get(k) else { return };
        let after = reverted(buffer.lines(), &original, chunk);
        let now = self.clock;
        self.file_buffers.replace_as_edit(&machine, &session, path, &after, now);
        self.code_edit.stamp += 1;
    }

    /// "Keep mine" on change `k`: the change stops being one, as the agent's lines are now yours.
    pub fn keep_mine_chunk(&mut self, path: &str, k: usize) {
        let session = self.canvas_session();
        let machine = self.machine_key_of(&session).to_string();
        let Some(original) = self.file_compare.original(&session, path).cloned() else { return };
        let Some(buffer) = self.file_buffers.buffer(&machine, &session, path) else { return };
        let found = chunks(&rows(&original, buffer.lines()));
        let Some(chunk) = found.get(k) else { return };
        let mut next = original;
        next.splice(chunk.base.clone(), buffer.lines()[chunk.doc.clone()].iter().cloned());
        self.file_compare.original.insert((session, path.to_string()), next);
        self.code_edit.stamp += 1;
    }
}
