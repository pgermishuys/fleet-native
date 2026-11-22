//! New workflow and Duplicate (component 18, piece R; `WorkflowCreateDialog.vue`, `useWorkflowsNav` `creating`, and
//! `WorkflowDetailPanel`'s `onCreated`): a name, the file Fleet makes in the repository's `.weave/workflows/`, and what happens
//! when the server has made it: the file opens in the designer without being read again and the library reloads. A name that's
//! taken is refused with the server's own words and the dialog stays open. The dialog is drawn in `view::workflows::create`.
use super::draft::{WorkflowDraft, WorkflowFile, slug_of};
use super::runs::error_from;
use super::{Step, StepKind, Workflow};
use crate::fleet::http;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

pub const NAME_FIELD: &str = "workflow-create-name";
pub const DESCRIBE_FIELD: &str = "workflow-describe-text";
const FALLBACK: &str = "Couldn't create the workflow file.";

/// A built-in being copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub name: String,
}

/// The two ways into a new workflow; Duplicate has only the first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Blank,
    /// The model drafts the workflow from a sentence (`workflow-describe-form`).
    Describe,
}

/// The dialog while it is open (`nav.creating`, and the dialog's own refs).
#[derive(Debug)]
pub struct Create {
    pub source: Option<Source>,
    pub name: TextEdit,
    pub mode: Mode,
    pub error: Option<String>,
    pub creating: bool,
    /// Describe it: what the workflow should do, the harness picked (none: the default), and its list open.
    pub description: TextEdit,
    pub harness_choice: Option<String>,
    pub picking_harness: bool,
}

impl Create {
    /// A Duplicate starts as "<name>, our way" (the `watch(open)`).
    pub fn new(source: Option<Source>) -> Create {
        let name = source.as_ref().map(|s| format!("{}, our way", s.name)).unwrap_or_default();
        Create {
            source,
            name: TextEdit::new(name),
            mode: Mode::Blank,
            error: None,
            creating: false,
            description: TextEdit::default(),
            harness_choice: None,
            picking_harness: false,
        }
    }

    pub fn trimmed_name(&self) -> &str {
        self.name.text.trim()
    }

    /// The file's path as the dialog shows it (`file`).
    pub fn file(&self, repository_name: Option<&str>) -> String {
        format!("{}/.weave/workflows/{}.yaml", repository_name.unwrap_or("this repo"), slug_of(&self.name.text))
    }
}

/// The body of `POST /api/workflows/files` (`createFile`): the trimmed name, and the built-in's id for a Duplicate.
pub fn create_body(directory: &str, name: &str, source: Option<&Source>) -> Value {
    json!({"directory": directory, "name": name.trim(), "workflowId": source.map(|s| s.id.as_str())})
}

/// What the server makes with no server to ask (the app on its invented data): a file with one agent step.
fn invented(name: &str, source: Option<&Source>) -> WorkflowFile {
    let slug = slug_of(name);
    let from = source.and_then(|s| crate::mock::workflows::library(None).workflows.into_iter().find(|w| w.id == s.id));
    let steps = from.map(|w| w.steps).filter(|s| !s.is_empty()).unwrap_or_else(|| {
        vec![Step {
            id: "step-1".into(),
            title: "Step 1".into(),
            kind: StepKind::Agent,
            prompt: Some("Say what to do.".into()),
            ..Step::default()
        }]
    });
    let draft = WorkflowDraft { name: name.trim().to_string(), description: None, placeholder: None, steps };
    WorkflowFile {
        workflow_id: format!("repo:{slug}"),
        file: format!(".weave/workflows/{slug}.yaml"),
        hash: "h0".into(),
        check: super::draft::WorkflowCheck {
            text: format!("name: {}\n", draft.name),
            errors: Vec::new(),
            draft: Some(draft),
            comments: Vec::new(),
        },
    }
}

impl State {
    /// New workflow, or Duplicate when a built-in is given (`startCreate`). Nothing opens without a repository: the buttons
    /// that ask are off, and a key can't reach past them.
    pub fn workflow_create_open(&mut self, source: Option<Source>, tx: &mut Tx) {
        if self.workflows_nav.repository_path().is_none() {
            return;
        }
        self.workflows_nav.create = Some(Create::new(source));
        self.ui.open = None;
        tx.focus(NAME_FIELD, false);
    }

    /// Cancel, the X, Escape and the scrim (`endCreate`). Not while the file is being made.
    pub fn workflow_create_close(&mut self) {
        // An open select closes alone, as a browser's list does on Escape.
        if let Some(c) = self.workflows_nav.create.as_mut().filter(|c| c.picking_harness) {
            c.picking_harness = false;
        } else if self.workflows_nav.create.as_ref().is_some_and(|c| !c.creating) {
            self.workflows_nav.create = None;
        }
    }

    /// Typing clears a refusal's words: the next try is a new one.
    pub fn workflow_create_edited(&mut self) {
        if let Some(c) = self.workflows_nav.create.as_mut() {
            c.error = None;
        }
    }

    pub fn workflow_create_set_mode(&mut self, mode: Mode, tx: &mut Tx) {
        if let Some(c) = self.workflows_nav.create.as_mut().filter(|c| c.source.is_none()) {
            c.mode = mode;
            c.picking_harness = false;
            tx.focus(if mode == Mode::Describe { DESCRIBE_FIELD } else { NAME_FIELD }, false);
        }
    }

    /// The harnesses Describe it can ask: those workflows run on that can be asked off the record, as (type, name).
    pub fn workflow_describe_harnesses(&self) -> Vec<(String, String)> {
        let all = self.run_box_harnesses();
        all.enabled()
            .into_iter()
            .filter(|h| h.supports_workflow_steps && h.supports_off_the_record)
            .map(|h| (h.kind.clone(), h.display_name.clone()))
            .collect()
    }

    /// `harnessType`: the one picked; else the default when it is one of them, else the first. None with no harness to ask.
    pub fn workflow_describe_harness(&self) -> Option<String> {
        let list = self.workflow_describe_harnesses();
        if let Some(picked) = self.workflows_nav.create.as_ref().and_then(|c| c.harness_choice.clone()) {
            return Some(picked);
        }
        let default = self.run_box_harnesses().default_type();
        if list.iter().any(|(kind, _)| *kind == default) { Some(default) } else { list.into_iter().next().map(|(kind, _)| kind) }
    }

    /// `standardModel`: "On your Standard model: …" says this one.
    pub fn workflow_describe_model(&self, harness: &str) -> String {
        super::helpers::model_short_name(self.role_choice(harness, super::Role::Standard).model.as_deref())
    }

    /// `canDescribe`.
    pub fn workflow_can_describe(&self) -> bool {
        let words = self.workflows_nav.create.as_ref().is_some_and(|c| !c.description.text.trim().is_empty());
        words && self.workflows_nav.repository_path().is_some() && self.workflow_describe_harness().is_some()
    }

    /// `describe`: the dialog closes and the model is asked. Nothing is created: the draft opens unsaved.
    pub fn workflow_describe_submit(&mut self) {
        if !self.workflow_can_describe() {
            return;
        }
        let harness_type = self.workflow_describe_harness();
        let Some(directory) = self.workflows_nav.repository_path().map(str::to_string) else { return };
        let Some(c) = self.workflows_nav.create.take() else { return };
        self.workflow_start_draft(super::drafting::Ask::Description {
            directory,
            description: c.description.text.trim().to_string(),
            harness_type,
        });
    }

    /// `canCreate`.
    pub fn workflow_can_create(&self) -> bool {
        let nav = &self.workflows_nav;
        nav.repository_path().is_some() && nav.create.as_ref().is_some_and(|c| !c.creating && !c.trimmed_name().is_empty())
    }

    /// `create`: asks the server for the file; the answer opens it. A refusal's words stay in the dialog.
    pub fn workflow_create_submit(&mut self) {
        if !self.workflow_can_create() {
            return;
        }
        let machine = self.workflows_machine();
        let nav = &mut self.workflows_nav;
        let Some(repository) = nav.repository_path().map(str::to_string) else { return };
        let Some(c) = nav.create.as_mut() else { return };
        c.creating = true;
        c.error = None;
        let (name, source) = (c.name.text.trim().to_string(), c.source.clone());
        if self.link.is_none() {
            let created = invented(&name, source.as_ref());
            self.workflow_created(&machine, &repository, created, false);
            return;
        }
        let body = create_body(&repository, &name, source.as_ref());
        let Some(link) = self.link_for_machine(&machine) else {
            self.workflow_create_failed(FALLBACK.into());
            return;
        };
        link.off_thread(
            move |base| {
                let (status, text) =
                    http::text_with("POST", base, "/api/workflows/files", Some(&body)).map_err(|_| FALLBACK.to_string())?;
                if !(200..300).contains(&status) {
                    return Err(error_from(&text, FALLBACK));
                }
                serde_json::from_str::<Value>(&text).map_err(|_| FALLBACK.to_string())
            },
            move |s, _, result| match result {
                Ok(v) => s.workflow_created(&machine, &repository, WorkflowFile::from_json(&v), true),
                Err(message) => s.workflow_create_failed(message),
            },
        );
    }

    fn workflow_create_failed(&mut self, message: String) {
        if let Some(c) = self.workflows_nav.create.as_mut() {
            c.creating = false;
            c.error = Some(message);
        }
    }

    /// `onCreated`: the dialog closes, the file opens in the designer without being read again, the workflow is the open one and
    /// the library reloads so the list has the new row. The repository the dialog was opened for must still be the one picked.
    fn workflow_created(&mut self, machine: &str, repository: &str, created: WorkflowFile, reload: bool) {
        let nav = &mut self.workflows_nav;
        nav.create = None;
        if nav.repository_path() != Some(repository) {
            return;
        }
        let id = created.workflow_id.clone();
        if !reload && let Some(library) = nav.library.as_mut() {
            // No server to read the library from: the new row is added to the invented one.
            library.workflows.push(Workflow {
                id: id.clone(),
                file: Some(created.file.clone()),
                name: created.check.draft.as_ref().map(|d| d.name.clone()).unwrap_or_default(),
                steps: created.check.draft.as_ref().map(|d| d.steps.clone()).unwrap_or_default(),
                ..Workflow::default()
            });
        }
        nav.editor.adopt(machine, repository, created);
        nav.move_to(&id);
        nav.showing_runs = false;
        nav.reload_wanted = reload;
    }
}
