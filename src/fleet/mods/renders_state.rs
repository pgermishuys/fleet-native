//! The threads and the app's state behind `renders.rs`: the requests a tool row's mods are asked with, what they hand back,
//! and the events and the Mods switch that make rows ask again. Every request runs off the UI thread and comes back as a job
//! (`Link::off_thread`); with no server nothing is sent.
use super::actions;
use super::renders::{Answer, Effect, Loaded, RenderJob, RowHead, RowRead, parse_answer, parse_sites};
use super::types::{Action, Surface};
use crate::conv::tools::{get_tool, tool_duration_ms};
use crate::fleet::events::{Event, Kind};
use crate::fleet::http;
use crate::fleet::model::PartData;
use crate::fleet::settings::mods_kept::{self as kept, Switch, switched_on};
use crate::fleet::settings::skills::MODS_PREFERENCE_KEY;
use crate::fleet::state::Topic;
use crate::{State, Tx};
use serde_json::Value;

fn session_path(session: &str, tail: &str) -> String {
    format!("/api/sessions/{}/mods/{tail}", crate::conv::image_sniff::encode_component(session))
}

/// `fetchHooks`: the switch first (read when nothing has read it), then the sites. Off, or a refusal (a 404 is Mods off):
/// no hooks and no panes; a request that never got an answer: no hooks, and the panes stay.
fn fetch_hooks(base: &str, session: &str, known: Option<Switch>, preference: Option<&str>) -> (Option<Switch>, Loaded) {
    let switch = known.or_else(|| kept::read_switch(base));
    // A switch that can't be read counts as on.
    let on = switch.is_none() || switched_on(preference, switch);
    if !on {
        return (switch, Loaded::Off);
    }
    let loaded = match http::text_with("GET", base, &session_path(session, "sites"), None) {
        Ok((status, body)) if (200..300).contains(&status) => parse_sites(&serde_json::from_str(&body).unwrap_or(Value::Null)),
        Ok(_) => Loaded::Off,
        Err(_) => Loaded::Failed,
    };
    (switch, loaded)
}

fn render(base: &str, job: &RenderJob) -> Option<Answer> {
    let (status, body) = http::text_with("POST", base, &session_path(&job.session_id, "render"), Some(&job.body)).ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    parse_answer(&serde_json::from_str(&body).ok()?)
}

impl State {
    fn mods_preference_value(&self) -> Option<String> {
        self.pickers.prefs.get(MODS_PREFERENCE_KEY).map(str::to_string)
    }

    /// The rows the app shows asked their mods again: each tool row of the open session (what a test says it shows, when it
    /// says). Rows nothing hooks are skipped before anything is copied, so a session with no mods costs a look at its tools.
    pub fn sync_mod_rows(&mut self) {
        let rows = self.mod_renders.fixed_rows.clone();
        let mut effects = Vec::new();
        if let Some(rows) = rows {
            for (head, read) in rows {
                let machine = self.machine_key_of(&head.session_id).to_string();
                effects.extend(self.mod_renders.sync(&machine, &head, || read));
            }
        } else if let (Some(session), Some(stream)) = (self.open_id(), self.stream.as_ref()) {
            let machine = self.machine_key_of(&session).to_string();
            for part in stream.messages.iter().flat_map(|m| m.parts.iter()) {
                let PartData::Tool { tool, call_id, state, .. } = part else { continue };
                if call_id.is_empty() || !self.mod_renders.may_ask(&session, &get_tool(tool).name) {
                    continue;
                }
                let head = RowHead {
                    session_id: session.clone(),
                    call_id: call_id.clone(),
                    raw_tool: tool.clone(),
                    status: state.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
                };
                effects.extend(self.mod_renders.sync(&machine, &head, || row_read(tool, state)));
            }
        }
        self.run_mod_effects(effects);
    }

    /// `watchModSession`: starts watching the session, reading its hooks and declared panes once.
    pub fn watch_mod_session(&mut self, session: &str) {
        let machine = self.machine_key_of(session).to_string();
        let effects = self.mod_renders.watch(&machine, session);
        self.run_mod_effects(effects);
    }

    pub(crate) fn run_mod_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::LoadHooks { session_id, .. } => self.load_mod_hooks(&session_id),
                Effect::Render(job) => self.start_mod_render(job),
            }
        }
    }

    fn load_mod_hooks(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        self.mod_renders.begin_load();
        let machine = self.machine_key_of(session).to_string();
        let (preference, known) = (self.mods_preference_value(), self.mod_renders.switches.get(&machine).copied());
        let id = session.to_string();
        let asked = session.to_string();
        link.off_thread(
            move |base| fetch_hooks(base, &asked, known, preference.as_deref()),
            move |s: &mut State, _tx: &mut Tx, (switch, loaded)| {
                match switch {
                    Some(sw) => {
                        s.mod_renders.switches.insert(machine, sw);
                    }
                    None => {
                        s.mod_renders.switches.remove(&machine);
                    }
                }
                s.mod_renders.hooks_loaded(&id, loaded);
                s.mod_renders.end_load();
                s.sync_mod_rows();
            },
        );
    }

    fn start_mod_render(&mut self, job: RenderJob) {
        let key = job.key.clone();
        let Some(link) = self.link_for(&job.session_id) else {
            let next = self.mod_renders.answered(&key, None);
            self.run_mod_effects(next);
            return;
        };
        link.off_thread(
            move |base| render(base, &job),
            move |s: &mut State, _tx: &mut Tx, answer: Option<Answer>| {
                let next = s.mod_renders.answered(&key, answer);
                s.run_mod_effects(next);
            },
        );
    }

    /// `mods.changed` and `mod.ui` on the `sessions` topic, and `mod.ui` on the open session's.
    pub(crate) fn on_mod_renders_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(kind, payload) = event else { return };
        let text = |k: &str| payload.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
        match (kind, topic) {
            (Kind::ModsChanged, Topic::Sessions) if text("reason") == Some("switch") => {
                // The switch moves: read it again, and ask once `keep_mod_renders` sees it has.
                self.reread_mod_switches();
            }
            (Kind::ModsChanged, Topic::Sessions) => self.reload_all_mod_hooks(),
            (Kind::ModUi, Topic::Sessions) => {
                // With no session it is Fleet's own (no mod) following the host loading or dropping mods, which changes which
                // sites they hook; a mod's own redraw can't, so only draw again.
                if text("sessionId").is_some() || text("mod").is_some() {
                    self.mod_renders.invalidate(text("sessionId"), text("requestId"));
                    self.sync_mod_rows();
                } else {
                    self.reload_all_mod_hooks();
                }
            }
            (Kind::ModUi, Topic::Session(id)) => {
                self.mod_renders.invalidate(Some(id), text("requestId"));
                self.sync_mod_rows();
            }
            _ => {}
        }
    }

    fn reload_all_mod_hooks(&mut self) {
        let mut effects = Vec::new();
        for machine in self.mod_renders.watched_machines() {
            effects.extend(self.mod_renders.reload_all(&machine));
        }
        self.run_mod_effects(effects);
    }

    fn reread_mod_switches(&mut self) {
        for machine in self.mod_renders.watched_machines() {
            let Some(link) = self.link_for_machine(&machine) else { continue };
            let key = machine.clone();
            self.mod_renders.begin_load();
            link.off_thread(kept::read_switch, move |s: &mut State, _tx: &mut Tx, got: Option<Switch>| {
                if let Some(sw) = got {
                    s.mod_renders.switches.insert(key, sw);
                }
                s.mod_renders.end_load();
            });
        }
    }

    /// Each frame: follows each machine's Mods switch (the user's choice in Settings is a preference, no event says it moved):
    /// when it turns on the sessions watched there ask for their sites, and when it turns off they forget their drawings. Not
    /// while it is first read: the first ask goes by that read.
    pub fn keep_mod_renders(&mut self) -> bool {
        let preference = self.mods_preference_value();
        let mut effects = Vec::new();
        for machine in self.mod_renders.watched_machines() {
            let now = self.mod_renders.switches.get(&machine).map(|sw| switched_on(preference.as_deref(), Some(*sw)));
            let was = self.mod_renders.switch_seen.insert(machine.clone(), now).flatten();
            if now.is_some() && was.is_some() && now != was {
                effects.extend(self.mod_renders.reload_all(&machine));
            }
        }
        let moved = !effects.is_empty();
        self.run_mod_effects(effects);
        moved
    }

    /// A used control of a mod's tree: `POST /mods/action`, and when the server can't deliver it (an expired handle, the host
    /// not running, a failed request) the call's row draws again, so the site shows the current tree instead of a dead control.
    pub fn mod_action(&mut self, session: &str, call: &str, action: Action, surface: Surface) {
        let Some(link) = self.link_for(session) else { return };
        let (id, call) = (session.to_string(), call.to_string());
        let sent = session.to_string();
        link.off_thread(
            move |base| actions::send(base, &sent, &action, surface),
            move |s: &mut State, _tx: &mut Tx, handled: bool| {
                if !handled {
                    s.mod_renders.invalidate(Some(&id), Some(&call));
                    s.sync_mod_rows();
                }
            },
        );
    }
}

/// What a row hands a mod: the call's input and output as the harness kept them, with the title a row shows.
fn row_read(tool: &str, state: &Value) -> RowRead {
    let input = state.get("input").cloned().unwrap_or(Value::Null);
    let text = |k: &str| state.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let title = Some(text("title")).filter(|t| !t.is_empty()).unwrap_or_else(|| get_tool(tool).label(&input));
    RowRead { output: text("output"), title, duration_ms: tool_duration_ms(state), input }
}
