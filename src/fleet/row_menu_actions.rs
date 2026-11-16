//! What the session row's right-click menu does beyond what the row already does (`SessionItem.vue`): Fork and New session in
//! this folder (a new session made at once, no dialog), Move to project, Move out of / back under a parent, Copy session ID.
//! Every call goes to the machine the session is on. With no server the invented list takes the change, so scenes and tests
//! need none.
use super::fork::{Made, fork_path, new_in_folder_path, post_made, prefill};
use super::groups::UNGROUPED_ID;
use super::settings::HOME_MACHINE_KEY as HOME_KEY;
use super::{http, store};
use crate::{State, Tx};
use serde_json::{Value, json};

/// `PATCH /api/sessions/{id}/project`.
pub fn project_path(id: &str) -> String {
    format!("/api/sessions/{}/project", crate::conv::image_sniff::encode_component(id))
}

/// `{ projectId }`: `null` is "ungrouped".
pub fn project_body(project: Option<&str>) -> Value {
    json!({ "projectId": project })
}

/// `PATCH /api/sessions/{id}/lineage`.
pub fn lineage_path(id: &str) -> String {
    format!("/api/sessions/{}/lineage", crate::conv::image_sniff::encode_component(id))
}

pub fn lineage_body(detached: bool) -> Value {
    json!({ "detached": detached })
}

/// What a refused New session in this folder says when the server gives no words.
pub const NEW_IN_FOLDER_FAILED: &str = "Couldn't start a session in this folder";

/// Where a session came from (`lineageOriginOf`): the id of its parent and whether it is a sub-agent's.
pub fn origin_of(l: &super::groups::Listing) -> Option<(String, bool)> {
    if let Some(p) = &l.parent {
        return Some((p.clone(), true));
    }
    let fork = l.forked_from.clone();
    let started = l.spawned_by.clone();
    let chosen = if l.spawn_kind.as_deref() == Some("fork") { fork.or(started) } else { started.or(fork) };
    chosen.map(|id| (id, false))
}

/// `movableOutOf`: the parent a fork or a started session can be moved out of now.
pub fn movable_out_of(l: &super::groups::Listing) -> Option<String> {
    origin_of(l).filter(|(_, sub)| !sub).filter(|_| !l.detached).map(|(id, _)| id)
}

/// `movableBackUnder`: the parent a session the user moved out can go back under.
pub fn movable_back_under(l: &super::groups::Listing) -> Option<String> {
    if l.detached { origin_of(l).map(|(id, _)| id) } else { None }
}

impl State {
    /// The session `id` as its machine's list holds it.
    fn row_copy_of(&self, id: &str) -> Option<crate::mock::Session> {
        if let Some(s) = self.store.session_by_id(&self.projects, id) {
            return Some(s.row_copy());
        }
        let key = self.machine_key_of(id);
        self.machines.others.get(key)?.sessions.iter().find(|s| s.id == id).map(|s| s.row_copy())
    }

    /// The menu's Fork: a copy of the conversation, made at once (the header's Fork asks for a title first).
    pub fn fork_row(&mut self, id: &str, tx: &mut Tx) {
        self.make_from_row(id, Made::Fork, tx);
    }

    /// The menu's New session in this folder.
    pub fn new_in_folder(&mut self, id: &str, tx: &mut Tx) {
        self.make_from_row(id, Made::InFolder, tx);
    }

    fn make_from_row(&mut self, id: &str, made: Made, tx: &mut Tx) {
        let Some(source) = self.row_copy_of(id) else { return };
        let machine = self.machine_key_of(id).to_string();
        // A row of another machine is kept (with its machine) so what is made from it can find it.
        if machine != HOME_KEY {
            self.machines.remember_sessions(&machine, &[id.to_string()]);
            if self.store.session_by_id(&self.projects, id).is_none() {
                self.store.upsert_elsewhere(&machine, source.row_copy());
            }
        }
        let fail = move |s: &mut State, tx: &mut Tx, why: String| {
            let words = if !why.is_empty() {
                why
            } else if made == Made::Fork {
                super::fork::FAILED.into()
            } else {
                NEW_IN_FOLDER_FAILED.into()
            };
            s.toast(words, None, 5.0, tx);
        };
        let Some(link) = self.link_for(id) else {
            let n = self.projects.iter().flat_map(|p| &p.sessions).count();
            let title = if made == Made::Fork { prefill(&source.title) } else { String::new() };
            let answer = json!({
                "instanceId": "mock-instance",
                "workspaceId": "mock-workspace",
                "harnessType": source.row.harness_type,
                "session": { "id": format!("mock.made.{n}"), "title": title, "tags": [] },
            });
            self.made_done(id, HOME_KEY, made, answer, tx);
            return;
        };
        let (sent, path) = match made {
            Made::Fork => (id.to_string(), fork_path(id)),
            Made::InFolder => (id.to_string(), new_in_folder_path(id)),
        };
        let body = if made == Made::Fork { json!({}) } else { Value::Null };
        link.off_thread(
            move |base| post_made(base, &path, &body),
            move |s, tx, result| match result {
                Ok(answer) => s.made_done(&sent, &machine, made, answer, tx),
                Err(why) => fail(s, tx, why),
            },
        );
    }

    /// The menu's Move to project (`useMoveSession`): the list takes it at once, the server is told, and a refusal is a toast.
    pub fn move_to_project(&mut self, id: &str, project: Option<&str>, name: &str) {
        let Some(current) = self.row_copy_of(id).map(|s| s.listing.project_id) else { return };
        if current.as_deref() == project || (current.is_none() && project == Some(UNGROUPED_ID)) {
            return;
        }
        let (target, label) = (project.map(str::to_string), name.to_string());
        let key = self.machine_key_of(id).to_string();
        let set = move |s: &mut crate::mock::Session, p: Option<String>, n: Option<String>| {
            s.listing.project_id = p;
            s.listing.project_name = n;
        };
        let (p2, n2) = (target.clone(), target.is_some().then(|| label.clone()));
        let mut projects = std::mem::take(&mut self.projects);
        self.store.patch(&mut projects, id, |s| set(s, p2.clone(), n2.clone()));
        self.projects = projects;
        self.machines.patch_session(&key, id, |s| set(s, p2.clone(), n2.clone()));
        let Some(link) = self.link_for(id) else { return };
        let (sent, body, path) = (id.to_string(), project_body(project), project_path(id));
        link.off_thread(
            move |base| super::toolbar::request(base, "PATCH", &path, Some(&body)),
            move |s, tx, result| {
                if let Err(why) = result {
                    s.toast(if why.is_empty() { "Failed to move session".into() } else { why }, None, 5.0, tx);
                }
                // The list is the truth, whichever way it went.
                s.list_again(&sent);
            },
        );
    }

    /// The menu's Move out of / Move back under (`lineage-moves`): the row leaves or rejoins its parent at once. Its Undo
    /// toast is piece M's.
    pub fn move_lineage(&mut self, id: &str, detached: bool) {
        let Some(was) = self.row_copy_of(id).map(|s| s.listing.detached) else { return };
        self.set_detached(id, detached);
        let Some(link) = self.link_for(id) else { return };
        let (sent, body, path) = (id.to_string(), lineage_body(detached), lineage_path(id));
        link.off_thread(
            move |base| {
                let (status, text) = http::text_with("PATCH", base, &path, Some(&body))?;
                if (200..300).contains(&status) { Ok(()) } else { Err(store::error_text(status, &text)) }
            },
            move |s, tx, result| {
                if let Err(why) = result {
                    s.set_detached(&sent, was);
                    s.toast(if why.is_empty() { "Couldn't move the session.".into() } else { why }, None, 5.0, tx);
                }
                s.list_again(&sent);
            },
        );
    }

    fn set_detached(&mut self, id: &str, detached: bool) {
        let key = self.machine_key_of(id).to_string();
        let mut projects = std::mem::take(&mut self.projects);
        self.store.patch(&mut projects, id, |s| s.listing.detached = detached);
        self.projects = projects;
        self.machines.patch_session(&key, id, |s| s.listing.detached = detached);
    }

    /// A toast for a menu row whose work is another component's (`waiting`).
    pub fn menu_waiting(&mut self, what: &str, owner: &str, tx: &mut Tx) {
        self.toast(format!("{what} isn't in the native app yet ({owner})."), None, 4.0, tx);
    }
}
