//! Which harness a new session starts on, and why none can (`composables/use-enabled-harnesses.ts`; rows A10.1-A10.3).
//! Pure: no Rune, no I/O. The list is `GET /api/harnesses`; the preferred one is the `defaultHarnessType` preference of the
//! machine the page asks (another machine's own preferences, or the one saved from the last visit, shown at once).
use serde_json::Value;

/// The preference that keeps the user's default harness (`DEFAULT_HARNESS_PREFERENCE_KEY`).
pub const DEFAULT_HARNESS_PREFERENCE_KEY: &str = "defaultHarnessType";

/// One harness of `GET /api/harnesses`, with the fields this choice reads (`HarnessInfo`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HarnessEntry {
    pub kind: String,
    pub display_name: String,
    pub available: bool,
    pub user_enabled: bool,
    /// Why it isn't available, in a sentence the user can act on.
    pub reason: Option<String>,
    /// A session that names no harness starts on this one: the user's pick, or the server's fallback.
    pub is_default: bool,
    pub supports_profiles: bool,
}

impl HarnessEntry {
    /// A harness that is installed and turned on.
    pub fn ready(kind: &str) -> HarnessEntry {
        HarnessEntry { kind: kind.into(), display_name: kind.into(), available: true, user_enabled: true, ..HarnessEntry::default() }
    }
}

/// The harnesses of `GET /api/harnesses`; an entry with no `type` is skipped.
pub fn parse_harness_entries(body: &Value) -> Vec<HarnessEntry> {
    let flag = |h: &Value, k: &str| h.get(k).and_then(Value::as_bool) == Some(true);
    body.as_array()
        .into_iter()
        .flatten()
        .filter_map(|h| {
            let kind = h.get("type")?.as_str()?.to_string();
            let text = |k: &str| h.get(k).and_then(Value::as_str).map(str::to_string);
            Some(HarnessEntry {
                display_name: text("displayName").unwrap_or_else(|| kind.clone()),
                kind,
                available: flag(h, "available"),
                user_enabled: flag(h, "userEnabled"),
                reason: text("reason"),
                is_default: flag(h, "isDefault"),
                supports_profiles: h.pointer("/capabilities/supportsProfiles").and_then(Value::as_bool) == Some(true),
            })
        })
        .collect()
}

/// Where the list is: asked for the first time, failed, or answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Load {
    #[default]
    Loading,
    Failed,
    Done,
}

/// The harness list as the page reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Harnesses {
    pub all: Vec<HarnessEntry>,
    pub load: Load,
    /// The user's default (the machine's `defaultHarnessType` preference); `None` or empty when none is set.
    pub preferred: Option<String>,
}

/// The harness a session that names none starts on: the user's pick, else the one the server marks, else the first. Empty
/// while there are no harnesses (`resolveDefaultHarness`).
pub fn resolve_default_harness(preferred: Option<&str>, harnesses: &[HarnessEntry]) -> String {
    preferred
        .filter(|p| !p.is_empty())
        .or_else(|| harnesses.iter().find(|h| h.is_default).map(|h| h.kind.as_str()))
        .or_else(|| harnesses.first().map(|h| h.kind.as_str()))
        .unwrap_or("")
        .to_string()
}

impl Harnesses {
    /// Available and turned on (A10.1).
    pub fn enabled(&self) -> Vec<&HarnessEntry> {
        self.all.iter().filter(|h| h.available && h.user_enabled).collect()
    }

    pub fn default_type(&self) -> String {
        resolve_default_harness(self.preferred.as_deref(), &self.all)
    }

    /// Why no harness can start a session: the default harness's reason ("OpenCode isn't installed: …"), or that every harness
    /// is turned off. `None` while the list first loads, when it failed to load (the session starts and reports its own error),
    /// or when one is ready.
    pub fn no_harness_reason(&self) -> Option<String> {
        let first_load = self.load == Load::Loading && self.all.is_empty();
        if first_load || self.load == Load::Failed || !self.enabled().is_empty() {
            return None;
        }
        let turned_on: Vec<&HarnessEntry> = self.all.iter().filter(|h| h.user_enabled).collect();
        let default = self.default_type();
        let Some(wanted) = turned_on.iter().find(|h| h.kind == default).or(turned_on.first()) else {
            return Some("Every harness is turned off.".into());
        };
        Some(wanted.reason.clone().unwrap_or_else(|| format!("{} isn't ready.", wanted.display_name)))
    }

    /// The harness a draft really uses (`resolvedHarnessType`): its own if enabled, else the default if enabled, else the
    /// first enabled, else the default. The page rewrites the draft's type to this when the stored one is no longer enabled.
    pub fn resolved_type(&self, draft: &str) -> String {
        let enabled = self.enabled();
        let on = |t: &str| enabled.iter().any(|h| h.kind == t);
        let default = self.default_type();
        if on(draft) {
            draft.to_string()
        } else if on(&default) {
            default
        } else {
            enabled.first().map(|h| h.kind.clone()).unwrap_or(default)
        }
    }

    /// The harness picker shows when there is a choice (A10.2).
    pub fn shows_picker(&self) -> bool {
        self.enabled().len() > 1
    }
}
