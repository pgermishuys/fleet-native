//! The "Path to weave-copilot" field (`HarnessPathField.vue`): where Fleet finds the program a harness runs, for when it isn't on
//! PATH. Typing changes nothing until Check: it saves the path (when it changed) and lists the harnesses again.
use super::super::new_session::harness_choice::HarnessEntry;
use crate::State;
use rune::TextEdit;

/// The preference that holds GitHub Copilot's path.
pub const COPILOT_PATH_KEY: &str = "copilot.path";
/// What the program is called, and what an empty field does.
pub const COPILOT_PROGRAM: &str = "weave-copilot";
pub const COPILOT_EMPTY_MEANING: &str = "use the copy Fleet installed, or weave-copilot on PATH";

/// What the field keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathField {
    pub draft: TextEdit,
    /// The saved value the draft was last set from: a saved value that arrives later replaces the draft.
    pub synced: Option<String>,
    pub checking: bool,
    pub error: Option<String>,
}

impl Eq for PathField {}

/// The line under the box: the server's words for a failed save, else what Fleet found, else why it can't (`HarnessPathField`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    Error(String),
    Found(String),
    Reason(String),
    Nothing,
}

pub fn result(h: &HarnessEntry, error: Option<&str>) -> Shown {
    if let Some(e) = error {
        return Shown::Error(e.to_string());
    }
    let found = [h.version.as_deref(), h.executable_path.as_deref()].into_iter().flatten().filter(|p| !p.is_empty()).collect::<Vec<_>>();
    if h.available && !found.is_empty() {
        return Shown::Found(found.join(" \u{b7} "));
    }
    match (&h.reason, h.available) {
        (Some(reason), false) if !reason.is_empty() => Shown::Reason(reason.clone()),
        _ => Shown::Nothing,
    }
}

impl State {
    /// The path as saved on the machine shown.
    pub fn harness_path_saved(&self) -> String {
        self.harness_prefs().get(COPILOT_PATH_KEY).unwrap_or_default().to_string()
    }

    /// The saved path arrives after the preferences load, and replaces what was typed (`watch(saved)`). True when it did.
    pub fn keep_harness_path(&mut self) -> bool {
        if !self.harness_prefs().fetched {
            return false;
        }
        let saved = self.harness_path_saved();
        let field = &mut self.settings.harnesses.path;
        if field.synced.as_deref() == Some(saved.as_str()) {
            return false;
        }
        field.synced = Some(saved.clone());
        field.draft = TextEdit::new(saved);
        true
    }

    /// Check: saves the trimmed path when it differs from the saved one, then lists the harnesses again.
    pub fn check_harness_path(&mut self) {
        if self.settings.harnesses.path.checking {
            return;
        }
        let machine = self.harness_machine();
        let path = self.settings.harnesses.path.draft.text.trim().to_string();
        let saved = self.harness_path_saved();
        self.settings.harnesses.path.checking = true;
        self.settings.harnesses.path.error = None;
        if path == saved {
            self.read_harnesses_then(&machine, true, |s| s.settings.harnesses.path.checking = false);
            return;
        }
        let again = machine.clone();
        self.save_harness_pref(COPILOT_PATH_KEY, &path, move |s, result| match result {
            Ok(()) => s.read_harnesses_then(&again, true, |s| s.settings.harnesses.path.checking = false),
            Err(why) => {
                let field = &mut s.settings.harnesses.path;
                field.checking = false;
                field.error = Some(if why.is_empty() { format!("Fleet couldn't save the path to {COPILOT_PROGRAM}.") } else { why });
            }
        });
        // With no Fleet to ask the value is shown and nothing is on its way.
        if self.link.is_none() {
            self.settings.harnesses.path.checking = false;
        }
    }
}
