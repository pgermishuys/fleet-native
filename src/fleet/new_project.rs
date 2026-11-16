//! `NewProjectDialog.vue` and `useCreateProject` (`use-session-actions.ts`): the dialog's state, the request and what a
//! created project does to the list. The dialog is drawn in `view::new_project`.
//! The request runs on a thread; with no server (scenes, tests) the invented list takes the project at once.
use super::settings::HOME_MACHINE_KEY;
use super::{http, store};
use crate::mock::Project;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

pub const NAME_FIELD: &str = "new-project-name";
pub const DESCRIPTION_FIELD: &str = "new-project-description";

/// `POST /api/projects`.
pub const PATH: &str = "/api/projects";

/// What the server is told: the trimmed name, and the description only when it has words (`undefined` leaves it out).
pub fn create_body(name: &str, description: &str) -> Value {
    let description = description.trim();
    if description.is_empty() { json!({ "name": name.trim() }) } else { json!({ "name": name.trim(), "description": description }) }
}

/// What a project that couldn't be reached says (`NewProjectDialog.vue` `handleSubmit`).
pub fn unreachable(machine: &str) -> String {
    format!("Can't reach {machine}. The project wasn't created.")
}

/// Why a create failed: the machine didn't answer at all, or it answered with a refusal.
#[derive(Clone, PartialEq, Debug)]
pub enum Failure {
    /// The request itself failed (Fleet's `TypeError`).
    Unreached(String),
    Refused(String),
}

/// Makes the request, blocking: run it on a thread. A refusal gives the server's own words.
pub fn request(base: &str, body: &Value) -> Result<Value, Failure> {
    let (status, text) = http::text_with("POST", base, PATH, Some(body)).map_err(Failure::Unreached)?;
    if !(200..300).contains(&status) {
        return Err(Failure::Refused(store::error_text(status, &text)));
    }
    Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// The dialog while it is open.
pub struct NewProject {
    pub name: TextEdit,
    pub description: TextEdit,
    /// The machine the project is created on (a machine's key; home's is `HOME_MACHINE_KEY`).
    pub machine: String,
    pub error: Option<String>,
    pub creating: bool,
    /// The "Create on" menu is open.
    pub picking: bool,
}

impl NewProject {
    pub fn trimmed_name(&self) -> &str {
        self.name.text.trim()
    }

    /// `canSubmit`.
    pub fn can_submit(&self) -> bool {
        !self.creating && !self.trimmed_name().is_empty()
    }
}

impl State {
    /// Opens the dialog, empty, on `machine` or else the machine you're on, with the name focused.
    pub fn open_new_project(&mut self, machine: Option<&str>, tx: &mut Tx) {
        let machine = machine.map_or_else(|| self.machines.current_key().to_string(), str::to_string);
        self.proj.new = Some(NewProject {
            name: TextEdit::default(),
            description: TextEdit::default(),
            machine,
            error: None,
            creating: false,
            picking: false,
        });
        self.ui.open = None;
        tx.focus(NAME_FIELD, false);
    }

    /// Cancel, the X, Escape and the scrim. Not while the project is being made.
    pub fn close_new_project(&mut self) {
        if self.proj.new.as_ref().is_some_and(|d| d.picking) {
            if let Some(d) = self.proj.new.as_mut() {
                d.picking = false;
            }
            return;
        }
        if self.proj.new.as_ref().is_some_and(|d| !d.creating) {
            self.proj.new = None;
        }
    }

    /// Typing clears an error (`watch(name)`, `watch(description)`); the field's own handler calls this.
    pub fn new_project_edited(&mut self) {
        if let Some(d) = self.proj.new.as_mut() {
            d.error = None;
        }
    }

    pub fn toggle_project_machine_menu(&mut self) {
        if let Some(d) = self.proj.new.as_mut().filter(|d| !d.creating) {
            d.picking = !d.picking;
        }
    }

    /// `pickMachine`: an unreachable machine can't be picked, and the error is gone.
    pub fn pick_project_machine(&mut self, key: &str) {
        if !self.machine_rows().iter().any(|r| r.key == key && r.reachable) {
            return;
        }
        if let Some(d) = self.proj.new.as_mut() {
            d.machine = key.to_string();
            d.picking = false;
            d.error = None;
        }
    }

    /// Add a machine… closes the dialog and goes to Settings (`addMachine`).
    pub fn add_machine_from_project(&mut self, tx: &mut Tx) {
        self.proj.new = None;
        self.add_machine();
        self.toast("Adding a machine is in Settings, which isn't in the native app yet (component 16).".into(), None, 4.0, tx);
    }

    /// Create (Enter in the name does the same): nothing for a blank name or while one is being made (`handleSubmit`).
    pub fn new_project_submit(&mut self, tx: &mut Tx) {
        let Some(d) = self.proj.new.as_mut().filter(|d| d.can_submit()) else { return };
        d.creating = true;
        d.error = None;
        d.picking = false;
        let body = create_body(&d.name.text, &d.description.text);
        let machine = d.machine.clone();
        // What a failed fetch says is decided now: the machine list can change while the request is out.
        let unreached = self.machines.has_machines().then(|| {
            unreachable(&self.machines.entries().into_iter().find(|e| e.key == machine).map_or_else(|| machine.clone(), |e| e.name))
        });
        let link = if machine == HOME_MACHINE_KEY { self.link.clone() } else { self.link_for_machine(&machine) };
        let Some(link) = link else {
            // No server: the invented list takes it, so a scene or a test needs none.
            self.new_project_done(&machine, &body, unreached, Ok(Value::Null), tx);
            return;
        };
        link.off_thread(
            move |base| request(base, &body),
            move |s, tx, result| s.new_project_done(&machine, &Value::Null, unreached, result, tx),
        );
    }

    /// The server answered. A failure keeps the dialog open, with what was typed; a success closes it, unfolds that machine's
    /// heading and reads its list again (`handleProjectCreated`).
    pub(crate) fn new_project_done(
        &mut self,
        machine: &str,
        sent: &Value,
        unreached: Option<String>,
        result: Result<Value, Failure>,
        tx: &mut Tx,
    ) {
        let Some(d) = self.proj.new.as_mut() else { return };
        d.creating = false;
        match result {
            Err(why) => {
                d.error = Some(match why {
                    Failure::Refused(words) => words,
                    Failure::Unreached(words) => unreached.unwrap_or(words),
                });
            }
            Ok(_) => {
                self.proj.new = None;
                if self.machines.has_machines() && !self.machine_is_open(machine) {
                    self.toggle_machine(machine);
                }
                if self.link.is_some() {
                    self.list_again_on(machine);
                } else if machine == HOME_MACHINE_KEY {
                    self.add_invented_project(sent, tx);
                }
            }
        }
    }

    /// With no server the invented list gets the project, last before Ungrouped.
    fn add_invented_project(&mut self, sent: &Value, tx: &mut Tx) {
        let mut projects = std::mem::take(&mut self.projects);
        let name = sent.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let at = projects.iter().position(|p| p.id == super::groups::UNGROUPED_ID).unwrap_or(projects.len());
        let position = projects.iter().filter(|p| p.id != super::groups::UNGROUPED_ID).map(|p| p.position).max().unwrap_or(0) + 1;
        let project = Project {
            id: format!("mock.project.{}", projects.len()),
            kind: Default::default(),
            position,
            name,
            path: String::new(),
            open: true,
            sessions: Vec::new(),
        };
        projects.insert(at, project);
        self.on_list(projects, tx);
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

/// A state of the dialog for the pictures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Empty,
    Filled,
    Creating,
    Failed,
    OnMachine,
    Picking,
}

impl State {
    pub fn show_new_project(&mut self, scene: Scene) {
        let (name, description) = match scene {
            Scene::Empty => ("", ""),
            _ => ("Release pipeline", "Builds, checks and the notes for each release."),
        };
        let machine = if matches!(scene, Scene::OnMachine | Scene::Picking) {
            self.machines.entries().get(1).map(|e| e.key.clone()).unwrap_or_else(|| HOME_MACHINE_KEY.into())
        } else {
            self.machines.current_key().to_string()
        };
        self.proj.new = Some(NewProject {
            name: TextEdit::new(name),
            description: TextEdit::new(description),
            machine,
            error: matches!(scene, Scene::Failed).then(|| "A project with this name already exists.".into()),
            creating: matches!(scene, Scene::Creating),
            picking: matches!(scene, Scene::Picking),
        });
        if let Some(d) = self.proj.new.as_mut()
            && matches!(scene, Scene::Empty | Scene::Filled | Scene::Failed)
        {
            d.name.focused = true;
        }
    }
}
