//! Pinning sessions and putting them in order (`stores/session-pins.ts`, rows 1.89-1.95, 3.143). The list changes at once and the
//! server is told right away; when it refuses, the list goes back and the error shows in the one toast (`archive_queue`). A call
//! goes to the machine the session is on, and changes that machine's list. The Pinned group itself is piece J's.
use super::groups::{Listed, Listing, pin_order_for, pinned_neighbour_for};
use super::settings::HOME_MACHINE_KEY;
use super::{http, store};
use crate::State;
use crate::mock::Session;
use serde_json::{Value, json};

/// `PUT /api/sessions/{id}/pin` and `DELETE` of the same.
pub fn pin_path(id: &str) -> String {
    format!("/api/sessions/{id}/pin")
}

/// The body of the pin request: the pinned session it goes before, or `null` for the end of the Pinned group.
pub fn pin_body(before: Option<&str>) -> Value {
    json!({ "beforeSessionId": before })
}

/// Pins (`PUT`) and answers where the session now sits (`pinSession`), blocking: run it on a thread.
pub fn send_pin(base: &str, id: &str, before: Option<&str>) -> Result<f64, String> {
    let (status, text) = http::text_with("PUT", base, &pin_path(id), Some(&pin_body(before)))?;
    if !(200..300).contains(&status) {
        return Err(store::error_text(status, &text));
    }
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v.get("pinOrder").and_then(Value::as_f64))
        .ok_or_else(|| "No data returned".to_string())
}

/// Unpins (`DELETE`), blocking (`unpinSession`).
pub fn send_unpin(base: &str, id: &str) -> Result<(), String> {
    let (status, text) = http::text_with("DELETE", base, &pin_path(id), None)?;
    if (200..300).contains(&status) { Ok(()) } else { Err(store::error_text(status, &text)) }
}

/// `GET /api/sessions?limit=<n>` read for each row's `pinOrder` alone, as (id, order) (`refreshPinOrders`).
pub fn read_pin_orders(base: &str, limit: usize) -> Result<Vec<(String, Option<f64>)>, String> {
    let (status, text) = http::text_with("GET", base, &format!("/api/sessions?limit={limit}"), None)?;
    if !(200..300).contains(&status) {
        return Err(store::error_text(status, &text));
    }
    let rows: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(rows
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| Some((r.pointer("/session/id")?.as_str()?.to_string(), r.get("pinOrder").and_then(Value::as_f64))))
        .collect())
}

#[derive(Default)]
pub struct Pins {
    /// The last failure, until it is dismissed or the next pin begins.
    pub error: Option<String>,
}

impl State {
    /// Where the pin of `id` is now, and the list it is ordered in: home's, or the machine's it is on.
    fn pin_context(&self, id: &str) -> Option<(Listing, Vec<&Session>)> {
        let home: Vec<&Session> = self.projects.iter().flat_map(|p| p.sessions.iter()).collect();
        if let Some(s) = home.iter().find(|s| s.id == id) {
            return Some((s.listing().clone(), home));
        }
        let list: Vec<&Session> = self.machines.others.get(self.machine_key_of(id))?.sessions.iter().collect();
        let listing = list.iter().find(|s| s.id == id)?.listing().clone();
        Some((listing, list))
    }

    /// Whether `id` is pinned (`pinOrder` is a number, 0 counts).
    pub fn is_pinned(&self, id: &str) -> bool {
        self.pin_context(id).is_some_and(|(l, _)| l.pin_order.is_some())
    }

    pub(crate) fn set_pin_order(&mut self, id: &str, order: Option<f64>) {
        let mut projects = std::mem::take(&mut self.projects);
        self.store.patch(&mut projects, id, |s| s.listing.pin_order = order);
        self.projects = projects;
        let key = self.machine_key_of(id).to_string();
        self.machines.patch_session(&key, id, |s| s.listing.pin_order = order);
    }

    /// Pins the session just before `before`, or at the end of the Pinned group; moves it there when it is pinned already.
    pub fn pin_session(&mut self, id: &str, before: Option<&str>) {
        let Some((listing, list)) = self.pin_context(id) else { return };
        if before == Some(id) {
            return;
        }
        let previous = listing.pin_order;
        let expected = pin_order_for(&list, id, before);
        let at_home = self.machine_key_of(id) == HOME_MACHINE_KEY;
        let count = list.len();
        self.pins.error = None;
        self.set_pin_order(id, Some(expected));
        let Some(link) = self.link_for(id) else { return };
        let (sent, id, before) = (id.to_string(), id.to_string(), before.map(str::to_string));
        link.off_thread(
            move |base| match send_pin(base, &id, before.as_deref()) {
                // The server numbered every pin again (the gap got too small): read the new numbers, on home only.
                Ok(order) if order != expected && at_home => Ok((order, read_pin_orders(base, if count == 0 { 100 } else { count }).ok())),
                Ok(order) => Ok((order, None)),
                Err(why) => Err(why),
            },
            move |s, _, result| match result {
                Ok((order, renumbered)) => {
                    s.set_pin_order(&sent, Some(order));
                    for (id, order) in renumbered.into_iter().flatten() {
                        s.set_pin_order(&id, order);
                    }
                }
                Err(why) => {
                    s.set_pin_order(&sent, previous);
                    s.pins.error = Some(if why.is_empty() { "Couldn't pin the session.".into() } else { why });
                }
            },
        );
    }

    /// Unpins the session: it goes back to its project.
    pub fn unpin_session(&mut self, id: &str) {
        let Some((listing, _)) = self.pin_context(id) else { return };
        let previous = listing.pin_order;
        self.pins.error = None;
        self.set_pin_order(id, None);
        let Some(link) = self.link_for(id) else { return };
        let (sent, id) = (id.to_string(), id.to_string());
        link.off_thread(
            move |base| send_unpin(base, &id),
            move |s, _, result| {
                if let Err(why) = result {
                    s.set_pin_order(&sent, previous);
                    s.pins.error = Some(if why.is_empty() { "Couldn't unpin the session.".into() } else { why });
                }
            },
        );
    }

    /// Whether a pinned `id` has a pinned neighbour that way (the menu's Move up and Move down turn off at the ends).
    pub fn can_move_pin(&self, id: &str, delta: i32) -> bool {
        self.pin_context(id).is_some_and(|(l, list)| l.pin_order.is_some() && pinned_neighbour_for(&list, id, delta).is_some())
    }

    /// Moves a pinned session one place up (-1) or down (+1) in the Pinned group; not past either end.
    pub fn move_pin(&mut self, id: &str, delta: i32) {
        let before = {
            let Some((_, list)) = self.pin_context(id) else { return };
            match pinned_neighbour_for(&list, id, delta) {
                Some(before) => before,
                None => return,
            }
        };
        self.pin_session(id, before.as_deref());
    }

    /// The pin button: unpins a pinned row, pins an unpinned one at the end.
    pub fn toggle_pin(&mut self, id: &str) {
        match self.pin_context(id) {
            Some((l, _)) if l.pin_order.is_some() => self.unpin_session(id),
            Some(_) => self.pin_session(id, None),
            None => {}
        }
    }
}
