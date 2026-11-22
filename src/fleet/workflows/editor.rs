//! One open workflow file (component 18, piece L; `composables/use-workflow-editor.ts`): the designer's draft and the File
//! view's text, the server's check of whichever was edited last, and saving. The File view's text is saved exactly as typed;
//! only a draft edited in the designer is written in Fleet's layout, which removes comments, so the first such save asks.
//!
//! Plain state plus the calls, which run on threads. Fleet's promises become a run number (a check that was overtaken is
//! ignored when it lands) and a list of things waiting for the check to finish (`flush`). The 300 ms wait after the last
//! key is the clock `poll` hands `keep_workflow_editor`. No view: piece M draws the frame, N the File view, O the canvas.
use super::draft::{DraftedWorkflow, WorkflowCheck, WorkflowComment, WorkflowDraft, WorkflowFile, same_draft, slug_of};
use super::runs::error_from;
use crate::State;
use crate::fleet::http;
use rune::Tx;
use serde_json::{Value, json};
use std::time::Duration;

/// How long after the last keystroke the server checks the file (`CHECK_DELAY_MS`).
pub const CHECK_DELAY: f64 = 0.3;

pub const DESIGNER_BLOCKED_MESSAGE: &str =
    "The designer can't show this file as it is. Fix the errors in the File view to use the designer.";

const CHECK_FALLBACK: &str = "Couldn't check the workflow.";
const OPEN_FALLBACK: &str = "Couldn't open the workflow file.";
const SAVE_FALLBACK: &str = "Couldn't save the workflow file.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorView {
    #[default]
    Designer,
    File,
}

/// Which view was edited last, so Save knows what to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Draft,
    Text,
}

/// The wait after the last key: armed by an edit, given its time by the next `poll` (a handler has no clock).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Timer {
    #[default]
    Off,
    Armed,
    At(f64),
}

/// What `flush` was holding back until the check finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    View(EditorView),
    Save(bool),
}

/// A save that just happened, for the frame to announce (piece M): taken once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub workflow_id: String,
    pub file: String,
}

#[derive(Debug, Default)]
pub struct Editor {
    /// The machine every request goes to (`useMachineTarget`).
    machine: String,
    pub directory: Option<String>,
    pub file: Option<WorkflowFile>,
    pub view: EditorView,
    pub draft: Option<WorkflowDraft>,
    pub text: String,
    /// None when nothing has changed since opening or saving.
    pub source: Option<Source>,
    pub check: Option<WorkflowCheck>,
    pub is_checking: bool,
    pub is_saving: bool,
    pub load_error: Option<String>,
    pub save_error: Option<String>,
    /// Why Designer didn't open: the File view's text can't be shown there without losing some of it.
    pub designer_blocked: Option<String>,
    /// The comments in the file's text as it is now: what a save from the designer would remove.
    pub comments: Vec<WorkflowComment>,
    comments_confirmed: bool,
    pub asking_to_remove_comments: bool,
    /// The file changed on disk since it was opened: Reload or Keep mine. Holds the server's words.
    pub conflict: Option<String>,
    /// A drafted workflow that has never been saved: there's no file yet, and Save creates it.
    pub drafted: Option<DraftedWorkflow>,
    saved_text: String,
    /// The text as it was when the designer's edits began: what Edit in File view goes back to, comments and all.
    text_before_designer: Option<String>,
    saved_draft: Option<WorkflowDraft>,
    timer: Timer,
    /// The number of the newest check; an answer with an older one is ignored.
    generation: u64,
    /// The newest check is out and unanswered.
    in_flight: bool,
    waiting: Vec<Then>,
    /// Counts opens, so an old file's answer can't land on a newer one.
    opens: u64,
    just_saved: Option<Saved>,
}

impl Editor {
    /// The machine the open file is on.
    pub fn machine(&self) -> &str {
        &self.machine
    }

    pub fn errors(&self) -> &[super::draft::WorkflowProblem] {
        self.check.as_ref().map(|c| c.errors.as_slice()).unwrap_or(&[])
    }

    pub fn is_dirty(&self) -> bool {
        if self.drafted.is_some() {
            return true;
        }
        match self.source {
            Some(Source::Draft) => !same_draft(self.draft.as_ref(), self.saved_draft.as_ref()),
            Some(Source::Text) => self.text != self.saved_text,
            None => false,
        }
    }

    pub fn can_save(&self) -> bool {
        self.is_dirty() && !self.is_checking && !self.is_saving && self.errors().is_empty() && self.check.is_some()
    }

    /// The file a drafted workflow would be saved as, following its name as it's edited; the open file's own otherwise.
    pub fn file_name(&self) -> String {
        if self.drafted.is_none() {
            return self.file.as_ref().map(|f| f.file.clone()).unwrap_or_default();
        }
        let from_text = self.source == Some(Source::Text);
        let drafted = if from_text { self.check.as_ref().and_then(|c| c.draft.as_ref()) } else { self.draft.as_ref() };
        draft_file(drafted.or(self.draft.as_ref()).map(|d| d.name.as_str()).unwrap_or(""))
    }

    /// What a save sends: the draft when the designer was edited last, else the text as typed.
    fn content(&self) -> (bool, Value) {
        match (&self.source, &self.draft) {
            (Some(Source::Draft), Some(d)) => (true, json!({"draft": d.to_json()})),
            _ => (false, json!({"text": self.text})),
        }
    }

    /// The save that just happened, once.
    pub fn take_saved(&mut self) -> Option<Saved> {
        self.just_saved.take()
    }

    /// Something is waiting to be checked, or a check is out: the frame shows "checking".
    pub fn is_busy(&self) -> bool {
        self.timer != Timer::Off || self.in_flight
    }

    fn reset(&mut self, opened: WorkflowFile) {
        self.check = Some(opened.check.clone());
        self.text = opened.check.text.clone();
        self.saved_text = opened.check.text.clone();
        self.draft = opened.check.draft.clone();
        self.saved_draft = self.draft.clone();
        self.comments = opened.check.comments.clone();
        self.file = Some(opened);
        self.source = None;
        self.text_before_designer = None;
        self.designer_blocked = None;
        self.save_error = None;
    }

    /// The start of every open, adopt and adopt-a-draft.
    fn begin(&mut self, machine: &str, directory: &str) {
        self.cancel_check();
        self.opens += 1;
        self.machine = machine.to_string();
        self.directory = Some(directory.to_string());
        self.load_error = None;
        self.comments_confirmed = false;
        self.asking_to_remove_comments = false;
        self.conflict = None;
    }

    /// `cancelCheck` (and `dispose`): nothing waits, and an answer on its way will be ignored.
    pub fn cancel_check(&mut self) {
        self.timer = Timer::Off;
        self.generation += 1;
        self.in_flight = false;
        self.is_checking = false;
        self.waiting.clear();
    }

    /// `adopt`: a file New or Duplicate just made, opened without reading it again.
    pub fn adopt(&mut self, machine: &str, repository: &str, created: WorkflowFile) {
        self.begin(machine, repository);
        self.drafted = None;
        self.view = if created.check.draft.is_some() { EditorView::Designer } else { EditorView::File };
        self.reset(created);
    }

    /// `adoptDraft`: a drafted workflow, unsaved. A draft the designer can show opens there; one with errors opens in the File
    /// view, with them.
    pub fn adopt_draft(&mut self, machine: &str, next: DraftedWorkflow) {
        self.begin(machine, &next.repository);
        let name = next.check.draft.as_ref().map(|d| d.name.as_str()).unwrap_or("");
        let opened = WorkflowFile { workflow_id: String::new(), file: draft_file(name), hash: String::new(), check: next.check.clone() };
        self.view = if next.check.draft.is_some() && next.check.errors.is_empty() { EditorView::Designer } else { EditorView::File };
        self.reset(opened);
        self.drafted = Some(next);
    }

    /// `editDraft`: the designer changed the workflow; the server checks it after the wait.
    pub fn edit_draft(&mut self, next: WorkflowDraft) {
        if self.source != Some(Source::Draft) {
            self.text_before_designer = Some(self.text.clone());
        }
        self.draft = Some(next);
        self.source = Some(Source::Draft);
        self.save_error = None;
        self.schedule_check();
    }

    /// `editText`: the File view's text changed.
    pub fn edit_text(&mut self, next: &str) {
        if next == self.text {
            return;
        }
        self.text = next.to_string();
        self.source = Some(Source::Text);
        self.save_error = None;
        self.designer_blocked = None;
        self.schedule_check();
    }

    fn schedule_check(&mut self) {
        self.timer = Timer::Armed;
        self.is_checking = true;
    }

    /// `setView` once nothing is waiting to be checked.
    fn switch_view(&mut self, next: EditorView) {
        if next == EditorView::File {
            // What the designer's edits write; otherwise the text as it is, comments and all.
            if self.source == Some(Source::Draft)
                && let Some(check) = &self.check
            {
                self.text = check.text.clone();
            }
            self.designer_blocked = None;
            self.view = EditorView::File;
            return;
        }
        if self.source == Some(Source::Text) {
            match self.check.as_ref().and_then(|c| c.draft.clone()) {
                Some(shown) => self.draft = Some(shown),
                None => {
                    self.designer_blocked = Some(DESIGNER_BLOCKED_MESSAGE.into());
                    return;
                }
            }
        } else if self.draft.is_none() {
            self.designer_blocked = Some(DESIGNER_BLOCKED_MESSAGE.into());
            return;
        }
        self.designer_blocked = None;
        self.view = EditorView::Designer;
    }

    /// The body a check sends, and whether it is the text (whose comments the answer then replaces ours with).
    fn check_body(&self) -> (Value, bool) {
        match (&self.source, &self.draft) {
            (Some(Source::Draft), Some(d)) => (json!({"draft": d.to_json()}), false),
            _ => (json!({"text": self.text}), true),
        }
    }
}

fn draft_file(name: &str) -> String {
    format!(".weave/workflows/{}.yaml", slug_of(name))
}

/// A refusal or a failure to ask: its words, and whether it was a 409.
struct Failed {
    message: String,
    conflict: bool,
}

/// One request: the good answer's JSON, or why not. A 409 of a PUT is the file having changed on disk.
fn send(method: &str, base: &str, path: &str, body: &Value, fallback: &str) -> Result<Value, Failed> {
    let (status, text) =
        http::text_with(method, base, path, Some(body)).map_err(|_| Failed { message: fallback.into(), conflict: false })?;
    if !(200..300).contains(&status) {
        return Err(Failed { message: error_from(&text, fallback), conflict: status == 409 && method == "PUT" });
    }
    serde_json::from_str(&text).map_err(|_| Failed { message: fallback.into(), conflict: false })
}

impl State {
    /// `open`: a repository's workflow, read from the server. A failure leaves `load_error`.
    pub fn workflow_editor_open(&mut self, machine: &str, repository: &str, workflow_id: &str) {
        let ed = &mut self.workflows_nav.editor;
        ed.begin(machine, repository);
        ed.drafted = None;
        let opens = ed.opens;
        let body = json!({"directory": repository, "workflowId": workflow_id});
        let Some(link) = self.link_for_machine(machine) else {
            self.workflows_nav.editor.fail_open(OPEN_FALLBACK.into());
            return;
        };
        link.off_thread(
            move |base| send("POST", base, "/api/workflows/files/open", &body, OPEN_FALLBACK),
            move |s, _, result| {
                let ed = &mut s.workflows_nav.editor;
                if ed.opens != opens {
                    return;
                }
                match result {
                    Ok(v) => {
                        let opened = WorkflowFile::from_json(&v);
                        ed.view = if opened.check.draft.is_some() { EditorView::Designer } else { EditorView::File };
                        ed.reset(opened);
                    }
                    Err(f) => ed.fail_open(f.message),
                }
            },
        );
    }

    /// Runs what `flush` held back, now that nothing is left to check.
    fn workflow_editor_then(&mut self, then: Then) {
        match then {
            Then::View(view) => self.workflows_nav.editor.switch_view(view),
            Then::Save(force) => self.workflow_editor_save_now(force),
        }
    }

    /// `flush`: checks now what's waiting, then does `then`. With nothing waiting `then` is done at once.
    fn workflow_editor_flush(&mut self, then: Then) {
        let ed = &mut self.workflows_nav.editor;
        match (ed.timer != Timer::Off, ed.in_flight) {
            (true, _) => {
                ed.timer = Timer::Off;
                ed.waiting.push(then);
                self.workflow_editor_run_check();
            }
            (false, true) => ed.waiting.push(then),
            (false, false) => self.workflow_editor_then(then),
        }
    }

    /// `runCheck`: sends what was edited last to the server; the answer counts only if no newer check was started.
    fn workflow_editor_run_check(&mut self) {
        let ed = &mut self.workflows_nav.editor;
        ed.generation += 1;
        let mine = ed.generation;
        let (body, from_text) = ed.check_body();
        ed.is_checking = true;
        ed.in_flight = true;
        let machine = ed.machine.clone();
        let Some(link) = self.link_for_machine(&machine) else {
            if self.workflows_nav.editor.checked(mine, from_text, Err(CHECK_FALLBACK.into())) {
                self.workflow_editor_release_then();
            }
            return;
        };
        link.off_thread(
            move |base| send("POST", base, "/api/workflows/check", &body, CHECK_FALLBACK).map_err(|f| f.message),
            move |s, _, result| {
                if s.workflows_nav.editor.checked(mine, from_text, result) {
                    s.workflow_editor_release_then();
                }
            },
        );
    }

    /// A check finished: what was waiting for it goes on.
    fn workflow_editor_release_then(&mut self) {
        let held = std::mem::take(&mut self.workflows_nav.editor.waiting);
        for then in held {
            self.workflow_editor_then(then);
        }
    }

    /// `setView`.
    pub fn workflow_editor_set_view(&mut self, next: EditorView) {
        if next == self.workflows_nav.editor.view {
            return;
        }
        self.workflow_editor_flush(Then::View(next));
    }

    /// Called from `poll`: the wait after the last key ends, and the check goes out. True when something changed.
    pub fn keep_workflow_editor(&mut self, now: f64, tx: &mut Tx) -> bool {
        let ed = &mut self.workflows_nav.editor;
        match ed.timer {
            Timer::Off => false,
            Timer::Armed => {
                ed.timer = Timer::At(now + CHECK_DELAY);
                tx.wake_after(Duration::from_secs_f64(CHECK_DELAY));
                false
            }
            Timer::At(due) if now < due => {
                tx.wake_after(Duration::from_secs_f64(due - now));
                false
            }
            Timer::At(_) => {
                ed.timer = Timer::Off;
                self.workflow_editor_run_check();
                true
            }
        }
    }

    /// `save`: saves what was edited last. A draft from the designer is written in Fleet's layout, so a file with comments
    /// asks first (once). The frame hears a save through `take_saved`.
    pub fn workflow_editor_save(&mut self, force: bool) {
        let ed = &self.workflows_nav.editor;
        if ed.file.is_none() || ed.directory.is_none() {
            return;
        }
        self.workflow_editor_flush(Then::Save(force));
    }

    fn workflow_editor_save_now(&mut self, force: bool) {
        let ed = &mut self.workflows_nav.editor;
        if !force && !ed.can_save() {
            return;
        }
        let (from_designer, content) = ed.content();
        if from_designer && !ed.comments.is_empty() && !ed.comments_confirmed {
            ed.asking_to_remove_comments = true;
            return;
        }
        ed.is_saving = true;
        ed.save_error = None;
        let (Some(directory), Some(file)) = (ed.directory.clone(), ed.file.clone()) else { return };
        let machine = ed.machine.clone();
        let drafted = ed.drafted.is_some();
        let opens = ed.opens;
        let (method, body) = if drafted {
            let mut body = json!({"directory": directory});
            body.as_object_mut().expect("an object").extend(content.as_object().cloned().unwrap_or_default());
            ("POST", body)
        } else {
            let mut body = json!({"directory": directory, "workflowId": file.workflow_id, "hash": file.hash, "force": force});
            body.as_object_mut().expect("an object").extend(content.as_object().cloned().unwrap_or_default());
            ("PUT", body)
        };
        let Some(link) = self.link_for_machine(&machine) else {
            let ed = &mut self.workflows_nav.editor;
            ed.is_saving = false;
            ed.save_error = Some(SAVE_FALLBACK.into());
            return;
        };
        link.off_thread(
            move |base| send(method, base, "/api/workflows/files", &body, SAVE_FALLBACK),
            move |s, _, result| {
                let ed = &mut s.workflows_nav.editor;
                if ed.opens != opens {
                    return;
                }
                ed.is_saving = false;
                match result {
                    Ok(v) => {
                        let saved = WorkflowFile::from_json(&v);
                        let keep_view = ed.view;
                        ed.drafted = None;
                        ed.just_saved = Some(Saved { workflow_id: saved.workflow_id.clone(), file: saved.file.clone() });
                        let has_draft = saved.check.draft.is_some();
                        ed.reset(saved);
                        // A saved draft the designer can't show goes to the File view; a draft that was only created follows
                        // the same rule as a file that was saved.
                        if !has_draft && (drafted || keep_view == EditorView::Designer) {
                            ed.view = EditorView::File;
                        }
                        ed.conflict = None;
                    }
                    Err(f) if f.conflict => ed.conflict = Some(f.message),
                    Err(f) => ed.save_error = Some(f.message),
                }
            },
        );
    }

    /// Save and remove them, from the comments confirmation.
    pub fn workflow_editor_confirm_remove_comments(&mut self) {
        let ed = &mut self.workflows_nav.editor;
        ed.asking_to_remove_comments = false;
        ed.comments_confirmed = true;
        self.workflow_editor_save(false);
    }

    /// `keepComments`, from the banner or the confirmation: the text as it was, comments and all. Edits made in the designer
    /// since would be written in Fleet's layout, so they're dropped: make them in the File view instead.
    pub fn workflow_editor_keep_comments(&mut self) {
        let ed = &mut self.workflows_nav.editor;
        ed.asking_to_remove_comments = false;
        let Some(before) = ed.text_before_designer.take().filter(|_| ed.source == Some(Source::Draft)) else {
            return self.workflow_editor_set_view(EditorView::File);
        };
        ed.cancel_check();
        ed.text = before;
        ed.source = if ed.text == ed.saved_text { None } else { Some(Source::Text) };
        if ed.source.is_none() {
            ed.draft = ed.saved_draft.clone();
        }
        ed.designer_blocked = None;
        ed.view = EditorView::File;
        self.workflow_editor_run_check();
    }

    /// `reload`, from the conflict dialog: what's on disk now, and the edits are dropped.
    pub fn workflow_editor_reload(&mut self) {
        let ed = &mut self.workflows_nav.editor;
        ed.conflict = None;
        let (Some(directory), Some(file)) = (ed.directory.clone(), ed.file.as_ref().map(|f| f.workflow_id.clone())) else { return };
        let machine = ed.machine.clone();
        self.workflow_editor_open(&machine, &directory, &file);
    }

    /// `keepMine`, from the conflict dialog: saves over the newer file.
    pub fn workflow_editor_keep_mine(&mut self) {
        self.workflows_nav.editor.conflict = None;
        self.workflow_editor_save(true);
    }
}

impl Editor {
    fn fail_open(&mut self, message: String) {
        self.file = None;
        self.load_error = Some(message);
    }

    /// A check answered. False when it was overtaken (its number is old), true when it was the newest and ends the wait.
    fn checked(&mut self, mine: u64, from_text: bool, result: Result<Value, String>) -> bool {
        if mine != self.generation {
            return false;
        }
        match result {
            Ok(v) => {
                let check = WorkflowCheck::from_json(&v);
                if from_text {
                    self.comments = check.comments.clone();
                }
                self.check = Some(check);
            }
            Err(message) => self.save_error = Some(message),
        }
        self.in_flight = false;
        self.is_checking = false;
        true
    }
}
