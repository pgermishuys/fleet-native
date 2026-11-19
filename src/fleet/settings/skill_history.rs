//! The History dialog (`skills/SkillHistoryDialog.vue`, rows 8.82 to 8.89): the user's versions of a skill, newest first, then
//! Fleet's. Any of them can be the one sessions get, and each compares with Fleet's text. Requests run on a thread; an answer for a
//! dialog that was closed or opened again is dropped (`seq`). With no Fleet to ask, the invented skill answers.
use super::skill_editor::{Detail, mock_detail, parse_detail};
use crate::State;
use crate::fleet::http;
use crate::fleet::skills::{BuiltInSkill, answer, at, parse_saved, switch_body};
use serde_json::Value;

/// The version being made the one sessions get (`busy`): a number, or Fleet's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Version(u32),
    Fleet,
}

/// One version's text, open next to the list (`comparing`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compare {
    pub number: u32,
    pub content: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillHistory {
    pub open: bool,
    seq: u64,
    pub name: String,
    pub from_settings: bool,
    pub detail: Option<Detail>,
    pub comparing: Option<Compare>,
    pub busy: Option<Busy>,
    pub error: Option<String>,
}

impl SkillHistory {
    /// The spinner shows until the skill came or failed.
    pub fn loading(&self) -> bool {
        self.detail.is_none() && self.error.is_none()
    }
}

impl State {
    /// Opens the History on a skill: reset, then read it.
    pub fn skill_history_open(&mut self, name: &str, from_settings: bool) {
        let seq = self.settings.history.seq + 1;
        self.settings.history = SkillHistory { open: true, seq, name: name.to_string(), from_settings, ..Default::default() };
        let Some(link) = self.skill_link(from_settings) else {
            let list = if from_settings { self.settings.built_in.list.clone() } else { self.skills.list.clone().unwrap_or_default() };
            self.settings.history.detail = Some(mock_detail(name, &list));
            return;
        };
        let (path, fallback) = (at(name), format!("Couldn't load {name}."));
        link.off_thread(
            move |base| answer(&fallback, http::text_with("GET", base, &path, None)),
            move |s: &mut State, _tx, got: Result<Value, String>| {
                let h = &mut s.settings.history;
                if !h.open || h.seq != seq {
                    return;
                }
                match got.and_then(|v| parse_detail(&v).ok_or_else(|| "No skill came back.".to_string())) {
                    Ok(detail) => h.detail = Some(detail),
                    Err(why) => h.error = Some(why),
                }
            },
        );
    }

    pub fn skill_history_close(&mut self) {
        self.settings.history.open = false;
    }

    /// Use this version, or Fleet's for `None` (`use`): one at a time. The list and the shared lists take the server's answer.
    pub fn skill_history_use(&mut self, version: Option<u32>) {
        let h = &mut self.settings.history;
        if !h.open || h.busy.is_some() {
            return;
        }
        h.busy = Some(version.map_or(Busy::Fleet, Busy::Version));
        h.error = None;
        let (seq, name, from_settings) = (h.seq, h.name.clone(), h.from_settings);
        let Some(link) = self.skill_link(from_settings) else {
            // Invented: the same skill with that version in use.
            let h = &mut self.settings.history;
            if let Some(d) = h.detail.as_mut() {
                d.version = version;
                for v in &mut d.versions {
                    v.active = Some(v.number) == version;
                }
                d.your_content = version.map(|_| d.fleet_content.clone());
            }
            let detail = h.detail.clone().unwrap_or_default();
            self.skill_history_used(Ok(detail), None, seq);
            return;
        };
        let (path, body, fallback) = (format!("{}/active", at(&name)), switch_body(version), format!("Couldn't switch {name}."));
        link.off_thread(
            move |base| answer(&fallback, http::text_with("PUT", base, &path, Some(&body))),
            move |s: &mut State, _tx, got: Result<Value, String>| {
                let skill = got.as_ref().ok().and_then(parse_saved);
                let detail = got.and_then(|v| parse_detail(&v).ok_or_else(|| "No skill came back.".to_string()));
                s.skill_history_used(detail, skill, seq);
            },
        );
    }

    fn skill_history_used(&mut self, got: Result<Detail, String>, skill: Option<BuiltInSkill>, seq: u64) {
        let h = &mut self.settings.history;
        if !h.open || h.seq != seq {
            return;
        }
        h.busy = None;
        let from_settings = h.from_settings;
        match got {
            Ok(detail) => {
                h.detail = Some(detail);
                if let Some(skill) = skill {
                    self.skill_history_took(skill, from_settings);
                }
            }
            Err(why) => h.error = Some(why),
        }
    }

    fn skill_history_took(&mut self, skill: BuiltInSkill, from_settings: bool) {
        if from_settings {
            self.built_in_took(skill);
        } else {
            self.skills_update(skill);
        }
    }

    /// Compare with Fleet's (`compare`): opens one version's text, or closes it when it is the open one. One is open at a time.
    pub fn skill_history_compare(&mut self, number: u32) {
        let h = &mut self.settings.history;
        if !h.open {
            return;
        }
        if h.comparing.as_ref().is_some_and(|c| c.number == number) {
            h.comparing = None;
            return;
        }
        h.error = None;
        let (seq, name, from_settings) = (h.seq, h.name.clone(), h.from_settings);
        let Some(link) = self.skill_link(from_settings) else {
            self.settings.history.comparing = Some(Compare { number, content: crate::view::skill_diff::AFTER.into() });
            return;
        };
        let (path, fallback) = (format!("{}/versions/{number}", at(&name)), format!("Couldn't load version {number} of {name}."));
        link.off_thread(
            move |base| answer(&fallback, http::text_with("GET", base, &path, None)),
            move |s: &mut State, _tx, got: Result<Value, String>| {
                let h = &mut s.settings.history;
                if !h.open || h.seq != seq {
                    return;
                }
                match got {
                    Ok(v) => {
                        h.comparing = Some(Compare { number, content: v.get("content").and_then(Value::as_str).unwrap_or_default().into() })
                    }
                    Err(why) => h.error = Some(why),
                }
            },
        );
    }
}
