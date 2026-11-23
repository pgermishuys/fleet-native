//! Opening a session by its id when no list holds it (`use-session-detail.ts`, `routes/sessions.$id.tsx`): a run's session
//! from Automations or Workflows, a link from GitHub, a dream's. The session's own record is read
//! (`GET /api/sessions/{id}`); the row it makes goes in the list, and a 404 is the "Session not found" view.
use super::settings::HOME_MACHINE_KEY as HOME_KEY;
use super::{http, sessions, store};
use crate::{State, Tx};
use serde_json::{Value, json};

/// Which session was asked for by id, and whether its machine said it has none.
#[derive(Default, Debug)]
pub struct Opening {
    id: Option<String>,
    missing: bool,
}

impl Opening {
    /// Whether the route names the session this was started for, so a list that doesn't hold it yet is not a reason to leave.
    pub fn holds(&self, route: &store::Route) -> bool {
        matches!(route, store::Route::Session(id) if self.id.as_deref() == Some(id.as_str()))
    }
}

/// `GET /api/sessions/{id}`.
pub fn detail_path(id: &str) -> String {
    format!("/api/sessions/{}", crate::conv::image_sniff::encode_component(id))
}

/// The session's record as a list item (`GET /api/sessions` shape), so the list's own parser reads it
/// (`buildSessionListItem`). The record is flat: `id`, `title`, `createdAt`; a list item nests those under `session`.
pub fn list_item_of_detail(id: &str, detail: &Value) -> Value {
    let mut item = detail.as_object().cloned().unwrap_or_default();
    let created = detail.get("createdAt").and_then(Value::as_str).and_then(super::work::parse_iso_ms).unwrap_or(0);
    item.insert(
        "session".into(),
        json!({
            "id": id,
            "title": detail.get("title").and_then(Value::as_str).unwrap_or(""),
            "time": { "created": created, "updated": created },
        }),
    );
    // A record names its lifecycle `lifecycleStatus` (and `status`); a list item `sessionStatus`.
    if item.get("sessionStatus").is_none() {
        let lifecycle = item.get("lifecycleStatus").or_else(|| item.get("status")).cloned().unwrap_or(Value::Null);
        item.insert("sessionStatus".into(), lifecycle);
    }
    // The list leaves a sub-agent's session out of its rows; opened by id it is wanted, so its parent is put back below.
    item.remove("parentSessionId");
    Value::Object(item)
}

impl State {
    /// A scene of "Session not found": a session id the machine has no record of, opened with the list on screen.
    pub fn show_session_not_found(&mut self, dark: bool) {
        self.dark = Some(dark);
        self.opening = Opening { id: Some("a1b2c3d4".into()), missing: true };
        self.go(store::Route::Session("a1b2c3d4".into()));
    }
    /// Whether the open session is one its machine said it doesn't have: the window shows "Session not found".
    pub fn session_missing(&self) -> bool {
        self.opening.missing && self.opening.holds(&self.route) && self.open_at().0 == usize::MAX
    }

    /// Opens the session `id` of `machine`, whether or not the list holds it. A listed one opens in place; any other is
    /// read from its machine, and put in the list once it answers (`useSessionDetail`).
    pub fn open_session_by_id(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        self.rail = crate::Rail::Sessions;
        let at = self.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|s| s.id == id).map(|i| (p, i)));
        if let Some((p, i)) = at {
            return self.open_session(p, i, tx);
        }
        if self.store.session_by_id(&self.projects, id).is_some() {
            self.go(store::Route::Session(id.to_string()));
            return self.after_open(tx);
        }
        if machine != HOME_KEY {
            self.machines.remember_sessions(machine, std::slice::from_ref(&id.to_string()));
        }
        self.opening = Opening { id: Some(id.to_string()), missing: false };
        self.go(store::Route::Session(id.to_string()));
        self.after_open(tx);
        let Some(link) = self.link_for_machine(machine) else { return };
        let (path, key, wanted) = (detail_path(id), machine.to_string(), id.to_string());
        link.off_thread(
            move |base| http::text_with("GET", base, &path, None),
            move |s, tx, result| s.session_detail_done(&key, &wanted, result, tx),
        );
    }

    /// The machine answered: a 404 is "Session not found"; a record is filed in the list, or in that machine's kept sessions.
    pub(crate) fn session_detail_done(&mut self, machine: &str, id: &str, result: Result<(u16, String), String>, tx: &mut Tx) {
        if !self.opening.holds(&store::Route::Session(id.to_string())) {
            return;
        }
        let Ok((status, text)) = result else { return };
        if status == 404 {
            self.opening.missing = true;
            return;
        }
        let Some(detail) = serde_json::from_str::<Value>(&text).ok().filter(|v| (200..300).contains(&status) && v.is_object()) else {
            return;
        };
        let item = list_item_of_detail(id, &detail);
        let retention = store::Retention::All;
        let Some(mut made) = sessions::parse_sessions_for(&json!([item]), 0, retention).pop() else { return };
        made.listing.parent = detail.get("parentSessionId").and_then(Value::as_str).map(str::to_string);
        // The list holds a machine's own active sessions; a sub-agent's or an archived one would be gone with its next read.
        let kept_apart = made.listing.parent.is_some() || made.row.archived;
        if machine == HOME_KEY && !kept_apart {
            store::upsert(&mut self.projects, made);
        } else {
            self.store.upsert_elsewhere(machine, made);
        }
        self.opening.missing = false;
        self.after_open(tx);
    }
}
