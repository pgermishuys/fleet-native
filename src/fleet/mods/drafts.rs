//! A session's draft mods, for the cards after its last reply (component 22, piece H): ports the draft half of
//! `lib/mods/kept-api.ts` (`fetchDrafts`, `setDraftOn`, `checkDraft`, `keepDraft`, `dismissKeepRequest`) and of `stores/mods.ts`
//! (the per-session list, one load at a time, the reload on `mods.changed`). The requests are plain functions a thread runs; what
//! touches the app is the `State` half below.
use super::check_summary;
use crate::conv::export_history::encode_component;
use crate::fleet::events::{Event, Kind};
use crate::fleet::settings::mods_kept::{self as kept, Off, Refused, Switch, switched_on};
use crate::fleet::settings::skills::MODS_PREFERENCE_KEY;
use crate::fleet::state::Topic;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Why the draft's last load was refused by the mod host (`ModDraftProblem`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Problem {
    pub code: String,
    pub message: String,
    pub line: Option<i64>,
}

/// The agent asked the user to keep a draft (`ModKeepRequest`), with its note.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeepRequest {
    pub note: Option<String>,
    pub at: String,
}

/// A mod the agent is writing in one session (`ModDraft`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModDraft {
    pub session_id: String,
    pub name: String,
    /// None while its `mod.json` is missing or invalid (the agent is still writing it).
    pub description: Option<String>,
    pub version: Option<String>,
    pub off: Option<Off>,
    /// The active version of the kept mod of the same name, which the draft stands in for in its session.
    pub kept: Option<i64>,
    pub keep_request: Option<KeepRequest>,
    pub problem: Option<Problem>,
}

impl ModDraft {
    /// No description and no version: the manifest isn't there yet.
    pub fn still_writing(&self) -> bool {
        self.description.is_none() && self.version.is_none()
    }

    /// "Check: {message}" with " (line {n})" when the problem has a line.
    pub fn problem_line(&self) -> Option<String> {
        let p = self.problem.as_ref()?;
        Some(match p.line {
            Some(n) => format!("Check: {} (line {n})", p.message),
            None => format!("Check: {}", p.message),
        })
    }

    /// The card's status word.
    pub fn status(&self) -> &'static str {
        match &self.off {
            Some(o) if o.by_strikes() => "Turned off after three failures",
            Some(_) => "Off in this session",
            None => "On in this session only",
        }
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// One draft; an answer with no name is not a draft. `session` stands in for a missing `sessionId`.
pub fn parse_draft(v: &Value, session: &str) -> Option<ModDraft> {
    let name = text(v, "name").filter(|n| !n.is_empty())?;
    let object = |k: &str| v.get(k).filter(|o| o.is_object());
    Some(ModDraft {
        session_id: text(v, "sessionId").unwrap_or_else(|| session.to_string()),
        name,
        description: text(v, "description"),
        version: text(v, "version"),
        off: object("off").map(|o| Off { by: text(o, "by").unwrap_or_default(), error: text(o, "error").filter(|e| !e.is_empty()) }),
        kept: v.get("kept").and_then(Value::as_i64),
        keep_request: object("keepRequest")
            .map(|r| KeepRequest { note: text(r, "note").filter(|n| !n.is_empty()), at: text(r, "at").unwrap_or_default() }),
        problem: object("problem").map(|p| Problem {
            code: text(p, "code").unwrap_or_default(),
            message: text(p, "message").unwrap_or_default(),
            line: p.get("line").and_then(Value::as_i64),
        }),
    })
}

// ─── Requests (a thread runs these) ─────────────────────────────────────────

fn draft_path(session: &str, name: &str) -> String {
    format!("/api/sessions/{}/mods/drafts/{}", encode_component(session), encode_component(name))
}

/// `fetchDrafts`.
pub fn fetch_drafts(base: &str, session: &str) -> Result<Vec<ModDraft>, Refused> {
    let path = format!("/api/sessions/{}/mods/drafts", encode_component(session));
    let v = kept::call(base, "GET", &path, None)?;
    Ok(v.as_array().map(|a| a.iter().filter_map(|d| parse_draft(d, session)).collect()).unwrap_or_default())
}

/// `setDraftOn`: the draft as it is now.
pub fn set_draft_on(base: &str, session: &str, name: &str, on: bool) -> Result<ModDraft, Refused> {
    let path = format!("{}/{}", draft_path(session, name), if on { "on" } else { "off" });
    let v = kept::call(base, "POST", &path, None)?;
    parse_draft(&v, session).ok_or(Refused { status: 200, message: "Something went wrong.".into() })
}

/// `checkDraft`: the check's report as the server wrote it, none when there is no checker.
pub fn check_draft(base: &str, session: &str, name: &str) -> Result<Option<Value>, Refused> {
    let v = kept::call(base, "GET", &format!("{}/check", draft_path(session, name)), None)?;
    Ok(v.get("check").filter(|c| !c.is_null()).cloned())
}

/// `keepDraft`: keeps the draft for all the user's sessions; an empty note sends no body. The kept mod's own answer.
pub fn keep_draft(base: &str, session: &str, name: &str, note: &str) -> Result<Value, Refused> {
    let note = note.trim();
    let body = (!note.is_empty()).then(|| json!({ "note": note }));
    kept::call(base, "POST", &format!("{}/keep", draft_path(session, name)), body.as_ref())
}

/// `dismissKeepRequest`: drops the agent's "keep this?" request. A 404 counts as done: it may already be gone.
pub fn dismiss_keep_request(base: &str, session: &str, name: &str) -> Result<(), Refused> {
    match kept::call(base, "DELETE", &format!("{}/keep-request", draft_path(session, name)), None) {
        Err(r) if r.status != 404 => Err(r),
        _ => Ok(()),
    }
}

// ─── What the app keeps ─────────────────────────────────────────────────────

/// The drafts of the sessions the cards track, and what they are doing.
#[derive(Default)]
pub struct ModDrafts {
    lists: HashMap<String, Vec<ModDraft>>,
    /// Sessions whose drafts were asked for; a later `mods.changed` for one asks again. A failed load stays here, so a frame
    /// doesn't ask over and over.
    pub(crate) tracked: HashSet<String>,
    loading: HashSet<String>,
    /// The Mods switch by machine, read when a card first needs it.
    pub(crate) switches: HashMap<String, Switch>,
    switch_asked: HashSet<String>,
    /// Turn off / Turn on again running, by `session/name`, and the refusal shown under a card.
    toggling: HashSet<String>,
    errors: HashMap<String, String>,
    /// The draft whose Keep dialog is open (session, name): set by Keep… and Review….
    pub review: Option<(String, String)>,
    /// The open dialog's own state (`ModReviewDialog.vue`): the check, the note, what it is doing.
    pub dialog: ReviewDialog,
    /// Requests on their way.
    asking: usize,
}

/// What the Keep dialog knows of the draft's check.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Checked {
    /// "Checking the mod…".
    #[default]
    Loading,
    /// The server's report, or none when it has no checker.
    Report(Option<Value>),
    /// The words the check request failed with.
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Keep,
    Decline,
}

/// The Keep dialog's state: reset each time it opens.
#[derive(Default)]
pub struct ReviewDialog {
    pub checked: Checked,
    pub note: TextEdit,
    pub busy: Option<Busy>,
    pub refused: Option<String>,
    /// Counts the openings, so an answer to an earlier one is dropped (`loadId`).
    serial: u64,
}

/// "Why keep it?" takes this many characters at most.
pub const NOTE_LIMIT: usize = 2000;

impl ReviewDialog {
    pub fn problems(&self) -> check_summary::Problems {
        match &self.checked {
            Checked::Report(r) => check_summary::check_problems(r.as_ref()),
            _ => Default::default(),
        }
    }

    /// `canKeep`: not while checking or busy, and not with an error in the report.
    pub fn can_keep(&self) -> bool {
        self.checked != Checked::Loading && self.busy.is_none() && self.problems().error.is_none()
    }
}

fn card_key(session: &str, name: &str) -> String {
    format!("{session}/{name}")
}

impl ModDrafts {
    pub fn list(&self, session: &str) -> &[ModDraft] {
        self.lists.get(session).map_or(&[], Vec::as_slice)
    }

    pub fn set(&mut self, session: &str, drafts: Vec<ModDraft>) {
        self.tracked.insert(session.to_string());
        self.lists.insert(session.to_string(), drafts);
    }

    /// No request is on its way.
    pub fn idle(&self) -> bool {
        self.asking == 0
    }

    pub fn is_toggling(&self, session: &str, name: &str) -> bool {
        self.toggling.contains(&card_key(session, name))
    }

    pub fn action_error(&self, session: &str, name: &str) -> Option<&str> {
        self.errors.get(&card_key(session, name)).map(String::as_str)
    }

    /// `replaceDraft`.
    fn replace(&mut self, session: &str, name: &str, change: impl FnOnce(ModDraft) -> ModDraft) {
        if let Some(list) = self.lists.get_mut(session)
            && let Some(slot) = list.iter_mut().find(|d| d.name == name)
        {
            *slot = change(std::mem::take(slot));
        }
    }

    fn without_request(mut d: ModDraft) -> ModDraft {
        d.keep_request = None;
        d
    }
}

impl State {
    fn mods_switched_on(&self, session: &str) -> bool {
        let machine = self.machine_key_of(session);
        switched_on(self.pickers.prefs.get(MODS_PREFERENCE_KEY), self.mod_drafts.switches.get(machine).copied())
    }

    /// The cards to draw under the open session's last reply: none with the Mods switch off.
    pub fn draft_cards(&self, session: &str) -> &[ModDraft] {
        if self.mods_switched_on(session) { self.mod_drafts.list(session) } else { &[] }
    }

    /// Each frame: the open session's drafts are read once its Mods switch is known to be on (the switch first, when nothing has
    /// read it) and read again when it turns off and on. True when it asked for something.
    pub fn keep_mod_drafts(&mut self) -> bool {
        let Some(session) = self.open_id() else { return false };
        let machine = self.machine_key_of(&session).to_string();
        if !self.mod_drafts.switches.contains_key(&machine) && !self.mod_drafts.switch_asked.contains(&machine) {
            let Some(link) = self.link_for(&session) else { return false };
            self.mod_drafts.switch_asked.insert(machine.clone());
            self.mod_drafts.asking += 1;
            link.off_thread(kept::read_switch, move |s: &mut State, _tx: &mut Tx, got: Option<Switch>| {
                s.mod_drafts.asking = s.mod_drafts.asking.saturating_sub(1);
                match got {
                    Some(sw) => {
                        s.mod_drafts.switches.insert(machine, sw);
                    }
                    // A switch that couldn't be read is asked again the next time a session is opened.
                    None => {
                        s.mod_drafts.switch_asked.remove(&machine);
                    }
                }
            });
            return true;
        }
        if !self.mods_switched_on(&session) {
            self.mod_drafts.tracked.remove(&session);
            return false;
        }
        if self.mod_drafts.tracked.contains(&session) {
            return false;
        }
        self.load_mod_drafts(&session)
    }

    /// `loadDrafts`: one load per session at a time. A refusal leaves what is shown, except a 404 (Mods off), which is none.
    pub fn load_mod_drafts(&mut self, session: &str) -> bool {
        let Some(link) = self.link_for(session) else { return false };
        self.mod_drafts.tracked.insert(session.to_string());
        if !self.mod_drafts.loading.insert(session.to_string()) {
            return false;
        }
        self.mod_drafts.asking += 1;
        let id = session.to_string();
        link.off_thread(
            {
                let id = id.clone();
                move |base| fetch_drafts(base, &id)
            },
            move |s: &mut State, _tx: &mut Tx, got: Result<Vec<ModDraft>, Refused>| {
                s.mod_drafts.asking = s.mod_drafts.asking.saturating_sub(1);
                s.mod_drafts.loading.remove(&id);
                match got {
                    Ok(list) => {
                        s.mod_drafts.lists.insert(id, list);
                    }
                    Err(r) if r.status == 404 => {
                        s.mod_drafts.lists.insert(id, Vec::new());
                    }
                    Err(_) => {}
                }
            },
        );
        true
    }

    /// `setDraftOn`, from the card: immediate, no confirmation. The draft takes the server's answer in place (turning it off also
    /// drops its keep request); a refusal is shown under the card.
    pub fn toggle_mod_draft(&mut self, session: &str, name: &str) {
        let key = card_key(session, name);
        if self.mod_drafts.toggling.contains(&key) {
            return;
        }
        let Some(link) = self.link_for(session) else { return };
        let on = self.mod_drafts.list(session).iter().find(|d| d.name == name).is_some_and(|d| d.off.is_some());
        self.mod_drafts.toggling.insert(key.clone());
        self.mod_drafts.errors.remove(&key);
        self.mod_drafts.asking += 1;
        let (id, draft) = (session.to_string(), name.to_string());
        let (asked, named) = (id.clone(), draft.clone());
        link.off_thread(
            move |base| set_draft_on(base, &asked, &named, on),
            move |s: &mut State, _tx: &mut Tx, got: Result<ModDraft, Refused>| {
                s.mod_drafts.asking = s.mod_drafts.asking.saturating_sub(1);
                s.mod_drafts.toggling.remove(&key);
                match got {
                    Ok(d) => s.mod_drafts.replace(&id, &draft, |_| if on { d } else { ModDrafts::without_request(d) }),
                    Err(r) => {
                        let message = if r.status == 0 { "Couldn't change the draft.".to_string() } else { r.message };
                        s.mod_drafts.errors.insert(key, message);
                    }
                }
            },
        );
    }

    /// Keep… and Review…: the Keep dialog for this draft. It checks the draft now, and only now.
    pub fn open_draft_review(&mut self, session: &str, name: &str) {
        // Keep… is disabled while the manifest is missing; a press that gets through anyway opens nothing.
        if self.mod_drafts.list(session).iter().any(|d| d.name == name && d.still_writing() && d.keep_request.is_none()) {
            return;
        }
        self.mod_drafts.review = Some((session.to_string(), name.to_string()));
        let note = self.mod_drafts.list(session).iter().find(|d| d.name == name).and_then(|d| d.keep_request.as_ref()?.note.clone());
        let serial = self.mod_drafts.dialog.serial + 1;
        let mut note = TextEdit::new(note.unwrap_or_default());
        note.focused = true;
        self.mod_drafts.dialog = ReviewDialog { note, serial, ..Default::default() };
        let Some(link) = self.link_for(session) else {
            self.mod_drafts.dialog.checked = Checked::Report(None);
            return;
        };
        self.mod_drafts.asking += 1;
        let (id, draft) = (session.to_string(), name.to_string());
        link.off_thread(
            move |base| check_draft(base, &id, &draft),
            move |s: &mut State, _tx: &mut Tx, got: Result<Option<Value>, Refused>| {
                s.mod_drafts.asking = s.mod_drafts.asking.saturating_sub(1);
                if s.mod_drafts.dialog.serial != serial || s.mod_drafts.review.is_none() {
                    return;
                }
                s.mod_drafts.dialog.checked = match got {
                    Ok(report) => Checked::Report(report),
                    Err(r) if r.message.is_empty() => Checked::Failed("Fleet couldn't check this mod.".into()),
                    Err(r) => Checked::Failed(r.message),
                };
            },
        );
    }

    /// Closes the dialog; whatever is still on its way is dropped when it lands.
    pub fn close_draft_review(&mut self) {
        self.mod_drafts.review = None;
        self.mod_drafts.dialog.serial += 1;
    }

    /// "Keep for all my sessions": the note trimmed and cut at its limit; closes on success, shows the server's words otherwise.
    pub fn keep_reviewed_draft(&mut self) {
        let Some((session, name)) = self.mod_drafts.review.clone() else { return };
        if !self.mod_drafts.dialog.can_keep() {
            return;
        }
        let d = &mut self.mod_drafts.dialog;
        d.busy = Some(Busy::Keep);
        d.refused = None;
        let (serial, note) = (d.serial, d.note.text.chars().take(NOTE_LIMIT).collect::<String>());
        self.keep_mod_draft(&session, &name, &note, move |s, _tx, got| {
            if s.mod_drafts.dialog.serial != serial {
                return;
            }
            s.mod_drafts.dialog.busy = None;
            match got {
                Ok(_) => s.close_draft_review(),
                Err(r) => {
                    s.mod_drafts.dialog.refused = Some(if r.message.is_empty() { "Couldn't keep this mod.".into() } else { r.message })
                }
            }
        });
        if self.link_for(&session).is_none() {
            self.mod_drafts.dialog.busy = None;
            self.mod_drafts.dialog.refused = Some("Couldn't keep this mod.".into());
        }
    }

    /// Cancel closes; with the agent's request, Discard tells it first (`decline`).
    pub fn decline_reviewed_draft(&mut self) {
        let Some((session, name)) = self.mod_drafts.review.clone() else { return };
        if self.mod_drafts.dialog.busy.is_some() {
            return;
        }
        if !self.mod_drafts.list(&session).iter().any(|d| d.name == name && d.keep_request.is_some()) {
            self.close_draft_review();
            return;
        }
        let d = &mut self.mod_drafts.dialog;
        d.busy = Some(Busy::Decline);
        d.refused = None;
        let serial = d.serial;
        self.dismiss_mod_keep_request(&session, &name, move |s, _tx, got| {
            if s.mod_drafts.dialog.serial != serial {
                return;
            }
            s.mod_drafts.dialog.busy = None;
            match got {
                Ok(()) => s.close_draft_review(),
                Err(r) => {
                    s.mod_drafts.dialog.refused = Some(if r.message.is_empty() { "Couldn't tell the agent.".into() } else { r.message })
                }
            }
        });
        if self.link_for(&session).is_none() {
            self.mod_drafts.dialog.busy = None;
            self.mod_drafts.dialog.refused = Some("Couldn't tell the agent.".into());
        }
    }

    /// `keep` of the store: keeps the draft, drops its request and reads the list again; `done` hears the refusal, which leaves the
    /// request where it was.
    pub fn keep_mod_draft(
        &mut self,
        session: &str,
        name: &str,
        note: &str,
        done: impl FnOnce(&mut State, &mut Tx, Result<Value, Refused>) + Send + 'static,
    ) {
        let Some(link) = self.link_for(session) else { return };
        self.mod_drafts.asking += 1;
        let (id, draft, note) = (session.to_string(), name.to_string(), note.to_string());
        let (asked, named) = (id.clone(), draft.clone());
        link.off_thread(
            move |base| keep_draft(base, &asked, &named, &note),
            move |s: &mut State, tx: &mut Tx, got: Result<Value, Refused>| {
                s.mod_drafts.asking = s.mod_drafts.asking.saturating_sub(1);
                if got.is_ok() {
                    s.mod_drafts.replace(&id, &draft, ModDrafts::without_request);
                    s.load_mod_drafts(&id);
                }
                done(s, tx, got);
            },
        );
    }

    /// `dismissKeepRequest` of the store.
    pub fn dismiss_mod_keep_request(
        &mut self,
        session: &str,
        name: &str,
        done: impl FnOnce(&mut State, &mut Tx, Result<(), Refused>) + Send + 'static,
    ) {
        let Some(link) = self.link_for(session) else { return };
        self.mod_drafts.asking += 1;
        let (id, draft) = (session.to_string(), name.to_string());
        let (asked, named) = (id.clone(), draft.clone());
        link.off_thread(
            move |base| dismiss_keep_request(base, &asked, &named),
            move |s: &mut State, tx: &mut Tx, got: Result<(), Refused>| {
                s.mod_drafts.asking = s.mod_drafts.asking.saturating_sub(1);
                if got.is_ok() {
                    s.mod_drafts.replace(&id, &draft, ModDrafts::without_request);
                    s.load_mod_drafts(&id);
                }
                done(s, tx, got);
            },
        );
    }

    /// `mods.changed`: a tracked session's drafts are read again, whichever machine said it; the switch is read again when it moved.
    pub(crate) fn on_mod_drafts_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(Kind::ModsChanged, payload) = event else { return };
        if !matches!(topic, Topic::Sessions) {
            return;
        }
        if let Some(session) = payload.get("sessionId").and_then(Value::as_str).filter(|s| self.mod_drafts.tracked.contains(*s)) {
            let session = session.to_string();
            self.load_mod_drafts(&session);
        }
        if payload.get("reason").and_then(Value::as_str) == Some("switch") {
            self.mod_drafts.switch_asked.clear();
            self.mod_drafts.switches.clear();
        }
    }
}
