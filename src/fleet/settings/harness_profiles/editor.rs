//! The profile editor (`HarnessProfileEditor.vue`): the name and the config being written, what Test and Save asked and what
//! the harness answered, and Duplicate and Delete. It lives in the panel while an editor is open and goes when it closes.
use super::{Check, Editing, NEW_PROFILE_CONTENT, parse_jsonc};
use crate::State;
use rune::TextEdit;

/// `maxlength` of the name box.
pub const MAX_NAME_CHARS: usize = 60;
/// What an empty name saves as.
pub const UNTITLED: &str = "Untitled profile";

/// What the editor is busy with: one thing at a time (`busy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Test,
    Save,
    Delete,
}

/// The editor's form: `name` and `content` are the boxes' text; `original` is what the profile was when it opened, to know
/// whether Save changed the config.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub name: TextEdit,
    pub content: TextEdit,
    pub original: Option<String>,
    pub busy: Option<Busy>,
    pub result: Option<Check>,
    pub confirming_delete: bool,
}

// A text edit is equal to itself.
impl Eq for Draft {}

impl Draft {
    /// What opens for `editing`: a profile as it is, a copy's name and text, or a new profile's starter text.
    pub fn open(editing: &Editing, profile: Option<(&str, &str)>) -> Draft {
        let (name, content, original) = match (editing, profile) {
            (Editing::Edit(_), Some((name, content))) => (name.to_string(), content.to_string(), Some(content.to_string())),
            (Editing::New(Some((name, content))), _) => (name.clone(), content.clone(), None),
            _ => (String::new(), NEW_PROFILE_CONTENT.to_string(), None),
        };
        Draft { name: TextEdit::new(name), content: TextEdit::new(content), original, ..Draft::default() }
    }

    /// `syntaxProblem`: shown when the text is not empty and doesn't read as JSON with comments.
    pub fn syntax_problem(&self) -> Option<&'static str> {
        let text = self.content.text.as_str();
        (!text.trim().is_empty() && parse_jsonc(text).is_none())
            .then_some("This isn't valid JSON yet. Comments and trailing commas are fine.")
    }

    /// The name that is saved.
    pub fn final_name(&self) -> String {
        let name = self.name.text.trim();
        if name.is_empty() { UNTITLED.to_string() } else { name.to_string() }
    }

    /// The label of the config box: "{name or Profile} config".
    pub fn config_label(&self) -> String {
        let name = self.name.text.as_str();
        format!("{} config", if name.is_empty() { "Profile" } else { name })
    }
}

/// What a close says after a save: the config changed under sessions that are open, or only a line (`save`).
pub fn saved_words(name: &str, content_changed: bool, open: u32) -> String {
    if content_changed && open > 0 {
        format!(
            "Saved {name}. New sessions use it now; the {open} open {} it up when Fleet restarts.",
            if open == 1 { "session picks" } else { "sessions pick" }
        )
    } else {
        format!("Saved {name}.")
    }
}

impl State {
    fn draft_of(&mut self) -> Option<(String, &mut Draft)> {
        let panel = &mut self.settings.harnesses.profiles;
        let kind = panel.harness.clone();
        panel.draft.as_mut().map(|d| (kind, d))
    }

    /// `maxlength="60"`: what is typed past it is cut. Called each frame while an editor is open; true when it cut.
    pub fn limit_profile_name(&mut self) -> bool {
        let Some((_, d)) = self.draft_of() else { return false };
        if d.name.text.chars().count() <= MAX_NAME_CHARS {
            return false;
        }
        d.name.replace_text(d.name.text.chars().take(MAX_NAME_CHARS).collect::<String>());
        true
    }

    /// Test: the harness tries the text and says whether it loads it; nothing is saved.
    pub fn test_profile(&mut self) {
        let Some((kind, d)) = self.draft_of() else { return };
        if d.busy.is_some() {
            return;
        }
        d.busy = Some(Busy::Test);
        let content = d.content.text.clone();
        self.check_profile(&kind, &content, |s, answer| {
            if let Some((_, d)) = s.draft_of() {
                d.busy = None;
                d.result = Some(answer.unwrap_or_else(|why| Check { ok: false, error: Some(why), details: None }));
            }
        });
    }

    /// Save: a new profile is created, an existing one updated; the machine tries the harness first and a refusal stays in the
    /// editor as the result.
    pub fn save_profile(&mut self) {
        let editing = self.settings.harnesses.profiles.editing.clone();
        let Some((kind, d)) = self.draft_of() else { return };
        if d.busy.is_some() {
            return;
        }
        d.busy = Some(Busy::Save);
        d.result = None;
        let (name, content, changed) = (d.final_name(), d.content.text.clone(), d.original.as_deref() != Some(d.content.text.as_str()));
        let failed = |s: &mut State, why: String| {
            if let Some((_, d)) = s.draft_of() {
                d.busy = None;
                d.result = Some(Check { ok: false, error: Some(why), details: None });
            }
        };
        match editing {
            Some(Editing::Edit(id)) => {
                let shut = kind.clone();
                self.update_profile(&kind, &id, &name, &content, move |s, answer| match answer {
                    Ok(saved) => s.close_profile_editor(&shut, Some(saved_words(&saved.name, changed, saved.open_sessions))),
                    Err(why) => failed(s, why),
                });
            }
            Some(Editing::New(_)) => {
                let shut = kind.clone();
                self.create_profile(&kind, &name, &content, move |s, answer| match answer {
                    Ok(made) => s.close_profile_editor(
                        &shut,
                        Some(format!("Created {}. Pick it from the profile chip when you start a session.", made.name)),
                    ),
                    Err(why) => failed(s, if why.is_empty() { "Couldn't save the profile.".into() } else { why }),
                });
            }
            None => {}
        }
    }

    /// Delete asks first (the button then says "Delete {name}?"), then deletes.
    pub fn delete_profile_asked(&mut self, name: &str, is_default: bool) {
        let Some(Editing::Edit(id)) = self.settings.harnesses.profiles.editing.clone() else { return };
        let Some((kind, d)) = self.draft_of() else { return };
        if d.busy.is_some() {
            return;
        }
        if !d.confirming_delete {
            d.confirming_delete = true;
            return;
        }
        d.busy = Some(Busy::Delete);
        let (name, shut) = (name.to_string(), kind.clone());
        self.delete_profile(&kind, &id, move |s, answer| match answer {
            Ok(()) => s.close_profile_editor(
                &shut,
                Some(if is_default { format!("Deleted {name}. New sessions start with no profile.") } else { format!("Deleted {name}.") }),
            ),
            Err(_) => {
                if let Some((_, d)) = s.draft_of() {
                    d.busy = None;
                    d.confirming_delete = false;
                    d.result = Some(Check { ok: false, error: Some("Couldn't delete the profile.".into()), details: None });
                }
            }
        });
    }

    /// Duplicate: a new profile named "{name or the profile's name} copy" with the text as it is now.
    pub fn duplicate_profile(&mut self, profile_name: &str) {
        let Some((kind, d)) = self.draft_of() else { return };
        let name = d.name.text.trim();
        let seed = (format!("{} copy", if name.is_empty() { profile_name } else { name }), d.content.text.clone());
        self.open_profile_editor(&kind, Editing::New(Some(seed)));
    }
}
