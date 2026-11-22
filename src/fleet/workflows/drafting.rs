//! Asking the model for a workflow (component 18, piece S; `useWorkflowsNav` `startDraft`, `cancelDraft`, `forgetDraft`,
//! `draftSaved`, and `stores/workflows.ts` `draftFromSession` and `draftFromDescription`). Two ways in: Save as workflow… on a
//! session row, and New workflow's Describe it. Either asks once, off the record, and nothing is written: the answer opens in the
//! designer unsaved (`Editor::adopt_draft`). One ask at a time: each has a number, and an answer whose number is not the newest
//! is dropped, which is how Cancel and a second ask stop the first (the server deletes whatever it made for it either way).
use super::draft::DraftedWorkflow;
use super::frame::Leave;
use super::nav::{BUILD_A_FEATURE, Nav};
use super::runs::error_from;
use crate::State;
use crate::fleet::http;
use crate::fleet::new_session::request::Folder;
use serde_json::{Value, json};

/// The library row's id, and the active id, of a drafted workflow that hasn't been saved yet.
pub const DRAFT_ID: &str = "draft:new";

const FROM_SESSION: &str = "Couldn't draft a workflow from this session.";
const FROM_DESCRIPTION: &str = "Couldn't draft the workflow.";
/// "It takes up to a minute or two": the model's own time, with room over it.
const WAIT_SECONDS: u64 = 300;

/// What is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Save as workflow…: the session's id and its title.
    Session { id: String, title: String },
    /// Describe it: the repository the file goes in, the words, and the harness picked (none: the server's choice).
    Description { directory: String, description: String, harness_type: Option<String> },
}

impl Ask {
    /// The session it reads, when it reads one (`sessionTitle`).
    pub fn session_title(&self) -> Option<&str> {
        match self {
            Ask::Session { title, .. } => Some(title),
            Ask::Description { .. } => None,
        }
    }

    /// The route, its body and the words for a failure that gave none.
    pub fn request(&self) -> (&'static str, Value, &'static str) {
        match self {
            Ask::Session { id, .. } => ("/api/workflows/drafts/from-session", json!({"sessionId": id}), FROM_SESSION),
            Ask::Description { directory, description, harness_type } => (
                "/api/workflows/drafts/from-description",
                json!({"directory": directory, "description": description, "harnessType": harness_type}),
                FROM_DESCRIPTION,
            ),
        }
    }
}

/// `drafting`: the ask on its way, or the words of its failure (`retry` asks the same again).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drafting {
    pub ask: Ask,
    pub error: Option<String>,
}

/// A draft with no Fleet to ask (the app on its invented data): the workflow of the invented repository, as a model would give it.
fn invented(ask: &Ask) -> DraftedWorkflow {
    let file = crate::mock::workflow_editor::file("repo:tidy-up-a-flaky-test");
    DraftedWorkflow {
        repository: crate::mock::workflows::REPOSITORY.into(),
        repository_name: "weave-fleet".into(),
        session_title: ask.session_title().map(str::to_string),
        check: file.check,
        asks: 1,
        tokens: Some(super::draft::DraftTokens { total: 3412, from_cache: 3100 }),
    }
}

impl Nav {
    /// `forgetDraft`: nothing is asked and nothing is held. A drafted workflow open in the editor goes with it.
    pub fn forget_draft(&mut self) {
        self.draft_run += 1;
        self.drafting = None;
        self.drafted = None;
        if self.editor.drafted.is_some() {
            self.editor.cancel_check();
            self.editor = Default::default();
        }
    }

    /// The draft's pane is the page's (what the scenes and tests set; a real ask does it in `workflow_draft_now`).
    pub fn show_draft(&mut self) {
        self.active = Some(DRAFT_ID.to_string());
        self.showing_runs = false;
    }

    /// `cancelDraft`: Cancel and Close, back to the first workflow of the library.
    pub fn cancel_draft(&mut self) {
        self.forget_draft();
        self.active = Some(self.library.as_ref().and_then(|l| l.workflows.first()).map_or(BUILD_A_FEATURE, |w| w.id.as_str()).to_string());
    }

    /// `draftSaved`: the draft is the repository's workflow now.
    pub fn draft_saved(&mut self, workflow_id: &str) {
        self.forget_draft();
        self.active = Some(workflow_id.to_string());
    }
}

impl State {
    /// Save as workflow… on a session (`draftFromSession`).
    pub fn workflow_draft_from_session(&mut self, id: &str, title: &str) {
        self.workflow_start_draft(Ask::Session { id: id.to_string(), title: title.to_string() });
    }

    /// `startDraft`: through the leave guard, which asks about unsaved edits, unless a draft is what is open already.
    pub fn workflow_start_draft(&mut self, ask: Ask) {
        if self.workflows_nav.active_id() == DRAFT_ID {
            self.workflow_draft_now(ask);
            return;
        }
        if self.workflows_nav.leave_guard.is_some_and(|guard| !guard(self)) {
            return;
        }
        self.workflow_leave(Leave::Draft(ask));
    }

    /// The ask itself: the draft row appears, the pane says it is drafting, the request goes.
    pub fn workflow_draft_now(&mut self, ask: Ask) {
        let machine = self.workflows_machine();
        let nav = &mut self.workflows_nav;
        nav.draft_run += 1;
        let run = nav.draft_run;
        nav.drafted = None;
        nav.drafting = Some(Drafting { ask: ask.clone(), error: None });
        nav.show_draft();
        if self.link.is_none() {
            self.workflow_drafted(run, &machine, invented(&ask));
            return;
        }
        let (path, body, fallback) = ask.request();
        let Some(link) = self.link_for_machine(&machine) else {
            self.workflow_draft_failed(run, fallback.to_string());
            return;
        };
        link.off_thread(
            move |base| {
                let (status, text) = http::text_within("POST", base, path, Some(&body), WAIT_SECONDS).map_err(|_| fallback.to_string())?;
                if !(200..300).contains(&status) {
                    return Err(error_from(&text, fallback));
                }
                serde_json::from_str::<Value>(&text).map_err(|_| fallback.to_string())
            },
            move |s, _, result| match result {
                Ok(v) => s.workflow_drafted(run, &machine, DraftedWorkflow::from_json(&v)),
                Err(message) => s.workflow_draft_failed(run, message),
            },
        );
    }

    /// Try again: the same ask.
    pub fn workflow_draft_retry(&mut self) {
        if let Some(d) = self.workflows_nav.drafting.clone() {
            self.workflow_draft_now(d.ask);
        }
    }

    /// The answer: the draft's repository becomes the Run box's folder, and the draft opens unsaved.
    fn workflow_drafted(&mut self, run: u64, machine: &str, drafted: DraftedWorkflow) {
        let nav = &mut self.workflows_nav;
        nav.draft_answers += 1;
        if run != nav.draft_run || nav.drafting.is_none() {
            return;
        }
        nav.drafting = None;
        nav.set_folder(Folder::Repository(drafted.repository.clone()), true);
        nav.drafted = Some(drafted.clone());
        nav.editor.adopt_draft(machine, drafted);
    }

    fn workflow_draft_failed(&mut self, run: u64, message: String) {
        let nav = &mut self.workflows_nav;
        nav.draft_answers += 1;
        if run != nav.draft_run {
            return;
        }
        if let Some(d) = nav.drafting.as_mut() {
            d.error = Some(message);
        }
    }
}
