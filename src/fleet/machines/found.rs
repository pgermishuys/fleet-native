//! Add a machine → Found and → On a new server: the requests and the rules behind them (`stores/machines.ts` `loadFound`,
//! `makeJoinCode`; `FoundMachinesCard.vue`'s `codeFor`, `reachLabel`, `osLabel`).
//!
//! Both requests block and run on a thread, on home's link. Found is asked again every [`LOOK_EVERY`] while the card is open.
use crate::fleet::http;
use crate::fleet::machine::normalize_manual_code;
use crate::fleet::machines::read_error;
use serde_json::Value;
use std::time::Duration;

/// How often Found looks again (`LOOK_EVERY_MS`).
pub const LOOK_EVERY: Duration = Duration::from_secs(4);

/// A machine waiting to be added (someone ran `fleet node pair` or `fleet node up` on it), seen on Tailscale or the home
/// network. `hint` is the first half of the code its terminal shows; the rest is typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundMachine {
    pub machine_id: String,
    pub name: String,
    pub os: Option<String>,
    pub base_url: String,
    pub hint: String,
    /// "Tailscale" or the home network's own word.
    pub source: String,
    pub expires_at: String,
}

impl FoundMachine {
    fn from_json(v: &Value) -> Option<FoundMachine> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Some(FoundMachine {
            machine_id: text("machineId")?,
            name: text("name")?,
            os: text("os").filter(|o| !o.is_empty()),
            base_url: text("baseUrl")?,
            hint: text("hint").unwrap_or_default(),
            source: text("source").unwrap_or_default(),
            expires_at: text("expiresAt").unwrap_or_default(),
        })
    }

    /// "On Tailscale" or "On your home network" (`reachLabel`); the row adds ". Waiting to join."
    pub fn reach_label(&self) -> &'static str {
        if self.source == "Tailscale" { "On Tailscale" } else { "On your home network" }
    }

    /// "<name> · <OS>" (the row's name line).
    pub fn title(&self) -> String {
        match self.os.as_deref().map(os_label) {
            Some(os) => format!("{} · {os}", self.name),
            None => self.name.clone(),
        }
    }

    /// The code as typed: either the rest after the hint, or the whole code. None until it is a whole, matching code (`codeFor`).
    pub fn code_for(&self, typed: &str) -> Option<String> {
        let typed: String = typed.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_uppercase();
        if typed.is_empty() {
            return None;
        }
        let hint: String = self.hint.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
        let whole = if typed.chars().count() >= 8 { typed } else { format!("{hint}{typed}") };
        normalize_manual_code(&whole).filter(|code| code.replace('-', "").starts_with(&hint))
    }
}

/// `osLabel`: Fleet's name for a system.
pub fn os_label(os: &str) -> String {
    match os.to_ascii_lowercase().as_str() {
        "linux" => "Linux".into(),
        "macos" => "macOS".into(),
        "windows" => "Windows".into(),
        _ => os.to_string(),
    }
}

/// `loadFound`: the machines waiting. Empty when it sees none or this Fleet can't say (an older one, a failed read).
pub fn load_found(base: &str) -> Vec<FoundMachine> {
    let Ok((status, text)) = http::text_with("GET", base, "/api/machines/found", None) else { return Vec::new() };
    if !(200..300).contains(&status) {
        return Vec::new();
    }
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|b| b.get("machines").and_then(Value::as_array).map(|list| list.iter().filter_map(FoundMachine::from_json).collect()))
        .unwrap_or_default()
}

/// A one-time code a new server joins this Fleet with. `target` is what `install.sh --join` takes: the addresses, `#`, the code.
#[derive(Clone, PartialEq, Eq)]
pub struct JoinCode {
    pub code: String,
    pub expires_at: String,
    pub addresses: Vec<String>,
    pub target: String,
    pub machine_name: String,
}

// The code works once and is a secret until used: it stays out of a `Debug`.
impl std::fmt::Debug for JoinCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinCode").field("expires_at", &self.expires_at).field("machine_name", &self.machine_name).finish_non_exhaustive()
    }
}

/// `makeJoinCode`: one use, 30 minutes, with every address this machine can be reached at. The error is the server's reason
/// when this machine only listens on itself.
pub fn make_join_code(base: &str) -> Result<JoinCode, String> {
    let fallback = "Couldn't make a join code.";
    let (status, text) = http::text_with("POST", base, "/api/machines/join-code", None).map_err(|_| fallback.to_string())?;
    if !(200..300).contains(&status) {
        return Err(read_error(&text, fallback.into()));
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| fallback.to_string())?;
    let word = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(JoinCode {
        code: word("code").ok_or_else(|| fallback.to_string())?,
        expires_at: word("expiresAt").unwrap_or_default(),
        addresses: v
            .get("addresses")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        target: word("target").ok_or_else(|| fallback.to_string())?,
        machine_name: word("machineName").unwrap_or_default(),
    })
}
