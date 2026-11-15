//! The server's preferences (`stores/preferences.ts`): `GET /api/preferences` is `{key: value}` and
//! `PUT /api/preferences/{key}` takes `{value}`. A write shows at once; if the server refuses it the old value comes
//! back and the whole map is fetched again. Settings (component 16) shares this; no Rune and no I/O here.
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Prefs {
    pub values: HashMap<String, String>,
    /// The first answer has come (`hasFetched`): until then no stored level is known.
    pub fetched: bool,
}

impl Prefs {
    /// The map `GET /api/preferences` answers with; a value that isn't text keeps its JSON.
    pub fn parse(body: &Value) -> HashMap<String, String> {
        body.as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_str().map_or_else(|| v.to_string(), str::to_string))).collect()
    }

    pub fn adopt(&mut self, body: &Value) {
        self.values = Self::parse(body);
        self.fetched = true;
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Writes `value` locally and says what was there, for `roll_back`.
    pub fn set(&mut self, key: &str, value: &str) -> Option<String> {
        self.values.insert(key.to_string(), value.to_string())
    }

    /// The server refused `value`: put `before` back, unless something newer was written meanwhile.
    pub fn roll_back(&mut self, key: &str, value: &str, before: Option<String>) {
        if self.get(key) != Some(value) {
            return;
        }
        match before {
            Some(old) => self.values.insert(key.to_string(), old),
            None => self.values.remove(key),
        };
    }
}
