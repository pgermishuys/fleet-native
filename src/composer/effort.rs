//! The reasoning effort remembered for a model (`lib/effort-memory.ts`, `composables/use-effort-memory.ts`): a
//! server preference per harness and model, so every device agrees. "" is Default (the model's own level) and is
//! stored as an empty value, which reads as no memory.
use serde_json::Value;
use std::collections::HashMap;

/// What the server reads as "go back to the model's own level"; only the composer sends it.
pub const MODEL_DEFAULT_EFFORT: &str = "fleet:default";
/// How long "Opus 5.5: back on High" stays after a model switch brings a level back, in seconds.
pub const NOTE_FOR: f64 = 4.0;

/// The catalog's selection key is a JSON pair `["provider","model"]`; the memory key reads `provider:model`.
fn model_part(model_key: &str) -> String {
    if let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(model_key) {
        let strings: Option<Vec<&str>> = parts.iter().map(Value::as_str).collect();
        if let Some(strings) = strings {
            return strings.join(":");
        }
    }
    model_key.to_string()
}

/// `effortPreferenceKey`: one key per harness and model; a model's slash becomes a colon so the key sits in one
/// URL segment.
pub fn effort_preference_key(harness_type: Option<&str>, model_key: &str) -> String {
    format!("effort.{}.{}", harness_type.unwrap_or("unknown"), model_part(model_key).replace('/', ":"))
}

/// The remembered level when the model still offers it, else "" (Default). A level the model dropped is ignored,
/// not deleted.
pub fn remembered_effort(stored: Option<&str>, variants: &[String]) -> String {
    match stored {
        Some(level) if !level.is_empty() && variants.iter().any(|v| v == level) => level.to_string(),
        _ => String::new(),
    }
}

pub fn effort_label(level: &str) -> String {
    let mut chars = level.chars();
    match chars.next() {
        None => "Default".into(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// The effort a prompt carries: the level picked, or, for Default on a model that has levels, the explicit reset,
/// so a harness whose process still runs at an earlier pick goes back to the default. Models without levels send
/// nothing.
pub fn effort_to_send(picked: &str, variants: Option<&[String]>) -> Option<String> {
    if !picked.is_empty() {
        return Some(picked.to_string());
    }
    variants.filter(|v| !v.is_empty()).map(|_| MODEL_DEFAULT_EFFORT.to_string())
}

/// The cycle the toggle's click walks: every level, then Default, then round again (row 4.3).
pub fn next_effort(current: &str, variants: &[String]) -> String {
    let cycle: Vec<&str> = variants.iter().map(String::as_str).chain(std::iter::once("")).collect();
    match cycle.iter().position(|level| *level == current) {
        Some(i) => cycle[(i + 1) % cycle.len()].to_string(),
        None => cycle[0].to_string(),
    }
}

/// What the memory reads each time the composer's state changes: the same things `useEffortMemory` is given.
#[derive(Debug, Clone, Default)]
pub struct Input<'a> {
    /// The preferences have been fetched (the stored levels are known).
    pub fetched: bool,
    pub harness_type: Option<&'a str>,
    /// The model the next prompt runs on (picked, else the session's own); "" while nothing names one.
    pub model_key: &'a str,
    pub model_name: &'a str,
    /// The model picked in the composer ("" when none): only a pick says "back on High".
    pub picked: &'a str,
    /// The levels that model offers; none means it has no effort.
    pub variants: &'a [String],
}

/// The memory's state machine. The draft's effort is owned by the caller and passed in; `preferences` is the
/// server's preference map, which `pick` also updates (the caller sends the returned write).
#[derive(Debug, Default)]
pub struct EffortMemory {
    /// What the last run saw, so a run changes things only when `[fetched, key, levels]` changed (Vue's `watch`).
    seen: Option<(bool, String, String)>,
    applied_for: Option<String>,
    last_picked: String,
    note: Option<(String, f64)>,
}

impl EffortMemory {
    /// `picked` is what the composer holds at the start, so opening a draft on a pick doesn't count as a switch.
    pub fn new(picked: &str) -> Self {
        Self { last_picked: picked.to_string(), ..Self::default() }
    }

    /// The note to show now, until it times out.
    pub fn note(&self, now: f64) -> Option<&str> {
        self.note.as_ref().filter(|(_, until)| now < *until).map(|(text, _)| text.as_str())
    }

    /// Run after any change to the input or to the stored levels. The draft takes the remembered level when the
    /// draft opens and when the model changes (a pick or a late catalog), but not over a level it already has on
    /// first run. Returns true when the draft's effort changed.
    pub fn update(&mut self, input: &Input, draft_effort: &mut String, preferences: &HashMap<String, String>, now: f64) -> bool {
        let levels = input.variants.join(",");
        let current = effort_preference_key(input.harness_type, input.model_key);
        let watched = (input.fetched, current.clone(), levels.clone());
        if self.seen.as_ref() == Some(&watched) {
            return false;
        }
        self.seen = Some(watched);
        if !input.fetched || input.model_key.is_empty() {
            return false;
        }
        let picked_changed = input.picked != self.last_picked;
        self.last_picked = input.picked.to_string();
        let first = self.applied_for.is_none();
        let changed = self.applied_for.as_deref() != Some(current.as_str());
        // A session on a named model knows its key before the catalog brings the model's levels. Until they're
        // here nothing is settled, so the levels arriving count as the draft opening.
        self.applied_for = if levels.is_empty() { None } else { Some(current.clone()) };
        if (first && !draft_effort.is_empty()) || !changed {
            return false;
        }
        let before = draft_effort.clone();
        *draft_effort = remembered_effort(preferences.get(&current).map(String::as_str), input.variants);
        if picked_changed && !draft_effort.is_empty() && *draft_effort != before {
            let name = if input.model_name.is_empty() { "This model" } else { input.model_name };
            self.note = Some((format!("{name}: back on {}", effort_label(draft_effort)), now + NOTE_FOR));
        }
        *draft_effort != before
    }

    /// A pick in the composer: the draft holds it and the machine remembers it for this model (Default forgets).
    /// Returns the preference to write (`PUT /api/preferences/<key>`), the key and the value.
    pub fn pick(
        &mut self,
        input: &Input,
        level: &str,
        draft_effort: &mut String,
        preferences: &mut HashMap<String, String>,
    ) -> (String, String) {
        *draft_effort = level.to_string();
        self.note = None;
        let key = effort_preference_key(input.harness_type, input.model_key);
        preferences.insert(key.clone(), level.to_string());
        (key, level.to_string())
    }
}
