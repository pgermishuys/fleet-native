//! What "Don't ask again" allowed in a session, and the permission level it counts against. Ports `toRule` of
//! `composables/use-session-permission-rules.ts` and `lib/permissions.ts` (rows 5.4, 5.11). Plain Rust, no Rune.
use super::permission::kind_of;
use crate::conv::tools::PermissionKind;
use serde_json::Value;

/// One rule Fleet keeps for a session: a click on "Don't ask again" that allowed a tool, or some of its uses.
#[derive(Clone, Debug, PartialEq)]
pub struct AllowedRule {
    pub id: String,
    pub kind: PermissionKind,
    pub tool: String,
    /// What it allows, such as `bun test*`; empty (or only `*`) when it is every use of the tool.
    pub patterns: Vec<String>,
    pub created_at: String,
}

/// `toRule`: a rule without an `id` or a `tool` is ignored; `kind` defaults to "other".
pub fn to_rule(value: &Value) -> Option<AllowedRule> {
    let raw = value.as_object()?;
    Some(AllowedRule {
        id: raw.get("id")?.as_str()?.to_string(),
        kind: kind_of(raw.get("kind").and_then(Value::as_str)),
        tool: raw.get("tool")?.as_str()?.to_string(),
        patterns: raw
            .get("patterns")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
        created_at: raw.get("createdAt").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

/// `toRules`: what doesn't read is left out; anything but a list is no rules.
pub fn read_rules(body: &Value) -> Vec<AllowedRule> {
    body.as_array().map(|items| items.iter().filter_map(to_rule).collect()).unwrap_or_default()
}

/// Settings → Permissions: what a session's agent may do without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionLevel {
    Ask,
    Edits,
    All,
}

/// `PERMISSION_LEVEL_KEY`: the level every harness uses unless one is set for it.
pub const PERMISSION_LEVEL_KEY: &str = "PermissionLevel";

/// `harnessPermissionKey`: one harness's own level.
pub fn harness_permission_key(harness: &str) -> String {
    format!("{PERMISSION_LEVEL_KEY}.{harness}")
}

impl PermissionLevel {
    /// `toPermissionLevel`.
    pub fn parse(value: Option<&str>) -> Option<PermissionLevel> {
        match value {
            Some("ask") => Some(PermissionLevel::Ask),
            Some("edits") => Some(PermissionLevel::Edits),
            Some("all") => Some(PermissionLevel::All),
            _ => None,
        }
    }

    /// `PERMISSION_LEVEL_NAMES`.
    pub fn name(self) -> &'static str {
        match self {
            PermissionLevel::Ask => "Ask before changes",
            PermissionLevel::Edits => "Allow edits",
            PermissionLevel::All => "Allow everything",
        }
    }

    /// `usePermissionLevel`: the harness's own level, else the default level, else "all" (`DEFAULT_PERMISSION_LEVEL`).
    pub fn of(harness: Option<&str>, pref: impl Fn(&str) -> Option<String>) -> PermissionLevel {
        let own = harness.and_then(|h| PermissionLevel::parse(pref(&harness_permission_key(h)).as_deref()));
        own.or_else(|| PermissionLevel::parse(pref(PERMISSION_LEVEL_KEY).as_deref())).unwrap_or(PermissionLevel::All)
    }
}
