//! The New session page's message box and what Enter does (`NewSessionComposer.vue`'s `submit`; rows A4.9, A4.12, A4.20,
//! A5.2-A5.6, A13.1-A13.9, A14.1-A14.2, A14.8). The box's text is `new_box.edit`; the draft's `message` is the copy that
//! outlives the page, and `sync_new_message` keeps the two the same. Starting a session is the state machine
//! guards → request → `is_starting` → answer: nothing here draws, and the one request runs on a thread.
use super::request::{
    Created, Folder, NewSessionRequest, NewSessionState, Workspace, build_create_session_request, build_created_session_row,
};
use crate::composer::choice::AgentModelChoice;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::store;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

/// The key of the message field.
pub const MESSAGE_KEY: &str = "new-session-message";

/// What the box holds that the draft doesn't: the field's own state and the two messages of the red box.
pub struct NewBox {
    pub edit: TextEdit,
    /// The draft's message as of the last `sync_new_message`: a draft that differs was changed from outside (a starter, a
    /// restored draft), and the field takes it.
    seen: String,
    /// `validationError`: cleared by typing, a good submit or a new folder.
    pub validation: Option<String>,
    /// `createError`: the server's words, cleared when the next create starts.
    pub create_error: Option<String>,
    /// The draft the keyboard was last given to, so a new page starts in the box (`focusMessage`).
    focused_for: Option<String>,
    /// When Enter was pressed, in milliseconds: the sent bubble's time.
    pub sent_at: i64,
    /// The `@` list, the picks' folder line and the GitHub place (piece M).
    pub refs: super::refs::NewRefs,
}

impl Default for NewBox {
    fn default() -> Self {
        NewBox {
            edit: TextEdit::new(""),
            seen: String::new(),
            validation: None,
            create_error: None,
            focused_for: None,
            sent_at: 0,
            refs: Default::default(),
        }
    }
}

/// What the answer needs of the moment Enter was pressed (the locals `submit` closes over).
struct Sent {
    request: NewSessionRequest,
    machine: String,
    folder: Folder,
    workspace: Workspace,
    /// The harness and the agent and model sent, when the chips send them.
    choice: Option<(String, AgentModelChoice)>,
    /// `projectForRow`: the project the row goes under.
    project: Option<(String, String)>,
    sent_at: i64,
}

impl State {
    /// The error box: `validationError ?? imageError ?? createError`.
    pub fn new_session_error(&self) -> Option<&str> {
        self.new_box.validation.as_deref().or(self.attach_error()).or(self.new_box.create_error.as_deref())
    }

    /// Whether a session is being made, so the box is read-only and Send shows a spinner (`isStarting`).
    pub fn new_session_starting(&self) -> bool {
        self.new_draft.draft().is_some_and(|d| d.is_starting)
    }

    /// `canSend`: not starting, no folder being made, a harness ready, and a message or an attached item.
    pub fn can_send_new_session(&self) -> bool {
        let Some(d) = self.new_draft.draft() else { return false };
        !d.is_starting
            && !self.folder_clone_busy()
            && self.harnesses_here().no_harness_reason().is_none()
            && (!d.message.trim().is_empty() || d.github_preset.is_some())
    }

    /// "Start without a message" shows with a folder chosen and nothing to send (`folder && !hasMessage && !gitHubPreset`).
    pub fn offers_start_without_message(&self) -> bool {
        self.new_draft.draft().is_some_and(|d| d.folder.is_some() && d.message.trim().is_empty() && d.github_preset.is_none())
    }

    /// The words sent while the session starts, shown as the conversation's first message (`sentMessage`).
    pub fn sent_message(&self) -> Option<(String, i64)> {
        let d = self.new_draft.draft().filter(|d| d.is_starting && !d.message.trim().is_empty())?;
        Some((d.message.clone(), self.new_box.sent_at))
    }

    /// Keeps the field and the draft's message the same; whether anything moved (so the page is drawn again).
    pub fn sync_new_message(&mut self) -> bool {
        let Some(draft) = self.new_draft.draft_mut() else { return false };
        let b = &mut self.new_box;
        if draft.message != b.seen {
            b.edit = TextEdit::new(draft.message.clone());
            b.seen = draft.message.clone();
            if let Some((from, to)) = self.starters.pending_selection.take() {
                b.edit.set_selection(from, to);
            }
            return true;
        }
        if b.edit.text != draft.message {
            draft.message = b.edit.text.clone();
            b.seen = draft.message.clone();
            // Typing clears the validation message (`handleInput`).
            b.validation = None;
            return true;
        }
        false
    }

    /// The box takes the keyboard when a page opens, after a starter fills it, and after a failed create (`focusMessage`).
    pub fn keep_new_message(&mut self, tx: &mut Tx) -> bool {
        if !self.on_new_session_page() {
            return false;
        }
        let moved = self.sync_new_message();
        let Some(key) = self.new_draft.draft().map(|d| d.row_key.clone()) else { return moved };
        let fresh = self.new_box.focused_for.as_ref() != Some(&key);
        if fresh || std::mem::take(&mut self.starters.focus_box) {
            self.new_box.focused_for = Some(key);
            tx.focus(MESSAGE_KEY, false);
        }
        moved
    }

    /// A new page has no old errors (they belong to one mounting of the page in Fleet).
    pub(crate) fn reset_new_box_errors(&mut self) {
        self.new_box.validation = None;
        self.new_box.create_error = None;
    }

    /// `projectForRow`: the project the draft chose, else Scratch (where the server puts a session with none).
    fn project_for_row(&self, chosen: Option<&str>) -> Option<(String, String)> {
        let projects = &self.project_load.projects;
        let picked = chosen.and_then(|id| projects.iter().find(|p| p.id == id));
        let scratch = || projects.iter().find(|p| p.kind == crate::fleet::groups::ProjectKind::Scratch);
        picked.or_else(scratch).map(|p| (p.id.clone(), p.name.clone()))
    }

    /// Enter, the Send button and "Start without a message" (`submit`).
    pub fn submit_new_session(&mut self, tx: &mut Tx, without_message: bool) {
        self.sync_new_message();
        let Some(d) = self.new_draft.draft().cloned() else { return };
        if !self.on_new_session_page() || d.is_starting || self.folder_clone_busy() || self.harnesses_here().no_harness_reason().is_some() {
            return;
        }
        // The first Enter asks where to run (A5.6): the message stays.
        let Some(folder) = d.folder.clone() else {
            self.open_folder_menu(tx);
            return;
        };
        if !without_message && d.message.trim().is_empty() && d.github_preset.is_none() {
            return;
        }
        let picked = self.sent_choices();
        let mut state = NewSessionState::new(folder.clone(), d.workspace.clone());
        state.message = if without_message { String::new() } else { d.message.clone() };
        state.title = Some(d.title.clone());
        state.project_id = d.project_id.clone();
        state.tags = d.tags.split(',').map(str::to_string).collect();
        state.harness_type = picked.harness_type;
        state.harness_profile_id = picked.harness_profile_id;
        state.github_preset = d.github_preset.clone();
        state.session_references = self.new_session_references(&state.message);
        state.attachments = self.new_session_images();
        state.branch = Some(d.branch_name.clone()).filter(|b| !b.is_empty());
        state.base_branch = d.base_branch.clone();
        state.fetch_origin = d.fetch_origin;
        state.agent = picked.agent;
        state.model = picked.model;
        let request = match build_create_session_request(&state) {
            Ok(request) => request,
            Err(why) => {
                self.new_box.validation = Some(why);
                return;
            }
        };
        self.new_box.validation = None;
        self.new_box.create_error = None;
        self.new_box.refs.notice = None;
        let machine = self.target_key();
        let Some(link) = self.link_for_machine(&machine) else {
            self.new_box.create_error = Some("There is no Fleet to start a session on.".into());
            return;
        };
        self.new_box.sent_at = crate::fleet::link::now_ms();
        let sent = Sent {
            request: request.clone(),
            machine,
            folder,
            workspace: d.workspace.clone(),
            choice: self
                .sends_agent_and_model()
                .then(|| (self.resolved_harness(), AgentModelChoice { agent: d.agent.clone(), model: d.model.clone() })),
            project: self.project_for_row(d.project_id.as_deref()),
            sent_at: self.new_box.sent_at,
        };
        // The message moves into the conversation at once; the session catches up (A13.4).
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.is_starting = true;
        }
        link.create_session(&request, move |s, tx, result| s.finish_new_session(tx, result, sent));
    }

    /// The answer to `POST /api/sessions`.
    fn finish_new_session(&mut self, tx: &mut Tx, result: Result<Value, String>, sent: Sent) {
        let created = result.and_then(|answer| Created::from_wire(&answer).ok_or_else(|| "Failed to create session".to_string()));
        let created = match created {
            Ok(created) => created,
            Err(why) => return self.fail_new_session(tx, why),
        };
        let id = created.session_id().to_string();
        // Remembered only now: a refused create remembers nothing (A13.9).
        let choice = sent.choice.as_ref().map(|(harness, c)| (harness.as_str(), c));
        self.remember_new_session(&sent.machine, &sent.folder, &sent.workspace, choice);
        let live = sent.machine == HOME_MACHINE_KEY;
        if !live {
            self.machines.remember_sessions(&sent.machine, std::slice::from_ref(&id));
        }
        // All in one tick: the session page has the message and its row before the history loads.
        if let Some(message) = &sent.request.options.initial_prompt {
            self.prompts.add(&id, message, sent.sent_at, self.clock);
        }
        let project = sent.project.as_ref().map(|(id, name)| (id.as_str(), name.as_str()));
        let row = build_created_session_row(&created, &sent.request, project);
        let Some(session) = crate::fleet::sessions::parse_sessions(&json!([row]), sent.sent_at).into_iter().next() else {
            return self.fail_new_session(tx, "Failed to create session".into());
        };
        if live {
            store::upsert(&mut self.projects, session);
        } else {
            self.store.upsert_elsewhere(&sent.machine, session.row_copy());
            self.machines.upsert_session(&sent.machine, session);
        }
        self.forget_new_refs();
        self.new_draft.hand_off(&id);
        // Someone who left while it started stays where they went; the row is there for them.
        if self.new_draft.is_page_open() {
            self.open_created(&sent.machine, &id, tx);
        }
    }

    fn open_created(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        if machine != HOME_MACHINE_KEY {
            self.open_machine_session(machine, id, tx);
            return;
        }
        let at = self.projects.iter().enumerate().find_map(|(p, project)| project.sessions.iter().position(|s| s.id == id).map(|i| (p, i)));
        if let Some((p, i)) = at {
            self.open_session(p, i, tx);
        }
    }

    /// The create was refused: the message is still in the box, the server's words go in the red box (A13.9).
    fn fail_new_session(&mut self, tx: &mut Tx, why: String) {
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.is_starting = false;
        }
        self.new_box.create_error = Some(why);
        if self.new_draft.is_page_open() {
            tx.focus(MESSAGE_KEY, false);
        } else {
            // Left while it started: a draft with nothing in it goes now.
            self.new_draft.drop_if_empty();
        }
    }
}

/// What a picture of the box shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendScene {
    /// A folder chosen and a message typed.
    Typed,
    /// Enter pressed, the session not there yet: the message sits in the conversation, the list row says Starting….
    Starting,
    /// The server refused: the red box over the message, which is back in the box.
    Refused,
}

impl State {
    /// Scenes for the pictures: the page with a folder and a message, then what Enter and a refusal do to it.
    pub(crate) fn show_new_session_send(&mut self, scene: SendScene) {
        self.open_new_session_page(None);
        self.set_folder(Folder::Repository(crate::mock::feeds::ROCKET.into()), true);
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.message = "Fix the login redirect after sign-in".into();
            draft.is_starting = scene == SendScene::Starting;
        }
        self.new_box.sent_at = crate::view::message::now_ms(self);
        if scene == SendScene::Refused {
            self.new_box.create_error = Some("Couldn't create the worktree: 'x' is not a valid branch name".into());
        }
        self.sync_new_message();
    }
}
