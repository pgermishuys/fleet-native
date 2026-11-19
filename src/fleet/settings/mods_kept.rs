//! The mods a user kept, as plain data (`stores/mods.ts`, `lib/mods/kept.ts`, `lib/mods/kept-api.ts`): how `GET /api/mods` and
//! `GET /api/features/mods` are read, the calls under `/api/mods`, the words of Undo, and what the Mods tab keeps while it is
//! open. No Rune and no blocking but [`call`], which a thread runs; what touches the app is `mods_kept_actions.rs`.
use crate::conv::export_history::encode_component;
use crate::fleet::http;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// One version of a mod the user kept (`KeptModVersion`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Version {
    pub number: i64,
    pub created_at: String,
    pub session_id: Option<String>,
    pub session_title: Option<String>,
}

/// Why a mod is off (`KeptModOff`): `user` turned it off, or `strikes` (three failures) did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Off {
    pub by: String,
    pub error: Option<String>,
}

impl Off {
    pub fn by_strikes(&self) -> bool {
        self.by == "strikes"
    }
}

/// `KeptMod`. `active` is the version number that runs; the server sends none only before anything is kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeptMod {
    pub name: String,
    pub description: Option<String>,
    pub active: Option<i64>,
    pub active_version: Option<String>,
    pub off: Option<Off>,
    pub versions: Vec<Version>,
}

impl KeptMod {
    pub fn is_on(&self) -> bool {
        self.off.is_none()
    }

    /// The version that runs, whose date and session the row shows.
    pub fn active_entry(&self) -> Option<&Version> {
        let n = self.active?;
        self.versions.iter().find(|v| v.number == n)
    }

    /// "Undo (back to v1)" on v2 and later, else "Undo (turn off)": the server's rule is that on vN (N > 1) the previous version
    /// returns and the mod turns on, and on v1 the mod turns off.
    pub fn undo_label(&self) -> String {
        match self.active.unwrap_or(1) {
            n if n > 1 => format!("Undo (back to v{})", n - 1),
            _ => "Undo (turn off)".into(),
        }
    }

    /// Why Undo can't be pressed, when it can't.
    pub fn undo_refusal(&self) -> Option<&'static str> {
        match self.active {
            None => Some("Nothing to undo: no version is kept yet."),
            Some(1) if !self.is_on() => Some("Nothing to undo: v1 is the first version and the mod is already off."),
            _ => None,
        }
    }
}

/// `ModsView`: the kept mods, and whether Fleet is running without them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub safe_mode: bool,
    pub mods: Vec<KeptMod>,
}

/// `ModsSwitch`: what `GET /api/features/mods` says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switch {
    pub on: bool,
    pub safe_mode: bool,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn parse_version(v: &Value) -> Option<Version> {
    Some(Version {
        number: v.get("number").and_then(Value::as_i64)?,
        created_at: text(v, "createdAt").unwrap_or_default(),
        session_id: text(v, "sessionId").filter(|s| !s.is_empty()),
        session_title: text(v, "sessionTitle").filter(|s| !s.is_empty()),
    })
}

/// One mod; an answer with no name is not a mod.
pub fn parse_mod(v: &Value) -> Option<KeptMod> {
    let name = text(v, "name").filter(|n| !n.is_empty())?;
    Some(KeptMod {
        name,
        description: text(v, "description").filter(|d| !d.is_empty()),
        active: v.get("active").and_then(Value::as_i64),
        active_version: text(v, "activeVersion").filter(|d| !d.is_empty()),
        off: v
            .get("off")
            .filter(|o| o.is_object())
            .map(|o| Off { by: text(o, "by").unwrap_or_default(), error: text(o, "error").filter(|e| !e.is_empty()) }),
        versions: v.get("versions").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_version).collect()).unwrap_or_default(),
    })
}

pub fn parse_view(v: &Value) -> View {
    View {
        safe_mode: v.get("safeMode").and_then(Value::as_bool).unwrap_or(false),
        mods: v.get("mods").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_mod).collect()).unwrap_or_default(),
    }
}

/// `fetchModsSwitch`: none for a 404, any failure or a body that isn't JSON.
pub fn parse_switch(answer: Result<(u16, String), String>) -> Option<Switch> {
    let (status, body) = answer.ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    let v: Value = serde_json::from_str(&body).ok()?;
    Some(Switch {
        on: v.get("on").and_then(Value::as_bool).unwrap_or(false),
        safe_mode: v.get("safeMode").and_then(Value::as_bool).unwrap_or(false),
    })
}

// ─── Requests ───────────────────────────────────────────────────────────────

pub const SWITCH: &str = "/api/features/mods";
pub const KEPT: &str = "/api/mods";
pub const SAFE_MODE: &str = "/api/mods/safe-mode";

pub fn mod_path(name: &str, tail: &str) -> String {
    format!("{KEPT}/{}{tail}", encode_component(name))
}

pub fn sample_path(name: &str) -> String {
    format!("{KEPT}/samples/{name}")
}

/// A refused request (`ModsRequestError`): the server's own words and its status (404 while the Mods switch is off).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

/// `request`: the body (null for none), else the server's words, else "Couldn't reach Mods (<status>)."
pub fn call(base: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Refused> {
    let (status, reply) =
        http::text_with(method, base, path, body).map_err(|_| Refused { status: 0, message: "Fleet couldn't be reached.".into() })?;
    if !(200..300).contains(&status) {
        let fallback = format!("Couldn't reach Mods ({status}).");
        return Err(Refused { status, message: super::skills::extract_error(&reply, &fallback) });
    }
    Ok(serde_json::from_str(&reply).unwrap_or(Value::Null))
}

pub fn read_view(base: &str) -> Result<View, Refused> {
    call(base, "GET", KEPT, None).map(|v| parse_view(&v))
}

pub fn read_switch(base: &str) -> Option<Switch> {
    parse_switch(http::text_with("GET", base, SWITCH, None))
}

/// `setModOn`, `undoMod`: the mod as it is now.
pub fn post_mod(base: &str, name: &str, tail: &str) -> Result<KeptMod, Refused> {
    let v = call(base, "POST", &mod_path(name, tail), None)?;
    parse_mod(&v).ok_or(Refused { status: 200, message: "Something went wrong.".into() })
}

pub fn put_safe_mode(base: &str, on: bool) -> Result<View, Refused> {
    call(base, "PUT", SAFE_MODE, Some(&json!({ "on": on }))).map(|v| parse_view(&v))
}

/// `add`: `POST /api/mods/samples/{name}`. Failure: the answer's `error` or `message`, else the status.
pub fn add_sample(base: &str, name: &str) -> Result<(), String> {
    let (status, reply) =
        http::text_with("POST", base, &sample_path(name), None).map_err(|_| "The sample couldn't be added.".to_string())?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let body: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
    Err(text(&body, "error").or_else(|| text(&body, "message")).unwrap_or_else(|| format!("The sample couldn't be added (HTTP {status}).")))
}

// ─── The tab ────────────────────────────────────────────────────────────────

/// What the Mods tab keeps while it is open (the refs of `ModsSection.vue`, `ModRow.vue` and `ModsShowcase.vue`, and the part
/// of the store it reads). Forgotten when the tab is left, as Fleet's unmounted component forgets its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModsTab {
    pub loaded_for: Option<String>,
    /// Numbers the tab's reads, so an answer to one the tab has left is dropped.
    pub seq: u64,
    pub loading: bool,
    pub error: Option<String>,
    pub switch: Option<Switch>,
    pub kept: Option<View>,
    /// "Turn mods back on" is running.
    pub saving: bool,
    /// The request a row is running (`on` or `undo`), by mod name: one per row at a time.
    pub busy: BTreeMap<String, &'static str>,
    pub row_errors: BTreeMap<String, String>,
    /// The sample being added (every Add is disabled meanwhile) and the refusal shown under one.
    pub adding: Option<String>,
    pub sample_error: Option<(String, String)>,
}

impl ModsTab {
    /// `safeMode`: the kept list's word, else the switch's.
    pub fn safe_mode(&self) -> bool {
        self.kept.as_ref().map(|k| k.safe_mode).or(self.switch.map(|s| s.safe_mode)).unwrap_or(false)
    }

    pub fn mods(&self) -> &[KeptMod] {
        self.kept.as_ref().map_or(&[], |k| &k.mods)
    }

    /// `isAdded`: a sample counts as added once a kept mod of its name is on; a kept one that is off can be added again.
    pub fn is_added(&self, name: &str) -> bool {
        self.mods().iter().any(|m| m.name == name && m.is_on())
    }

    /// `replaceMod`: the row takes the server's answer; a mod the list didn't have is added at the end.
    pub fn replace_mod(&mut self, m: KeptMod) {
        let Some(kept) = self.kept.as_mut() else { return };
        match kept.mods.iter_mut().find(|k| k.name == m.name) {
            Some(slot) => *slot = m,
            None => kept.mods.push(m),
        }
    }

    pub fn row_busy(&self, name: &str) -> bool {
        self.busy.contains_key(name)
    }
}

/// `isSwitchedOn`: the user's choice (the `Mods` preference) as soon as it is made, else the switch's answer, else off.
pub fn switched_on(preference: Option<&str>, switch: Option<Switch>) -> bool {
    match preference {
        Some("") | None => switch.is_some_and(|s| s.on),
        Some(chosen) => chosen == "true",
    }
}

/// The reasons `mods.changed` carries that change the kept list.
pub fn is_kept_reason(reason: &str) -> bool {
    matches!(reason, "kept" | "version" | "undone" | "on" | "off" | "strikes")
}
