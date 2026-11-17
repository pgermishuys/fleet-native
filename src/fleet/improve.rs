//! The state of the Improve a skill dialog (`ImproveSkillDialog.vue`), piece U of component 9. It holds what the form
//! holds (the note, which parts of the session go along), the stage the dialog is at, the proposal and the hand-edited
//! text, and runs the requests `fleet::skills` makes. Each request carries the dialog's ticket: an answer that comes back
//! after the dialog was closed or opened again for another skill changes nothing in it. A kept version posts the notice
//! "<skill> is now your version N" with Undo (`announce`). The view is `view::improve_skill`.
use super::http;
use super::notices::{Action, ActionTone, IconRef, Notice};
use super::skills::*;
use crate::State;
use crate::icons::SPARKLES;
use crate::view::conversation::Part;
use rune::TextEdit;
use serde_json::Value;
use std::collections::HashMap;

/// What the question adds around the parts: the rules and the tags (`QUESTION_OVERHEAD_TOKENS`).
pub const QUESTION_OVERHEAD_TOKENS: u64 = 250;
/// `maxlength` of the note.
pub const NOTE_MAX: usize = 2000;
/// What Undo's button tells the notice's source; the notice's id follows.
pub const UNDO_PREFIX: &str = "skill-undo:";

/// A skill with its text, as `GET /api/skills/built-in/<name>` answers (`BuiltInSkillDetail`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SkillDetail {
    pub name: String,
    /// The user's version sessions get; `None` for Fleet's.
    pub version: Option<u32>,
    pub fleet_content: String,
    pub your_content: Option<String>,
    /// The numbers of the user's versions, newest first.
    pub versions: Vec<u32>,
}

impl SkillDetail {
    /// The text sessions get now.
    pub fn current(&self) -> &str {
        self.your_content.as_deref().unwrap_or(&self.fleet_content)
    }

    /// The number Keep gives the next version.
    pub fn next_version(&self) -> u32 {
        self.versions.first().copied().unwrap_or(0) + 1
    }
}

pub fn parse_detail(v: &Value) -> Option<SkillDetail> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some(SkillDetail {
        name: text("name")?,
        version: v.get("version").and_then(Value::as_u64).map(|n| n as u32).filter(|n| *n > 0),
        fleet_content: text("fleetContent").unwrap_or_default(),
        your_content: text("yourContent"),
        versions: v
            .get("versions")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.get("number").and_then(Value::as_u64)).map(|n| n as u32).collect())
            .unwrap_or_default(),
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stage {
    #[default]
    Compose,
    Asking,
    Proposal,
}

#[derive(Default)]
pub struct ImproveDialog {
    pub open: bool,
    /// Bumped by every open, ask and keep; an answer for an older one is dropped.
    ticket: u32,
    pub skill: String,
    pub session_id: String,
    pub turn: ImproveTurn,
    pub detail: Option<SkillDetail>,
    pub note: TextEdit,
    pub exchange: bool,
    pub tools: bool,
    pub whole: bool,
    pub stage: Stage,
    pub proposal: Option<SkillProposal>,
    /// The proposal's text, as the user may have edited it.
    pub draft: TextEdit,
    pub editing: bool,
    pub saving: bool,
    pub error: Option<String>,
    /// Undo's payload by notice id: the skill, and the version sessions got before (`None` for Fleet's).
    undo: HashMap<String, (String, Option<u32>)>,
}

impl ImproveDialog {
    pub fn skill_tokens(&self) -> u64 {
        self.detail.as_ref().map_or(0, |d| estimate_tokens(d.current()))
    }

    pub fn exchange_tokens(&self) -> u64 {
        estimate_tokens(&self.turn.exchange)
    }

    pub fn tool_tokens(&self) -> u64 {
        estimate_tokens(&self.turn.tool_calls)
    }

    pub fn note_tokens(&self) -> u64 {
        estimate_tokens(&self.note.text)
    }

    /// The question's size before it is asked (`totalTokens`); the whole conversation is the session's own.
    pub fn total_tokens(&self) -> u64 {
        let parts = if self.whole {
            0
        } else {
            (if self.exchange { self.exchange_tokens() } else { 0 }) + (if self.tools { self.tool_tokens() } else { 0 })
        };
        QUESTION_OVERHEAD_TOKENS + self.skill_tokens() + self.note_tokens() + parts
    }

    /// The parts the user ticked, joined (`context`).
    pub fn context(&self) -> String {
        [(self.exchange, &self.turn.exchange), (self.tools, &self.turn.tool_calls)]
            .into_iter()
            .filter(|(on, text)| *on && !text.is_empty())
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn can_ask(&self) -> bool {
        self.detail.is_some() && !self.note.text.trim().is_empty()
    }

    /// What Propose a change sends (`ask`).
    pub fn request(&self) -> ImproveRequest {
        let context = self.context();
        ImproveRequest {
            session_id: self.session_id.clone(),
            note: self.note.text.trim().chars().take(NOTE_MAX).collect(),
            context: if self.whole { None } else { Some(context).filter(|c| !c.is_empty()) },
            whole_conversation: self.whole,
        }
    }

    /// Keep is on once the text differs from what it was made to (`draft === proposal.base` disables it).
    pub fn can_keep(&self) -> bool {
        !self.saving && self.proposal.as_ref().is_some_and(|p| self.draft.text != p.base)
    }
}

/// The skill the server shows with no server to ask: Fleet's own, unchanged.
fn mock_detail(skill: &str) -> SkillDetail {
    SkillDetail { name: skill.into(), fleet_content: crate::view::skill_diff::BEFORE.into(), ..Default::default() }
}

impl State {
    /// Opens the dialog for a skill called in the open session: reads its text, and the turn the call was in.
    pub fn improve_open(&mut self, skill: &str, tool_id: &str) {
        let messages = improve_messages(&self.session().messages);
        let found = self
            .session()
            .messages
            .iter()
            .find(|m| m.parts.iter().any(|p| matches!(p, Part::Tool { id, .. } if id == tool_id)))
            .map(|m| m.id.clone());
        let turn = found.map(|id| improve_turn(&messages, &id)).unwrap_or_default();
        let session_id = self.open_id().unwrap_or_default();
        let d = &mut self.skills.dialog;
        let ticket = d.ticket + 1;
        *d = ImproveDialog { ticket, open: true, skill: skill.into(), turn, undo: std::mem::take(&mut d.undo), ..Default::default() };
        d.exchange = !d.turn.exchange.is_empty();
        d.session_id = session_id;
        let Some(link) = self.link.clone() else {
            self.skills.dialog.detail = Some(mock_detail(skill));
            return;
        };
        let (name, fallback) = (skill.to_string(), format!("Couldn't load {skill}."));
        link.off_thread(
            move |base| http::get(base, &at(&name)).map_err(|_| fallback.clone()),
            move |s, _, result| {
                let d = &mut s.skills.dialog;
                if !d.open || d.ticket != ticket {
                    return;
                }
                match result.and_then(|v| parse_detail(&v).ok_or_else(|| "No skill came back.".to_string())) {
                    Ok(detail) => d.detail = Some(detail),
                    Err(why) => d.error = Some(why),
                }
            },
        );
    }

    pub fn improve_close(&mut self) {
        self.skills.dialog.open = false;
        self.skills.improving = None;
    }

    /// Propose a change (`ask`).
    pub fn improve_ask(&mut self) {
        let d = &mut self.skills.dialog;
        if !d.can_ask() || d.stage == Stage::Asking {
            return;
        }
        d.stage = Stage::Asking;
        d.error = None;
        d.ticket += 1;
        let (name, request, ticket) = (d.skill.clone(), d.request(), d.ticket);
        self.skill_ask_for(&name, &request, ticket);
    }

    /// The answer to a question: the diff, or back to the form with the server's words.
    pub(super) fn improve_asked(&mut self, ticket: u32) {
        let answer = self.skills.proposal.clone();
        let d = &mut self.skills.dialog;
        if ticket == 0 || !d.open || d.ticket != ticket {
            return;
        }
        match answer {
            Some(Ok(p)) => {
                d.draft = TextEdit::new(p.content.clone());
                d.proposal = Some(p);
                d.editing = false;
                d.stage = Stage::Proposal;
            }
            Some(Err(why)) => {
                d.error = Some(why);
                d.stage = Stage::Compose;
            }
            None => {}
        }
    }

    /// Keep it (`keep`).
    pub fn improve_keep(&mut self) {
        let d = &mut self.skills.dialog;
        let Some(proposal) = d.proposal.as_ref().filter(|_| !d.saving) else { return };
        let previous = proposal.base_version;
        d.saving = true;
        d.error = None;
        d.ticket += 1;
        let (name, text, ticket) = (d.skill.clone(), d.draft.text.clone(), d.ticket);
        let note = d.note.text.trim().to_string();
        let session = Some(d.session_id.clone()).filter(|id| !id.is_empty());
        self.skill_keep_for(&name, &text, Some(&note), session.as_deref(), ticket, previous);
    }

    /// The answer to Keep. A saved version is announced whether or not the dialog is still there; the dialog closes, or
    /// says why it couldn't save.
    pub(super) fn improve_kept(&mut self, ticket: u32, previous: Option<u32>) {
        let Some(answer) = self.skills.saved.clone().filter(|_| ticket != 0) else { return };
        let mine = self.skills.dialog.open && self.skills.dialog.ticket == ticket;
        if mine {
            self.skills.dialog.saving = false;
        }
        match answer {
            Ok(saved) => {
                self.skills_update(saved.clone());
                if mine {
                    self.improve_close();
                }
                self.improve_announce(&saved, previous);
            }
            Err(why) if mine => self.skills.dialog.error = Some(why),
            Err(_) => {}
        }
    }

    /// A quiet notice with Undo, which puts back the version sessions got before (`announce`).
    pub(crate) fn improve_announce(&mut self, saved: &BuiltInSkill, previous: Option<u32>) {
        let id = format!("skill-version-{}-{}", saved.name, saved.version.unwrap_or(0));
        let notice = Notice {
            body: Some("Sessions you start from now on use it.".into()),
            icon: Some(IconRef(&SPARKLES)),
            countdown: true,
            actions: vec![Action { label: "Undo".into(), icon: None, tone: ActionTone::Quiet, run: format!("{UNDO_PREFIX}{id}") }],
            ..Notice::new(&id, format!("{} is now your version {}", saved.name, saved.version.unwrap_or(0)))
        };
        self.skills.dialog.undo.insert(id, (saved.name.clone(), previous));
        self.notices.post(notice, self.clock);
    }

    /// Runs the Undo the card's button told about, on a thread: the card closes, or says why it couldn't.
    pub fn keep_skill_undo(&mut self) {
        for run in self.notices.take_heard_with(UNDO_PREFIX) {
            let id = run[UNDO_PREFIX.len()..].to_string();
            let Some((name, previous)) = self.skills.dialog.undo.get(&id).cloned() else { continue };
            let Some(link) = self.link.clone() else {
                self.skill_undone(&id, Err("Fleet isn't reachable.".into()));
                continue;
            };
            let (path, body, fallback) = (format!("{}/active", at(&name)), switch_body(previous), format!("Couldn't switch {name}."));
            link.off_thread(
                move |base| answer(&fallback, http::text_with("PUT", base, &path, Some(&body))),
                move |s, _, result| {
                    let result = result.and_then(|v| parse_saved(&v).ok_or_else(|| "No skill came back.".to_string()));
                    s.skill_undone(&id, result)
                },
            );
        }
    }

    pub fn skill_undone(&mut self, id: &str, result: Result<BuiltInSkill, String>) {
        match result {
            Ok(skill) => {
                self.skills_update(skill);
                self.skills.dialog.undo.remove(id);
                self.notices.remove(id);
            }
            Err(why) => self.notices.update(id, |n| {
                n.title = "Couldn't undo it".into();
                n.body = Some(why);
                n.actions.clear();
            }),
        }
    }

    /// The list takes the skill's new state (`store.update`).
    fn skills_update(&mut self, saved: BuiltInSkill) {
        if let Some(list) = self.skills.list.as_mut() {
            match list.iter_mut().find(|s| s.name == saved.name) {
                Some(slot) => *slot = saved,
                None => list.push(saved),
            }
        }
    }
}
