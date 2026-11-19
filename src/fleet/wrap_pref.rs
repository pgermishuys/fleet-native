//! Whether a file's code view wraps long lines (`lib/code-editor/wrap-preference.ts`, row 5.5 of docs/parity/11-files.md). Wrap is on
//! for every file; only a file turned off is remembered, the latest 200 of them, so the settings file doesn't grow with every file
//! ever opened.
use serde_json::Value;

/// `WRAP_BY_DEFAULT`.
pub const WRAP_BY_DEFAULT: bool = true;
/// `MAX_FILES`.
pub const MAX_FILES: usize = 200;

/// The files that differ from the default, oldest first (Fleet's insertion order is age).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WrapPrefs {
    differs: Vec<String>,
}

impl WrapPrefs {
    /// `wrapFor`.
    pub fn wrap_for(&self, path: &str) -> bool {
        if self.differs.iter().any(|p| p == path) { !WRAP_BY_DEFAULT } else { WRAP_BY_DEFAULT }
    }

    /// `setWrapFor`: the newest choice goes last, the oldest drop off the front. Returns whether anything changed.
    pub fn set_wrap_for(&mut self, path: &str, wrap: bool) -> bool {
        let before = self.differs.clone();
        self.differs.retain(|p| p != path);
        if wrap != WRAP_BY_DEFAULT {
            self.differs.push(path.to_string());
        }
        let over = self.differs.len().saturating_sub(MAX_FILES);
        self.differs.drain(..over);
        self.differs != before
    }

    /// What is stored: a list of paths. Anything else (a broken value) reads as no choices.
    pub fn parse(value: Option<&Value>) -> Self {
        let Some(Value::Array(items)) = value else { return Self::default() };
        let mut prefs = Self::default();
        for p in items.iter().filter_map(Value::as_str) {
            prefs.set_wrap_for(p, !WRAP_BY_DEFAULT);
        }
        prefs
    }

    pub fn to_json(&self) -> Option<Value> {
        (!self.differs.is_empty()).then(|| serde_json::json!(self.differs))
    }

    pub fn len(&self) -> usize {
        self.differs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.differs.is_empty()
    }
}

// wrap-preference.test.ts
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_by_default() {
        const { assert!(WRAP_BY_DEFAULT) };
        assert!(WrapPrefs::default().wrap_for("src/app.ts"));
    }

    #[test]
    fn remembers_a_file_turned_off_and_only_that_file() {
        let mut p = WrapPrefs::default();
        p.set_wrap_for("src/app.ts", false);
        assert!(!p.wrap_for("src/app.ts"));
        assert!(p.wrap_for("src/other.ts"));
    }

    #[test]
    fn keeps_only_choices_that_differ_from_the_default() {
        let mut p = WrapPrefs::default();
        p.set_wrap_for("src/app.ts", false);
        p.set_wrap_for("src/app.ts", true);
        assert!(p.wrap_for("src/app.ts"));
        assert!(p.is_empty());
        assert_eq!(p.to_json(), None);
    }

    #[test]
    fn keeps_the_latest_200_files() {
        let mut p = WrapPrefs::default();
        for i in 0..205 {
            p.set_wrap_for(&format!("src/file-{i}.ts"), false);
        }
        assert_eq!(p.len(), 200);
        assert!(p.wrap_for("src/file-0.ts"));
        assert!(!p.wrap_for("src/file-204.ts"));
    }

    #[test]
    fn ignores_a_broken_stored_value() {
        for broken in [serde_json::json!("not json"), serde_json::json!({"a": false}), serde_json::json!(7)] {
            assert!(WrapPrefs::parse(Some(&broken)).wrap_for("src/app.ts"));
        }
        assert!(WrapPrefs::parse(None).is_empty());
        // A good list with a bad entry in it keeps the good ones.
        let p = WrapPrefs::parse(Some(&serde_json::json!(["a.rs", 3, "b.rs"])));
        assert!(!p.wrap_for("a.rs") && !p.wrap_for("b.rs") && p.len() == 2);
    }

    #[test]
    fn reads_back_what_it_wrote() {
        let mut p = WrapPrefs::default();
        p.set_wrap_for("a.rs", false);
        p.set_wrap_for("b.rs", false);
        assert_eq!(WrapPrefs::parse(p.to_json().as_ref()), p);
    }
}
