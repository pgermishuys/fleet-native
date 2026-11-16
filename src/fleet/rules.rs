//! The rules "Don't ask again" made for each session (`composables/use-session-permission-rules.ts`, rows 5.9-5.11 and
//! 7.8 of `docs/parity/04-questions-and-permissions.md`). They load when a session opens and after a reconnect;
//! `session.permission-rules` carries the whole list whenever it changes; a removal shows at once and is sent after.
use super::events::{Event, Kind};
use super::http;
use crate::State;
use crate::asks::rules::{AllowedRule, PermissionLevel, read_rules};
use serde_json::Value;
use std::collections::HashMap;

/// Each session's rules as Fleet last said, so switching back shows them at once, and the number of the newest load or
/// event per session: a load asked for before an event arrived is older than it (`loadId`).
#[derive(Debug, Default)]
pub struct Rules {
    by_session: HashMap<String, Vec<AllowedRule>>,
    generation: HashMap<String, u64>,
}

impl Rules {
    pub fn of(&self, session: &str) -> &[AllowedRule] {
        self.by_session.get(session).map_or(&[], Vec::as_slice)
    }

    pub fn begin_load(&mut self, session: &str) -> u64 {
        let n = self.generation.entry(session.to_string()).or_default();
        *n += 1;
        *n
    }

    /// A load's answer: dropped when an event or a removal came since it was asked for. True when the list grew.
    pub fn loaded(&mut self, session: &str, generation: u64, rules: Vec<AllowedRule>) -> bool {
        self.generation.get(session) == Some(&generation) && self.put(session, rules)
    }

    /// `session.permission-rules`: newer than any load in flight. True when the list grew.
    pub fn replace(&mut self, session: &str, rules: Vec<AllowedRule>) -> bool {
        *self.generation.entry(session.to_string()).or_default() += 1;
        self.put(session, rules)
    }

    /// A removal's local effect: the rule leaves at once, and a load in flight no longer applies.
    pub fn remove(&mut self, session: &str, rule: &str) {
        *self.generation.entry(session.to_string()).or_default() += 1;
        if let Some(held) = self.by_session.get_mut(session) {
            held.retain(|r| r.id != rule);
        }
    }

    pub fn remove_all(&mut self, session: &str) {
        *self.generation.entry(session.to_string()).or_default() += 1;
        self.by_session.insert(session.to_string(), Vec::new());
    }

    fn put(&mut self, session: &str, rules: Vec<AllowedRule>) -> bool {
        let grew = rules.len() > self.of(session).len();
        self.by_session.insert(session.to_string(), rules);
        grew
    }
}

impl State {
    /// The open session's rules, oldest first.
    pub fn open_rules(&self) -> &[AllowedRule] {
        self.draft_owner().map_or(&[], |id| self.rules.of(&id))
    }

    /// The level the open session's harness runs at now, from the preferences (`usePermissionLevel`).
    pub fn open_level(&self) -> PermissionLevel {
        let harness = self.session().row.harness_type.clone();
        PermissionLevel::of(harness.as_deref(), |key| self.pickers.prefs.get(key).map(str::to_string))
    }

    /// The chip's count went up for the open session: it flashes once.
    fn rules_grew(&mut self, session: &str) {
        if self.draft_owner().as_deref() == Some(session) {
            self.allowed_flash = Some(self.clock);
        }
    }

    /// The chip closes its popover when the last rule goes.
    fn keep_allowed_open(&mut self) {
        if self.open_rules().is_empty() && self.ui.open == Some(crate::ui::Open::Popover("allowed")) {
            self.ui.open = None;
        }
    }

    /// `session.permission-rules` `{sessionId, items}`: the whole list; another session's own is kept under its id.
    pub fn on_rules_event(&mut self, event: &Event) {
        let Event::Other(Kind::SessionPermissionRules, payload) = event else { return };
        let Some(session) = payload.get("sessionId").and_then(Value::as_str) else { return };
        if self.rules.replace(session, read_rules(payload.get("items").unwrap_or(&Value::Null))) {
            self.rules_grew(session);
        }
        self.keep_allowed_open();
    }

    /// Loads the session's rules when it opens and after a reconnect. A failed load is not shown.
    pub fn load_rules(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        let generation = self.rules.begin_load(session);
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::rules(base, &asked),
            move |s, _, result| {
                if let Ok((status, body)) = result
                    && (200..300).contains(&status)
                    && s.rules.loaded(&id, generation, read_rules(&body))
                {
                    s.rules_grew(&id);
                }
                s.keep_allowed_open();
            },
        );
    }

    /// Opening the popover reads the level again, so a change made in Settings shows (the panel mounts only when opened).
    pub fn open_allowed(&mut self) {
        self.load_preferences();
        self.ui.open = Some(crate::ui::Open::Popover("allowed"));
    }

    /// `remove`: the rule goes at once and Fleet is told; refused (not ok, not 404) or not reached, the list loads again.
    pub fn remove_rule(&mut self, rule: &str) {
        let Some(session) = self.draft_owner() else { return };
        self.rules.remove(&session, rule);
        self.keep_allowed_open();
        self.send_removal(&session, Some(rule.to_string()));
    }

    /// `removeAll`.
    pub fn remove_all_rules(&mut self) {
        let Some(session) = self.draft_owner() else { return };
        self.rules.remove_all(&session);
        self.keep_allowed_open();
        self.send_removal(&session, None);
    }

    fn send_removal(&mut self, session: &str, rule: Option<String>) {
        let Some(link) = self.link_for(session) else { return };
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| match &rule {
                Some(rule) => http::rule_remove(base, &asked, rule),
                None => http::rules_remove_all(base, &asked),
            },
            move |s, _, result| match result {
                Ok((status, _)) if (200..300).contains(&status) || status == 404 => {}
                _ => s.load_rules(&id),
            },
        );
    }
}
