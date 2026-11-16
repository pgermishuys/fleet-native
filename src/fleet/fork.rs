//! Fork (`ForkSessionDialog.vue`, `useForkSession` in `use-session-actions.ts`): the dialog's state, the request, and what a
//! fork does to the list once the server has made it. The dialog is drawn in `view::fork`.
//! The request runs on a thread; with no server (scenes, the invented list) the fork is made from the invented row, so
//! nothing here needs one.
use super::settings::HOME_MACHINE_KEY as HOME_KEY;
use super::{http, row_status::display_title, sessions, store};
use crate::mock::Session;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

/// The title field's key.
pub const TITLE_FIELD: &str = "fork-session-title";

/// What the server falls back to when a fork is given no title.
pub const FAILED: &str = "Failed to fork session";

/// `POST /api/sessions/{id}/fork`.
pub fn fork_path(id: &str) -> String {
    format!("/api/sessions/{}/fork", crate::conv::image_sniff::encode_component(id))
}

/// `{ title }`: a blank title is sent as `null`, so the server names the fork.
pub fn fork_body(typed: &str) -> Value {
    let title = typed.trim();
    json!({ "title": if title.is_empty() { Value::Null } else { Value::from(title) } })
}

/// The source's title as the dialog says it.
pub fn source_title(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() { "Untitled session".into() } else { t.to_string() }
}

/// What the field holds when the dialog opens: what the server calls a fork given no title.
pub fn prefill(raw: &str) -> String {
    format!("Fork of {}", source_title(raw))
}

/// The dialog while it is open.
pub struct ForkDialog {
    pub session: String,
    pub source_title: String,
    pub title: TextEdit,
    pub forking: bool,
    pub error: Option<String>,
}

impl ForkDialog {
    pub fn open(session: &str, raw_title: &str) -> ForkDialog {
        ForkDialog {
            session: session.to_string(),
            source_title: source_title(raw_title),
            title: TextEdit::new(prefill(raw_title)),
            forking: false,
            error: None,
        }
    }
}

/// `buildForkedSessionListItem` and what `forkSession` adds to it: a list item in the shape `GET /api/sessions` sends, so the
/// list's own parser reads it. `source` is the row the fork was made from; `answer` is the server's reply.
pub fn forked_row(source: &Session, answer: &Value, source_id: &str) -> Value {
    let session = answer.get("session").cloned().unwrap_or(Value::Null);
    let directory = &source.listing.workspace_directory;
    let tags = session.get("tags").cloned().unwrap_or_else(|| json!([]));
    json!({
        "instanceId": answer.get("instanceId"),
        "workspaceId": answer.get("workspaceId"),
        "workspaceDirectory": if directory.is_empty() { &source.row.directory } else { directory },
        "workspaceDisplayName": null,
        "isolationStrategy": "existing",
        "sessionStatus": "idle",
        "session": session,
        "instanceStatus": "running",
        "parentSessionId": null,
        "sourceDirectory": Some(source.row.directory.as_str()).filter(|d| !d.is_empty()),
        "branch": Some(source.branch.as_str()).filter(|b| !b.is_empty()),
        "activityStatus": "idle",
        "lifecycleStatus": "running",
        "retentionStatus": "active",
        "archivedAt": null,
        "typedInstanceStatus": "running",
        "isHidden": false,
        "totalTokens": source.row.total_tokens,
        "totalCost": source.row.total_cost,
        "projectId": source.listing.project_id,
        "projectName": source.listing.project_name,
        "harnessType": answer.get("harnessType"),
        "tags": tags,
        "forkedFromSessionId": answer.get("forkedFromSessionId").and_then(Value::as_str).unwrap_or(source_id),
        "spawnKind": "fork",
    })
}

/// The server's answer to a fork, blocking: its body, or its own words for a refusal.
pub fn request(base: &str, id: &str, typed: &str) -> Result<Value, String> {
    let (status, text) = http::text_with("POST", base, &fork_path(id), Some(&fork_body(typed)))?;
    if !(200..300).contains(&status) {
        return Err(store::error_text(status, &text));
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(v) if v.pointer("/session/id").and_then(Value::as_str).is_some_and(|id| !id.is_empty()) => Ok(v),
        _ => Err(FAILED.into()),
    }
}

impl State {
    /// Opens the dialog for `id` (the form starts over each time, and an earlier error is gone).
    pub(crate) fn open_fork_dialog(&mut self, tx: &mut Tx) {
        let session = self.session();
        let mut dialog = ForkDialog::open(&session.id, &session.title);
        // The browser selects an autofocused field's text; the prefill is meant to be typed over.
        let end = dialog.title.text.len();
        dialog.title.set_selection(0, end);
        self.head.bar.fork = Some(dialog);
        tx.focus(TITLE_FIELD, false);
    }

    /// Closing the dialog while the request runs leaves the request to finish (Fleet's dialog can be closed then too).
    pub fn close_fork_dialog(&mut self) {
        self.head.bar.fork = None;
    }

    /// Submit: sends the typed title (Enter does the same). Nothing while a fork is already running.
    pub fn fork_submit(&mut self, tx: &mut Tx) {
        let Some(d) = self.head.bar.fork.as_mut() else { return };
        if d.forking {
            return;
        }
        d.forking = true;
        d.error = None;
        let (id, typed) = (d.session.clone(), d.title.text.clone());
        let Some(link) = self.link_for(&id) else {
            // No server: the invented session is copied the way the server would, so scenes and tests need none.
            let n = self.projects.iter().flat_map(|p| &p.sessions).count();
            let title = if typed.trim().is_empty() { prefill(&self.session().title) } else { typed.trim().to_string() };
            let answer = json!({
                "instanceId": "mock-instance",
                "workspaceId": "mock-workspace",
                "harnessType": self.session().row.harness_type,
                "session": { "id": format!("mock.fork.{n}"), "title": title, "tags": [] },
            });
            self.fork_done(&id, HOME_KEY, Ok(answer), tx);
            return;
        };
        let machine = self.machine_key_of(&id).to_string();
        let session = id.clone();
        link.off_thread(move |base| request(base, &session, &typed), move |s, tx, result| s.fork_done(&id, &machine, result, tx));
    }

    /// The server answered. A failure keeps the dialog open with the server's words; a success closes it, puts the new
    /// session in the list (or, for another machine's session, in that machine's kept sessions) and opens it.
    pub(crate) fn fork_done(&mut self, source_id: &str, machine: &str, result: Result<Value, String>, tx: &mut Tx) {
        if let Some(d) = self.head.bar.fork.as_mut().filter(|d| d.session == source_id) {
            d.forking = false;
        }
        let answer = match result {
            Ok(a) => a,
            Err(why) => {
                if let Some(d) = self.head.bar.fork.as_mut().filter(|d| d.session == source_id) {
                    d.error = Some(why);
                }
                return;
            }
        };
        if self.head.bar.fork.as_ref().is_some_and(|d| d.session == source_id) {
            self.head.bar.fork = None;
        }
        let Some(source) = self.store.session_by_id(&self.projects, source_id).map(|s| s.row_copy()) else { return };
        let Some(mut made) = sessions::parse_sessions(&json!([forked_row(&source, &answer, source_id)]), 0).pop() else { return };
        made.title = display_title(None, answer.pointer("/session/title").and_then(Value::as_str).unwrap_or(""));
        let id = made.id.clone();
        if machine == HOME_KEY {
            store::upsert(&mut self.projects, made);
            let at = self.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|s| s.id == id).map(|i| (p, i)));
            if let Some((p, i)) = at {
                self.rail = crate::Rail::Sessions;
                self.open_session(p, i, tx);
            }
        } else {
            self.store.upsert_elsewhere(machine, made);
            self.rail = crate::Rail::Sessions;
            self.go(store::Route::Session(id.clone()));
            self.after_open(tx);
        }
        // The list is the truth: it comes back with the fork in it.
        if !store::is_invented(&id) {
            self.list_again_on(machine);
        }
    }
}
