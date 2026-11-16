//! Smart links on the wire (`stores/smart-links.ts`, `composables/use-smart-link-updates.ts`): the loads, the actions and
//! the `smart_link.updated` events that keep them current. The requests run on threads and hand their answers back to the
//! UI thread; a call about a session goes to the machine the session is on (`link_for`).
use super::events::{Event, Kind};
use super::http::{self, escape};
use super::smart_links::read_origin_records;
use super::state::Topic;
use crate::State;
use rune::Tx;
use serde_json::{Value, json};

/// `GET /api/smart-links`: every session's header links.
pub const ALL_PATH: &str = "/api/smart-links";

/// `/api/sessions/{id}/smart-links` and a suffix.
pub fn path(session: &str, suffix: &str) -> String {
    format!("/api/sessions/{}/smart-links{suffix}", escape(session))
}

fn ok(status: u16) -> bool {
    (200..300).contains(&status)
}

impl State {
    /// Reads every session's header links once; pushed updates keep them current afterwards (`ensureHeaderLinksLoaded`).
    pub fn ensure_header_links_loaded(&mut self) {
        if !self.smart_links.header_loaded {
            self.load_header_links();
        }
    }

    /// Reads every session's header links again, e.g. after missing pushed updates while disconnected. The links already
    /// held stay, and pushes keep applying to them, until the answer arrives: a server that isn't answering yet must not
    /// leave the badges frozen (`reloadHeaderLinks`).
    pub fn reload_header_links(&mut self) {
        self.load_header_links();
    }

    fn load_header_links(&mut self) {
        let Some(link) = self.link.clone() else { return };
        if self.smart_links.header_inflight {
            return;
        }
        self.smart_links.header_inflight = true;
        link.off_thread(
            |base| http::text_with("GET", base, ALL_PATH, None),
            |s, _, result| {
                s.smart_links.header_inflight = false;
                // Badges are extra; the list works without them.
                if let Ok((status, text)) = result
                    && ok(status)
                    && let Ok(Value::Array(wires)) = serde_json::from_str::<Value>(&text)
                {
                    s.smart_links.header_links_loaded(&wires);
                }
            },
        );
    }

    /// Loads a session's links once (`ensureLoaded`); the Context tab asks again to retry after a failure.
    pub fn ensure_links_loaded(&mut self, session: &str) {
        if session.is_empty() || self.smart_links.by_session.contains_key(session) || self.smart_links.is_loading(session) {
            return;
        }
        let Some(link) = self.link_for(session) else { return };
        self.smart_links.set_loading(session, true);
        self.smart_links.set_load_error(session, None);
        let (asked, id) = (path(session, "/all"), session.to_string());
        link.off_thread(
            move |base| http::text_with("GET", base, &asked, None),
            move |s, _, result| {
                s.smart_links.set_loading(&id, false);
                match result {
                    Ok((status, text)) if ok(status) => match serde_json::from_str::<Value>(&text) {
                        Ok(Value::Array(wires)) => s.smart_links.set_links(&id, &wires),
                        _ => s.smart_links.set_load_error(&id, Some("Couldn't reach Fleet.".into())),
                    },
                    Ok((status, _)) => s.smart_links.set_load_error(&id, Some(format!("Fleet answered HTTP {status}."))),
                    // Links are optional context; the session works without them, and the Context tab offers another try.
                    Err(_) => s.smart_links.set_load_error(&id, Some("Couldn't reach Fleet.".into())),
                }
            },
        );
    }

    /// Reads the session's origin records (`GET /api/sessions/{id}/origin`). They only add a detail line: a failure leaves
    /// the tab as it was.
    pub fn load_origin_records(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        self.smart_links.origins.remove(session);
        let (asked, id) = (format!("/api/sessions/{}/origin", escape(session)), session.to_string());
        link.off_thread(
            move |base| http::text_with("GET", base, &asked, None),
            move |s, _, result| {
                if let Ok((status, text)) = result
                    && ok(status)
                    && let Ok(body) = serde_json::from_str::<Value>(&text)
                {
                    s.smart_links.origins.insert(id, read_origin_records(&body));
                }
            },
        );
    }

    /// Attaches a pull request or issue URL. `done` gets an error message, or `None` on success (`addLink`).
    pub fn add_link(&mut self, session: &str, url: &str, done: impl FnOnce(&mut State, &mut Tx, Option<String>) + Send + 'static) {
        let Some(link) = self.link_for(session) else { return };
        let (target, body) = (path(session, ""), json!({ "url": url }));
        link.off_thread(
            move |base| http::text_with("POST", base, &target, Some(&body)),
            move |s, tx, result| {
                let message = match result {
                    Ok((400, _)) => Some("Paste a link to a GitHub pull request or issue.".to_string()),
                    Ok((status, _)) if !ok(status) => Some(format!("Couldn't attach the link (HTTP {status}).")),
                    Ok((_, text)) => match serde_json::from_str::<Value>(&text) {
                        Ok(wire) => {
                            s.smart_links.upsert_link(&wire);
                            None
                        }
                        Err(_) => Some("Couldn't reach Fleet to attach the link.".to_string()),
                    },
                    Err(_) => Some("Couldn't reach Fleet to attach the link.".to_string()),
                };
                done(s, tx, message);
            },
        );
    }

    /// Pins or unpins at once and tells the server; a refusal or a failure puts the link back (`setPinned`).
    pub fn set_link_pinned(&mut self, session: &str, link_id: &str, pinned: bool) {
        let Some(previous) = self.smart_links.pin_now(session, link_id, pinned) else { return };
        let target = path(session, &format!("/{}/{}", escape(link_id), if pinned { "pin" } else { "unpin" }));
        self.patch_link_request(session, link_id, target, move |s, session, id| s.smart_links.set_relationship(session, id, previous));
    }

    /// Dismisses at once and tells the server; a refusal or a failure brings the link back (`dismiss`).
    pub fn dismiss_link(&mut self, session: &str, link_id: &str) {
        if !self.smart_links.by_session.get(session).is_some_and(|l| l.iter().any(|l| l.id == link_id)) {
            return;
        }
        self.smart_links.set_dismissed(session, link_id, true);
        let target = path(session, &format!("/{}/dismiss", escape(link_id)));
        self.patch_link_request(session, link_id, target, |s, session, id| s.smart_links.set_dismissed(session, id, false));
    }

    fn patch_link_request(
        &self,
        session: &str,
        link_id: &str,
        target: String,
        roll_back: impl FnOnce(&mut State, &str, &str) + Send + 'static,
    ) {
        let Some(link) = self.link_for(session) else { return };
        let (session, link_id) = (session.to_string(), link_id.to_string());
        link.off_thread(
            move |base| http::text_with("PATCH", base, &target, None),
            move |s, _, result| {
                if !matches!(result, Ok((status, _)) if ok(status)) {
                    roll_back(s, &session, &link_id);
                }
            },
        );
    }

    /// Asks the server to check the session's links again now; the results arrive as pushed updates. `done` says whether
    /// it was accepted (`refresh`).
    pub fn refresh_links(&mut self, session: &str, done: impl FnOnce(&mut State, &mut Tx, bool) + Send + 'static) {
        let Some(link) = self.link_for(session) else { return };
        let target = path(session, "/refresh");
        link.off_thread(
            move |base| http::text_with("POST", base, &target, None),
            move |s, tx, result| done(s, tx, matches!(result, Ok((status, _)) if ok(status))),
        );
    }

    /// `smart_link.updated` on the `sessions` topic carries a wire link (`useSmartLinkUpdates`). One without an `id` or a
    /// `sessionId` is ignored.
    pub fn on_smart_link_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(Kind::SmartLinkUpdated, wire) = event else { return };
        let named = |key: &str| wire.get(key).and_then(Value::as_str).is_some_and(|s| !s.is_empty());
        if *topic == Topic::Sessions && named("id") && named("sessionId") {
            self.smart_links.apply_pushed(wire);
        }
    }

    /// What the connection's start and its return both do: read every session's header links.
    pub fn load_smart_links(&mut self) {
        self.reload_header_links();
    }
}
