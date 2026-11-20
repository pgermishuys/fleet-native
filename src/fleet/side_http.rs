//! The side conversation's routes and what they do to its state (`use-side-conversation.ts`): load, ask, minimize, seen,
//! discard, Undo and keep. Every call is about one session and goes to that session's machine (`link_for`), on a thread; the
//! answer comes back as a job. State changes show at once and the server's refusal undoes them, as Fleet's do.
//! With no server (scenes, the invented list) nothing is sent and the side conversation is made here, so tests and
//! scenes need none.
use super::http::{self, Pick};
use super::queue::error_message;
use super::side::{Draft, Progress, SIDE_DISCARD_UNDO_MS, Side, SideState, new_correlation_id};
use super::{settings::HOME_MACHINE_KEY as HOME_KEY, store};
use crate::composer::drafts::{side_draft_key, side_main_draft_key};
use crate::{State, Tx};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Every session's side conversation, kept while other sessions are open.
pub type Sides = HashMap<String, SideState>;

/// `/api/sessions/{id}/side` and what follows it.
pub fn path(session: &str, tail: &str) -> String {
    format!("/api/sessions/{}/side{tail}", crate::conv::image_sniff::encode_component(session))
}

/// One call: the status and the body (`Null` when there is none, as for a 204), or why it never got an answer.
pub fn call(base: &str, method: &str, session: &str, tail: &str, body: Option<&Value>) -> Result<(u16, Value), String> {
    let (status, text) = http::text_with(method, base, &path(session, tail), body)?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// `POST /side`'s body: the question with the composer's agent, model and effort (`null` for none).
pub fn ask_body(text: &str, pick: &Pick, correlation_id: &str) -> Value {
    json!({
        "text": text,
        "agent": pick.agent,
        "model": pick.model.as_ref().map(|(p, m)| json!({ "providerID": p, "modelID": m })),
        "effort": pick.effort,
        "correlationId": correlation_id,
    })
}

/// A refusal in the server's words (`{error}`, `detail` or `title`), else `fallback`.
fn refusal(body: &Value, status: u16, what: &str) -> String {
    error_message(body, format!("{what} (HTTP {status})."))
}

/// What loading a session's side conversation found: the side (200), and for none, a discard still in its Undo window.
pub struct Loaded {
    pub side: Option<Side>,
    pub discarded: Option<(Side, u64)>,
}

fn load(base: &str, session: &str) -> Result<Loaded, String> {
    let (status, body) = call(base, "GET", session, "", None)?;
    let side = if status == 200 { Side::from_json(&body) } else { None };
    if side.is_some() {
        return Ok(Loaded { side, discarded: None });
    }
    let (status, body) = call(base, "GET", session, "/discarded", None)?;
    let left = body.get("undoRemainingMs").and_then(Value::as_f64).unwrap_or(0.0);
    let discarded = (status == 200 && left > 0.0).then(|| Side::from_json(&body["sideConversation"]).map(|s| (s, left as u64))).flatten();
    Ok(Loaded { side: None, discarded })
}

impl State {
    pub(crate) fn side_state(&mut self, session: &str) -> &mut SideState {
        self.sides.entry(session.to_string()).or_default()
    }

    /// The open session's side conversation, if the app has looked for one.
    pub fn side(&self) -> Option<&SideState> {
        self.sides.get(&self.draft_owner()?)
    }

    /// The side conversation's error, which the composer shows first of its errors (`sendError`).
    pub fn side_error(&self) -> Option<&str> {
        self.side()?.error.as_deref()
    }

    /// Keeps the side conversations (each frame): the open session's is loaded the first time it shows, an Undo window that
    /// has passed ends, and the composer follows the state. True when something changed.
    pub fn keep_side(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        let ended: Vec<(String, Side)> =
            self.sides.iter_mut().filter_map(|(owner, st)| st.expire(now).map(|side| (owner.clone(), side))).collect();
        for (owner, side) in ended {
            self.drafts.write(&side_draft_key(&owner, &side.session_id), "", now);
            changed = true;
        }
        let Some(owner) = self.draft_owner() else { return changed };
        let has_link = self.link_for(&owner).is_some();
        if has_link && !self.sides.contains_key(&owner) {
            self.sides.insert(owner.clone(), SideState::default());
        }
        let Some(st) = self.sides.get(&owner) else { return changed };
        if has_link && !st.loaded && !st.loading && self.side_tried.as_deref() != Some(owner.as_str()) {
            self.side_tried = Some(owner.clone());
            self.load_side(&owner);
        }
        let st = &self.sides[&owner];
        if let Some(left) = st.undo_until {
            tx.wake_after(std::time::Duration::from_secs_f64((left - now).clamp(0.05, 1.0)));
        }
        // The composer swaps drafts when a side conversation opens or folds, and also for one just discarded.
        let id = st.side.as_ref().or(st.discarded.as_ref()).map(|s| s.session_id.clone());
        let open = st.is_open();
        if self.side_id != id || self.side_open != open {
            self.side_id = id;
            self.side_open = open;
            changed = true;
        }
        changed
    }

    pub(crate) fn load_side(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        self.side_state(session).loading = true;
        let owner = session.to_string();
        let asked = session.to_string();
        link.off_thread(move |base| load(base, &asked), move |s, tx, result| s.on_side_loaded(&owner, result, tx));
    }

    /// The side conversation arrived (`load`): a question asked meanwhile wins; a discard still in its window is offered
    /// again for what is left; a failure leaves it closed, and the next visit asks again.
    pub fn on_side_loaded(&mut self, session: &str, result: Result<Loaded, String>, _tx: &mut Tx) {
        let now = self.clock;
        let st = self.side_state(session);
        st.loading = false;
        let Ok(found) = result else { return };
        if !st.asking {
            st.side = found.side;
        }
        if st.side.is_none()
            && st.discarded.is_none()
            && let Some((side, left)) = found.discarded
        {
            st.offer_undo(side, left, now);
        }
        st.loaded = true;
        self.restore_side_drafts(session);
    }

    /// After loading (`restoreDrafts`): the put-away drafts of side conversations that are gone are forgotten; with a side,
    /// the composer takes its draft and the session's goes away (the same swap as opening it); with none, a put-away
    /// session draft goes back in the composer.
    fn restore_side_drafts(&mut self, session: &str) {
        let now = self.clock;
        let st = &self.sides[session];
        let keep = st.draft_side_id().map(|id| side_draft_key(session, id));
        let opened = st.side.as_ref().filter(|s| !s.minimized).map(|s| s.session_id.clone());
        let any = keep.is_some();
        for key in self.drafts.keys(&format!("side.{session}.")) {
            if Some(&key) != keep.as_ref() {
                self.drafts.write(&key, "", now);
            }
        }
        if any {
            // Opened at once: the composer holds the session's draft, so the swap of opening is due.
            if let Some(id) = opened.filter(|_| self.draft_owner().as_deref() == Some(session)) {
                self.side_seen = Some((id, false));
            }
            return;
        }
        let main = self.drafts.read(&side_main_draft_key(session)).map(str::to_string);
        if let Some(main) = main.filter(|m| !m.is_empty()) {
            if self.draft_owner().as_deref() == Some(session) {
                self.set_draft_text(&main);
            }
            self.drafts.write(&side_main_draft_key(session), "", now);
        }
    }

    // ─── ask ───────────────────────────────────────────────────────────────────────────────────────────────────────

    /// Asks `text` in the side conversation, starting one when there is none (`ask`). False when nothing was asked: it is
    /// empty, or a question is already on its way. `done` runs with whether the server took it.
    pub fn ask_side(
        &mut self,
        session: &str,
        text: &str,
        pick: Pick,
        tx: &mut Tx,
        done: impl FnOnce(&mut State, bool) + Send + 'static,
    ) -> bool {
        let text = text.trim().to_string();
        let st = self.side_state(session);
        if text.is_empty() || st.asking {
            return false;
        }
        st.asking = true;
        st.error = None;
        if st.side.is_none() {
            st.starting = Some(text.clone());
        }
        let correlation = new_correlation_id();
        let Some(link) = self.link_for(session) else {
            // No server: the side conversation is made here, with the question as its title.
            let n = self.sides.values().filter(|s| s.side.is_some()).count();
            let id = self.sides[session].side.as_ref().map_or_else(|| format!("mock.side.{n}"), |s| s.session_id.clone());
            let minimized = false;
            let side = Side {
                session_id: id,
                instance_id: "mock-instance".into(),
                title: text.clone(),
                boundary_message_id: None,
                created_at: String::new(),
                minimized,
                seen_answer_id: None,
            };
            let answer = json!({ "sideConversation": side_json(&side), "correlationId": correlation, "messageId": null });
            let ok = self.on_side_asked(session, &text, Ok((200, answer)), tx);
            done(self, ok);
            return true;
        };
        let (owner, asked, sent) = (session.to_string(), session.to_string(), text.clone());
        let body = ask_body(&text, &pick, &correlation);
        link.off_thread(
            move |base| call(base, "POST", &asked, "", Some(&body)),
            move |s, tx, result| {
                let ok = s.on_side_asked(&owner, &sent, result, tx);
                done(s, ok);
            },
        );
        true
    }

    /// The server answered a question. True when it was taken.
    pub fn on_side_asked(&mut self, session: &str, text: &str, result: Result<(u16, Value), String>, _tx: &mut Tx) -> bool {
        let (now, at) = (self.clock, http_now());
        let st = self.side_state(session);
        st.asking = false;
        st.starting = None;
        let (side, correlation, message) = match result {
            Ok((status, body)) if (200..300).contains(&status) => {
                match (Side::from_json(&body["sideConversation"]), body["correlationId"].as_str(), body["messageId"].as_str()) {
                    (Some(side), c, m) => (side, c.map(str::to_string), m.map(str::to_string)),
                    _ => {
                        st.error = Some(format!("The side question couldn't be asked (HTTP {status})."));
                        return false;
                    }
                }
            }
            Ok((status, body)) => {
                st.error = Some(refusal(&body, status, "The side question couldn't be asked"));
                return false;
            }
            Err(why) => {
                st.error = Some(if why.trim().is_empty() { "The side question couldn't be asked.".into() } else { why });
                return false;
            }
        };
        // The server deleted a discarded one to start this: its Undo goes.
        let gone = st.discarded.as_ref().filter(|d| d.session_id != side.session_id).map(|d| d.session_id.clone());
        if let Some(old) = gone {
            st.end_undo();
            st.set_draft(Draft::Side, "");
            self.drafts.write(&side_draft_key(session, &old), "", now);
        }
        let st = self.side_state(session);
        // Asking a minimized one opens it (the server says so too).
        let id = side.session_id.clone();
        st.side = Some(side);
        st.loaded = true;
        if let Some(message) = message {
            let correlation = correlation.unwrap_or_default();
            self.prompts.show_sent(&id, &message, &correlation, text, at, now);
        }
        true
    }

    // ─── minimize, seen ────────────────────────────────────────────────────────────────────────────────────────────

    /// Folds the side conversation into its tab, or opens it again. Shown at once; the server keeps it (`setMinimized`).
    pub fn set_side_minimized(&mut self, session: &str, value: bool, tx: &mut Tx) {
        let st = self.side_state(session);
        let Some(side) = st.side.as_mut().filter(|s| s.minimized != value) else { return };
        side.minimized = value;
        if !value {
            st.pulse_answer_id = None;
            if let Some(answer) = st.latest_answer_id.clone().filter(|_| !st.working) {
                self.mark_side_seen(session, &answer);
            }
        }
        let Some(link) = self.link_for(session) else { return };
        let owner = session.to_string();
        let asked = session.to_string();
        let body = json!({ "minimized": value });
        link.off_thread(
            move |base| call(base, "PUT", &asked, "/minimized", Some(&body)),
            move |s, _tx, result| {
                let st = s.side_state(&owner);
                match result {
                    Ok((status, body)) if (200..300).contains(&status) && !body.is_null() => {
                        if let (Some(now), Some(sent)) = (st.side.as_mut(), Side::from_json(&body))
                            && now.session_id == sent.session_id
                        {
                            let seen = now.seen_answer_id.take();
                            *now = Side { seen_answer_id: seen, ..sent };
                        }
                    }
                    Ok((status, body)) => {
                        let what =
                            if value { "The side conversation couldn't be minimized" } else { "The side conversation couldn't be opened" };
                        st.error = Some(refusal(&body, status, what));
                    }
                    Err(why) => {
                        st.error = Some(if why.trim().is_empty() { "The side conversation couldn't be changed.".into() } else { why })
                    }
                }
            },
        );
        let _ = tx;
    }

    /// Ctrl+/: folds it, or opens it (`toggleMinimized`).
    pub fn toggle_side_minimized(&mut self, session: &str, tx: &mut Tx) {
        let to = !self.side_state(session).minimized();
        self.set_side_minimized(session, to, tx);
    }

    /// Records `answer` as seen, here and on the server (`markSeen`); the server's failure is ignored.
    pub fn mark_side_seen(&mut self, session: &str, answer: &str) {
        if !self.side_state(session).mark_seen(answer) {
            return;
        }
        let Some(link) = self.link_for(session) else { return };
        let (asked, body) = (session.to_string(), json!({ "answerId": answer }));
        link.off_thread(move |base| call(base, "PUT", &asked, "/seen", Some(&body)), |_, _, _| {});
    }

    /// The side conversation's stream reports (`reportProgress`).
    pub fn report_side_progress(&mut self, session: &str, progress: Progress) {
        if let Some(answer) = self.side_state(session).report_progress(progress) {
            // `report_progress` marked it seen here; the server has to hear too.
            if let Some(link) = self.link_for(session) {
                let (asked, body) = (session.to_string(), json!({ "answerId": answer }));
                link.off_thread(move |base| call(base, "PUT", &asked, "/seen", Some(&body)), |_, _, _| {});
            }
        }
    }

    // ─── discard, Undo, keep ───────────────────────────────────────────────────────────────────────────────────────

    /// Discards the side conversation: it goes at once and Undo is offered for [`SIDE_DISCARD_UNDO_MS`]; the server deletes
    /// the fork once the window has passed. A refusal puts it back (`close`).
    pub fn discard_side(&mut self, session: &str) {
        let now = self.clock;
        let st = self.side_state(session);
        let Some(closing) = st.side.take() else { return };
        st.error = None;
        st.offer_undo(closing.clone(), SIDE_DISCARD_UNDO_MS, now);
        let Some(link) = self.link_for(session) else { return };
        let (owner, asked) = (session.to_string(), session.to_string());
        link.off_thread(
            move |base| call(base, "DELETE", &asked, "", None),
            move |s, _tx, result| {
                let why = match result {
                    Ok((status, _)) if (200..300).contains(&status) => return,
                    Ok((status, body)) => refusal(&body, status, "The side conversation couldn't be discarded"),
                    Err(why) if why.trim().is_empty() => "The side conversation couldn't be discarded.".into(),
                    Err(why) => why,
                };
                let st = s.side_state(&owner);
                st.end_undo();
                st.side = Some(closing);
                st.error = Some(why);
            },
        );
    }

    /// Undo: brings back the side conversation discarded moments ago, as it was (`undoDiscard`).
    pub fn undo_side_discard(&mut self, session: &str) {
        let st = self.side_state(session);
        let Some(discarded) = st.discarded.clone() else { return };
        st.undo_until = None;
        let Some(link) = self.link_for(session) else {
            let st = self.side_state(session);
            st.side = Some(discarded);
            st.end_undo();
            return;
        };
        let (owner, asked) = (session.to_string(), session.to_string());
        link.off_thread(
            move |base| call(base, "POST", &asked, "/restore", None),
            move |s, _tx, result| {
                let st = s.side_state(&owner);
                match result {
                    Ok((status, body)) if (200..300).contains(&status) && !body.is_null() => {
                        st.side = Some(discarded.with(&body));
                        st.end_undo();
                    }
                    Ok((status, body)) => {
                        st.end_undo();
                        st.error = Some(refusal(&body, status, "The side conversation couldn't be brought back"));
                    }
                    Err(why) => {
                        st.end_undo();
                        st.error = Some(if why.trim().is_empty() { "The side conversation couldn't be brought back.".into() } else { why });
                    }
                }
            },
        );
    }

    /// Keep as session: the side conversation becomes a session of its own and opens (`keep`). A refusal keeps the panel.
    pub fn keep_side_as_session(&mut self, session: &str, tx: &mut Tx) {
        let st = self.side_state(session);
        let Some(side) = st.side.clone() else { return };
        st.error = None;
        let Some(link) = self.link_for(session) else {
            self.side_state(session).side = None;
            return;
        };
        let (owner, asked) = (session.to_string(), session.to_string());
        let machine = self.machine_key_of(session).to_string();
        link.off_thread(
            move |base| call(base, "POST", &asked, "/keep", None),
            move |s, tx, result| {
                let st = s.side_state(&owner);
                match result {
                    Ok((status, body)) if (200..300).contains(&status) && !body.is_null() => {
                        st.side = None;
                        let kept = Side::from_json(&body).unwrap_or(side);
                        s.open_kept_side(&machine, &kept, tx);
                    }
                    Ok((status, body)) => st.error = Some(refusal(&body, status, "The side conversation couldn't be kept")),
                    Err(why) => st.error = Some(if why.trim().is_empty() { "The side conversation couldn't be kept.".into() } else { why }),
                }
            },
        );
        let _ = tx;
    }

    /// The kept session is now a session of the list: open it, and read the list again so it is in it.
    fn open_kept_side(&mut self, machine: &str, kept: &Side, tx: &mut Tx) {
        self.rail = crate::Rail::Sessions;
        self.go(store::Route::Session(kept.session_id.clone()));
        self.after_open(tx);
        self.list_again_on(if machine.is_empty() { HOME_KEY } else { machine });
    }
}

fn side_json(s: &Side) -> Value {
    json!({
        "sessionId": s.session_id, "instanceId": s.instance_id, "title": s.title, "boundaryMessageId": s.boundary_message_id,
        "createdAt": s.created_at, "minimized": s.minimized, "seenAnswerId": s.seen_answer_id,
    })
}

fn http_now() -> i64 {
    super::link::now_ms()
}
