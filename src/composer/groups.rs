//! The model picker's list in sections (`lib/model-picker-groups.ts`) and the `modelPicker` preference that holds
//! the starred and recent models (`stores/model-picker.ts`).
use super::choice::ModelOption;
use serde_json::{Value, json};

/// More models than this from several providers get a header per provider.
const PROVIDER_HEADERS_ABOVE: usize = 8;
const RECENT_SHOWN: usize = 5;
const RECENT_PER_HARNESS: usize = 5;
/// The preference's name.
pub const PREFERENCE_KEY: &str = "modelPicker";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Group,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSection {
    pub title: Option<String>,
    pub level: Level,
    pub items: Vec<ModelOption>,
}

/// One entry per model per harness. Only ever compared, never parsed back.
pub fn model_picker_key(harness_type: &str, provider_id: &str, model_id: &str) -> String {
    format!("{harness_type}:{provider_id}/{model_id}")
}

/// The picker's unfiltered list: Favourites, Recent, then everything (a favourite stays in it too).
pub fn group_models(
    models: &[ModelOption],
    harness_type: &str,
    favourites: &[String],
    recent: &[String],
    show_recent: bool,
) -> Vec<ModelSection> {
    let listed = |keys: &[&String]| -> Vec<ModelOption> {
        keys.iter()
            .filter_map(|key| models.iter().find(|m| model_picker_key(harness_type, &m.provider_id, &m.id) == **key).cloned())
            .collect()
    };
    let favourite_items = listed(&favourites.iter().collect::<Vec<_>>());
    let prefix = format!("{harness_type}:");
    let recent_items = if show_recent {
        let keys: Vec<&String> = recent.iter().filter(|k| k.starts_with(&prefix) && !favourites.contains(k)).collect();
        listed(&keys).into_iter().take(RECENT_SHOWN).collect()
    } else {
        Vec::new()
    };

    let mut sections = Vec::new();
    if !favourite_items.is_empty() {
        sections.push(ModelSection { title: Some("Favourites".into()), level: Level::Group, items: favourite_items });
    }
    if !recent_items.is_empty() {
        sections.push(ModelSection { title: Some("Recent".into()), level: Level::Group, items: recent_items });
    }

    let titled = !sections.is_empty();
    let mut providers: Vec<&str> = Vec::new();
    for m in models {
        if !providers.contains(&m.provider.as_str()) {
            providers.push(&m.provider);
        }
    }
    if models.len() > PROVIDER_HEADERS_ABOVE && providers.len() > 1 {
        // The "All models" header stands alone above its provider headers.
        if titled {
            sections.push(ModelSection { title: Some("All models".into()), level: Level::Group, items: Vec::new() });
        }
        for provider in providers {
            sections.push(ModelSection {
                title: Some(provider.into()),
                level: Level::Provider,
                items: models.iter().filter(|m| m.provider == provider).cloned().collect(),
            });
        }
        return sections;
    }
    sections.push(ModelSection { title: titled.then(|| "All models".into()), level: Level::Group, items: models.to_vec() });
    sections
}

/// The `modelPicker` preference: starred and recently used models, kept with the other preferences so they follow
/// the user between devices. `hidden` and keys the model list no longer has stay as they were; nothing prunes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelPicker {
    pub favourites: Vec<String>,
    pub hidden: Vec<String>,
    pub recent: Vec<String>,
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

/// The harness part of a key: everything before the first colon (Fleet's `slice(0, indexOf(":"))`).
fn harness_of(key: &str) -> &str {
    match key.find(':') {
        Some(i) => &key[..i],
        None => key.char_indices().next_back().map_or("", |(i, _)| &key[..i]),
    }
}

impl ModelPicker {
    /// Reads the stored text; anything that isn't the right shape reads as empty.
    pub fn parse(raw: &str) -> Self {
        let value = serde_json::from_str::<Value>(raw).unwrap_or(Value::Null);
        Self { favourites: strings(value.get("favourites")), hidden: strings(value.get("hidden")), recent: strings(value.get("recent")) }
    }

    /// The text to store.
    pub fn to_json(&self) -> String {
        json!({ "favourites": self.favourites, "hidden": self.hidden, "recent": self.recent }).to_string()
    }

    pub fn is_favourite(&self, key: &str) -> bool {
        self.favourites.iter().any(|k| k == key)
    }

    /// The change a star click makes.
    pub fn toggle_favourite(&self, key: &str) -> Self {
        let mut next = self.clone();
        if self.is_favourite(key) {
            next.favourites.retain(|k| k != key);
        } else {
            next.favourites.push(key.to_string());
        }
        next
    }

    /// Moves the key to the front of this harness's recent models, once, keeping five; other harnesses' entries
    /// keep their place after them.
    pub fn record_recent(&self, key: &str) -> Self {
        let harness = harness_of(key);
        let mut mine = vec![key.to_string()];
        mine.extend(self.recent.iter().filter(|k| *k != key && harness_of(k) == harness).cloned());
        mine.truncate(RECENT_PER_HARNESS);
        mine.extend(self.recent.iter().filter(|k| harness_of(k) != harness).cloned());
        Self { recent: mine, ..self.clone() }
    }
}
