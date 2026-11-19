//! A built-in skill's detail, and the Editor dialog (`skills/SkillEditorDialog.vue`, rows 8.75 to 8.81): the skill's text in the
//! code box, what changed, and a note, saved as the user's next version. It opens from Settings' Edit, and from Improve's "Edit by
//! hand" with the text, note and session to start from. Every request goes to a thread; the answer comes back to the UI thread and
//! is dropped if the dialog was closed or opened again for something else (`seq`). With no Fleet to ask, the invented skill answers.
use crate::State;
use crate::fleet::http;
use crate::fleet::skills::{BuiltInSkill, answer, at, parse_saved, save_body};
use rune::TextEdit;
use serde_json::Value;

/// `maxlength` of the note.
pub const NOTE_MAX: usize = 2000;

/// One of the user's versions (`BuiltInSkillVersion`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Version {
    pub number: u32,
    pub created_at: String,
    pub note: Option<String>,
    pub session_title: Option<String>,
    pub active: bool,
}

/// `BuiltInSkillDetail`: the skill with both texts and the versions, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Detail {
    pub name: String,
    pub fleet_content: String,
    pub your_content: Option<String>,
    /// The version sessions get; `None` for Fleet's.
    pub version: Option<u32>,
    pub versions: Vec<Version>,
}

impl Detail {
    /// The text sessions get now (`yourContent ?? fleetContent`).
    pub fn current(&self) -> &str {
        self.your_content.as_deref().unwrap_or(&self.fleet_content)
    }

    /// Highest version number plus one, read off the newest (`versions[0]`); 1 when there are none.
    pub fn next_version(&self) -> u32 {
        self.versions.first().map_or(0, |v| v.number) + 1
    }
}

pub fn parse_detail(v: &Value) -> Option<Detail> {
    let text = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
    let version = |x: &Value| {
        Some(Version {
            number: x.get("number")?.as_u64()? as u32,
            created_at: text(x, "createdAt").unwrap_or_default(),
            note: text(x, "note").filter(|n| !n.is_empty()),
            session_title: text(x, "sessionTitle").filter(|n| !n.is_empty()),
            active: x.get("active").and_then(Value::as_bool).unwrap_or(false),
        })
    };
    Some(Detail {
        name: text(v, "name")?,
        fleet_content: text(v, "fleetContent").unwrap_or_default(),
        your_content: text(v, "yourContent"),
        version: v.get("version").and_then(Value::as_u64).map(|n| n as u32).filter(|n| *n > 0),
        versions: v.get("versions").and_then(Value::as_array).map(|a| a.iter().filter_map(version).collect()).unwrap_or_default(),
    })
}

/// What the Editor starts from instead of the current text (`initialContent`, `initialNote`, `sessionId`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Start {
    pub content: Option<String>,
    pub note: Option<String>,
    pub session_id: Option<String>,
}

/// The skill's own text, for a run with no server and for scenes.
pub fn mock_detail(name: &str, list: &[BuiltInSkill]) -> Detail {
    let version = list.iter().find(|s| s.name == name).and_then(|s| s.version);
    Detail { name: name.into(), fleet_content: crate::view::skill_diff::BEFORE.into(), version, ..Default::default() }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillEditor {
    pub open: bool,
    /// Bumped by every open and every save; an answer for an older one is dropped.
    seq: u64,
    pub name: String,
    /// Opened from Settings, so its requests go to the machine Settings shows; else to the one you're on.
    pub from_settings: bool,
    pub loading: bool,
    pub detail: Option<Detail>,
    pub draft: TextEdit,
    pub note: TextEdit,
    pub changes: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub session_id: Option<String>,
    /// The detail the last save answered with, for whoever opened the Editor (`saved`); taken once.
    pub saved: Option<BuiltInSkill>,
}

impl Eq for SkillEditor {}

impl SkillEditor {
    /// `changed`: the draft differs from the text sessions get.
    pub fn changed(&self) -> bool {
        self.detail.as_ref().is_some_and(|d| self.draft.text != d.current())
    }

    pub fn next_version(&self) -> u32 {
        self.detail.as_ref().map_or(1, Detail::next_version)
    }

    /// "Start from Fleet's version" shows while the user has a version and the draft isn't already Fleet's text.
    pub fn can_start_from_fleets(&self) -> bool {
        self.detail.as_ref().is_some_and(|d| d.your_content.is_some() && self.draft.text != d.fleet_content)
    }

    /// What the save sends: the note trimmed and cut to `maxlength`, null when empty.
    pub fn trimmed_note(&self) -> Option<String> {
        Some(self.note.text.trim().chars().take(NOTE_MAX).collect::<String>()).filter(|n| !n.is_empty())
    }
}

impl State {
    pub(crate) fn skill_link(&self, from_settings: bool) -> Option<crate::fleet::link::Link> {
        if from_settings { self.built_in_link() } else { self.link.clone() }
    }

    /// Opens the Editor on a skill: reset, then read it. `start` is what Improve's "Edit by hand" brings.
    pub fn skill_editor_open(&mut self, name: &str, start: Start, from_settings: bool) {
        let seq = self.settings.editor.seq + 1;
        let note = start.note.clone().unwrap_or_default();
        self.settings.editor = SkillEditor {
            open: true,
            seq,
            name: name.to_string(),
            from_settings,
            note: TextEdit::new(note),
            session_id: start.session_id.clone(),
            ..Default::default()
        };
        let Some(link) = self.skill_link(from_settings) else {
            let list = if from_settings { self.settings.built_in.list.clone() } else { self.skills.list.clone().unwrap_or_default() };
            self.skill_editor_loaded(Ok(mock_detail(name, &list)), start.content);
            return;
        };
        self.settings.editor.loading = true;
        let (path, fallback, initial) = (at(name), format!("Couldn't load {name}."), start.content);
        link.off_thread(
            move |base| answer(&fallback, http::text_with("GET", base, &path, None)),
            move |s: &mut State, _tx, got: Result<Value, String>| {
                if s.settings.editor.open && s.settings.editor.seq == seq {
                    s.skill_editor_loaded(got.and_then(|v| parse_detail(&v).ok_or_else(|| "No skill came back.".to_string())), initial);
                }
            },
        );
    }

    fn skill_editor_loaded(&mut self, got: Result<Detail, String>, initial: Option<String>) {
        let e = &mut self.settings.editor;
        e.loading = false;
        match got {
            Ok(detail) => {
                e.draft = TextEdit::new(initial.unwrap_or_else(|| detail.current().to_string()));
                e.detail = Some(detail);
            }
            Err(why) => e.error = Some(why),
        }
    }

    pub fn skill_editor_close(&mut self) {
        // Cancel is disabled while saving, and so is Escape.
        if !self.settings.editor.saving {
            self.settings.editor.open = false;
        }
    }

    pub fn skill_editor_toggle_changes(&mut self) {
        let e = &mut self.settings.editor;
        if e.changed() {
            e.changes = !e.changes;
        }
    }

    pub fn skill_editor_start_from_fleets(&mut self) {
        let e = &mut self.settings.editor;
        if let Some(d) = &e.detail {
            e.draft = TextEdit::new(d.fleet_content.clone());
        }
    }

    /// The server's answer for a skill goes into the lists Settings and the conversation share (`store.update`).
    fn skill_took(&mut self, skill: BuiltInSkill, from_settings: bool) {
        if from_settings {
            self.built_in_took(skill);
        } else {
            self.skills_update(skill);
        }
    }

    /// Save as version N (`save`): one at a time. A failure keeps the dialog and says why.
    pub fn skill_editor_save(&mut self) {
        let e = &mut self.settings.editor;
        if !e.open || !e.changed() || e.saving {
            return;
        }
        e.error = None;
        e.seq += 1;
        let (seq, name, from_settings) = (e.seq, e.name.clone(), e.from_settings);
        let (content, note, session) = (e.draft.text.clone(), e.trimmed_note(), e.session_id.clone());
        let Some(link) = self.skill_link(from_settings) else {
            // Invented: the skill takes the next version, in use.
            let number = self.settings.editor.next_version();
            let listed = self.skill_listed(&name, from_settings);
            let saved = BuiltInSkill { enabled: true, version: Some(number), version_count: listed.version_count + 1, ..listed };
            self.skill_editor_saved(Ok(saved), seq);
            return;
        };
        let e = &mut self.settings.editor;
        e.saving = true;
        let (path, body, fallback) = (
            format!("{}/versions", at(&name)),
            save_body(&content, note.as_deref(), session.as_deref()),
            format!("Couldn't save your version of {name}."),
        );
        link.off_thread(
            move |base| answer(&fallback, http::text_with("POST", base, &path, Some(&body))),
            move |s: &mut State, _tx, got: Result<Value, String>| {
                let got = got.and_then(|v| parse_saved(&v).ok_or_else(|| "No skill came back.".to_string()));
                s.skill_editor_saved(got, seq);
            },
        );
    }

    fn skill_listed(&self, name: &str, from_settings: bool) -> BuiltInSkill {
        let list: &[BuiltInSkill] =
            if from_settings { &self.settings.built_in.list } else { self.skills.list.as_deref().unwrap_or_default() };
        list.iter().find(|s| s.name == name).cloned().unwrap_or_default()
    }

    fn skill_editor_saved(&mut self, got: Result<BuiltInSkill, String>, seq: u64) {
        let e = &mut self.settings.editor;
        let mine = e.open && e.seq == seq;
        if mine {
            e.saving = false;
        }
        match got {
            Ok(skill) => {
                let from_settings = self.settings.editor.from_settings;
                self.skill_took(skill.clone(), from_settings);
                if mine {
                    self.settings.editor.saved = Some(skill);
                    self.settings.editor.open = false;
                }
            }
            Err(why) if mine => self.settings.editor.error = Some(why),
            Err(_) => {}
        }
    }
}
