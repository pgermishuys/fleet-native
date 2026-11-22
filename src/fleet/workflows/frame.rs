//! The editor's frame (component 18, piece M): what `WorkflowEditor.vue`'s script and `WorkflowDetailPanel.vue` do around the
//! editor state of piece L. Which workflow file is open, the toast after a save and after Try it, Try it itself, the runs note,
//! the row's "Edited", and the leave guard (`setLeaveGuard`): leaving unsaved edits asks first, through `ui::confirm`.
use super::{Run, RunStatus};
use crate::fleet::actions::Call;
use crate::ui::confirm::Confirm;
use crate::{State, Tx};
use std::time::Duration;

/// How long the toast stays (`flash`).
pub const NOTICE_SECONDS: f64 = 4.0;

pub const SAVE_FIRST: &str = "Save first: a run uses the file as saved.";

/// Where a guarded move was going; the question's Yes takes it. Piece S adds the drafts' starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Leave {
    /// Another workflow's row (`setActiveWorkflow`).
    Open(String),
}

#[derive(Debug, Default)]
pub struct Frame {
    /// The toast and when it goes.
    pub notice: Option<(String, f64)>,
    /// The file the page last asked the server for (machine, repository, workflow): a failed open isn't asked again until the
    /// choice changes, as Fleet's watch fires on a change only.
    asked: Option<(String, String, String)>,
    /// The page was open at the last frame, so leaving it is seen once.
    on_page: bool,
}

impl Frame {
    /// The page's own editor is gone (another page, `onBeforeUnmount`): the next visit asks again.
    pub fn forget(&mut self) {
        self.asked = None;
    }
}

/// `runsNote`: "2 runs in progress keep the version they started with."
pub fn runs_note(runs: &[&Run]) -> Option<String> {
    let n = runs.len();
    (n > 0).then(|| {
        let (s, v, it) = if n == 1 { ("", "s", "it") } else { ("s", "", "they") };
        format!("{n} run{s} in progress keep{v} the version {it} started with.")
    })
}

impl State {
    /// `showEditor`: a repository's workflow, unless the run view is on (Try it).
    pub fn workflow_editor_shown(&self) -> bool {
        let nav = &self.workflows_nav;
        nav.active_id().starts_with("repo:") && !nav.showing_runs
    }

    /// Runs of the open file that haven't finished: they keep the version they started with.
    pub fn workflow_runs_note(&self) -> Option<String> {
        let id = self.workflows_nav.editor.file.as_ref()?.workflow_id.as_str();
        let store = self.workflow_runs(&self.workflows_machine());
        let going: Vec<&Run> = store
            .ordered()
            .into_iter()
            .filter(|r| r.workflow_id == id && matches!(r.status, RunStatus::Running | RunStatus::Waiting))
            .collect();
        runs_note(&going)
    }

    /// `flash`: the toast, for four seconds.
    pub fn workflow_flash(&mut self, message: String, tx: &mut Tx) {
        self.workflows_nav.frame.notice = Some((message, self.clock + NOTICE_SECONDS));
        tx.wake_after(Duration::from_millis(100));
    }

    /// Try it: with unsaved edits it says to save first; else the run view for the file as saved, the library read again.
    pub fn workflow_try_it(&mut self, tx: &mut Tx) {
        if self.workflows_nav.editor.is_dirty() {
            return self.workflow_flash(SAVE_FIRST.into(), tx);
        }
        self.workflows_nav.showing_runs = true;
        self.workflows_nav.reload_wanted = true;
        tx.focus(super::run_box::REQUEST_KEY, false);
    }

    /// Ctrl/Cmd+S and the Save button. A save that wasn't allowed does nothing.
    pub fn workflow_save_pressed(&mut self) {
        self.workflow_editor_save(false);
    }

    /// Whether leaving the open workflow would lose something (`editor.isDirty`).
    pub fn workflow_has_unsaved(&self) -> bool {
        self.rail == crate::Rail::Workflows && self.workflows_nav.editor.is_dirty()
    }

    /// `setLeaveGuard`'s answer: the move happens now when nothing is unsaved; else the question is asked, and its Yes moves.
    /// True when it moved.
    pub fn workflow_leave(&mut self, to: Leave) -> bool {
        if !self.workflow_has_unsaved() {
            self.workflow_leave_now(to);
            return true;
        }
        let ed = &self.workflows_nav.editor;
        let name = ed.draft.as_ref().map(|d| d.name.clone()).filter(|n| !n.is_empty());
        let call = Call::LeaveWorkflow { to };
        let confirm = if ed.drafted.is_some() {
            ask("Discard the drafted workflow?", name.unwrap_or_default(), "It hasn't been saved.", "Discard draft", call)
        } else {
            let subject = name.or_else(|| ed.file.as_ref().map(|f| f.file.clone())).unwrap_or_default();
            ask("Discard your unsaved changes?", subject, "Your edits to this workflow are lost.", "Discard changes", call)
        };
        self.ui.open = None;
        self.ui.confirm = Some(confirm);
        false
    }

    /// The question's Yes, or nothing to ask.
    pub fn workflow_leave_now(&mut self, to: Leave) {
        match to {
            Leave::Open(id) => self.workflows_nav.move_to(&id),
        }
    }

    /// The frame's upkeep, once a frame (`poll`): follows the open workflow into the editor, tells the list which row is
    /// edited, announces a save, ends the toast. True when something changed.
    pub fn keep_workflow_frame(&mut self, now: f64, tx: &mut Tx) -> bool {
        let on_page = self.rail == crate::Rail::Workflows;
        let left = std::mem::replace(&mut self.workflows_nav.frame.on_page, on_page) && !on_page;
        if !on_page {
            if !left {
                return false;
            }
            // The editor goes with its page (`dispose`): edits left behind on another page are gone, as in Fleet.
            let nav = &mut self.workflows_nav;
            nav.editor.cancel_check();
            nav.editor = Default::default();
            nav.unsaved = None;
            nav.frame.forget();
            return true;
        }
        let mut changed = self.follow_workflow_into_editor();
        let ed = &self.workflows_nav.editor;
        let unsaved = ed.file.as_ref().filter(|_| ed.is_dirty()).map(|f| f.workflow_id.clone());
        if self.workflows_nav.unsaved != unsaved {
            self.workflows_nav.unsaved = unsaved;
            changed = true;
        }
        if let Some(saved) = self.workflows_nav.editor.take_saved() {
            let note = self.workflow_runs_note();
            let said = format!(
                "Saved {}. Commit it with your other changes to share it.{}",
                saved.file,
                note.map(|n| format!(" {n}")).unwrap_or_default()
            );
            self.workflow_flash(said, tx);
            // The library reloads so the list and the Run box see the file as it is now.
            self.workflows_nav.reload_wanted = true;
            changed = true;
        }
        match &self.workflows_nav.frame.notice {
            Some((_, until)) if now >= *until => {
                self.workflows_nav.frame.notice = None;
                changed = true;
            }
            Some(_) => tx.wake_after(Duration::from_millis(100)),
            None => {}
        }
        changed
    }

    /// The `watch` on the active id and the repository: a repository's workflow is read from the server once it is chosen.
    fn follow_workflow_into_editor(&mut self) -> bool {
        let nav = &self.workflows_nav;
        let id = nav.active_id().to_string();
        let Some(repository) = nav.repository_path().map(str::to_string).filter(|_| id.starts_with("repo:")) else { return false };
        let ed = &nav.editor;
        // A drafted workflow has no file yet and stays until it is saved or discarded.
        if ed.drafted.is_some() {
            return false;
        }
        if ed.file.as_ref().is_some_and(|f| f.workflow_id == id) && ed.directory.as_deref() == Some(repository.as_str()) {
            return false;
        }
        let machine = self.workflows_machine();
        let asked = (machine.clone(), repository.clone(), id.clone());
        if self.workflows_nav.frame.asked.as_ref() == Some(&asked) {
            return false;
        }
        self.workflows_nav.frame.asked = Some(asked);
        if self.link.is_none() {
            // With no Fleet to ask (the app on its invented data, scenes, tests) the file is an invented one.
            let opened = crate::mock::workflow_editor::file(&id);
            self.workflows_nav.editor.adopt(&machine, &repository, opened);
            return true;
        }
        self.workflow_editor_open(&machine, &repository, &id);
        true
    }
}

fn ask(title: &str, subject: String, body: &str, confirm_label: &str, call: Call) -> Confirm {
    Confirm {
        title: title.into(),
        subject,
        body: body.into(),
        confirm_label: confirm_label.into(),
        busy_label: String::new(),
        error_message: String::new(),
        destructive: true,
        busy: false,
        error: None,
        call,
    }
}
