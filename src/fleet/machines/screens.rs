//! The phones and browsers that have keys (Settings → Machines → Phones and browsers): `lib/devices-api.ts`
//! (`listFleetScreens`, `listDevices`, `removeFleetScreen`) and `composables/use-home-access.ts` (`refreshDevices`).
//!
//! Requests block and run on a thread, on home's link. The Fleet-wide list is asked first; a Fleet too old to have it falls back to
//! this machine's own list.
use crate::fleet::http;
use crate::fleet::machines::read_error;
use serde_json::Value;

/// A phone or browser with its own key, or another machine paired with a code (`PairedDevice`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub id: String,
    pub name: String,
    pub platform: Option<String>,
    pub created_at: String,
    pub last_used_at: String,
    /// The machine that asked for this device's token on its behalf; None when it paired here.
    pub paired_via: Option<String>,
    /// Another Fleet paired with a code is a machine, not a screen.
    pub is_machine: bool,
    /// Your Fleet: the machine it connected through, by name.
    pub through_name: Option<String>,
    /// Remove reaches every machine by its Fleet-wide id.
    pub fleet_wide: bool,
}

/// A machine that didn't answer when the Fleet-wide list was asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotAnswering {
    pub id: String,
    pub name: String,
}

/// What a read of the list gave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screens {
    /// Phones and browsers only: another machine's key is not a screen.
    pub screens: Vec<Screen>,
    pub not_answering: Vec<NotAnswering>,
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn fleet_screen(v: &Value) -> Option<Screen> {
    Some(Screen {
        id: text(v, "id")?,
        name: text(v, "name")?,
        platform: text(v, "platform"),
        created_at: text(v, "createdAt").unwrap_or_default(),
        last_used_at: text(v, "lastUsedAt").unwrap_or_default(),
        paired_via: text(v, "throughId"),
        is_machine: false,
        through_name: text(v, "throughName"),
        fleet_wide: true,
    })
}

fn own_device(v: &Value) -> Option<Screen> {
    Some(Screen {
        id: text(v, "id")?,
        name: text(v, "name")?,
        platform: text(v, "platform"),
        created_at: text(v, "createdAt").unwrap_or_default(),
        last_used_at: text(v, "lastUsedAt").unwrap_or_default(),
        paired_via: text(v, "pairedVia"),
        is_machine: v.get("kind").and_then(Value::as_str) == Some("machine"),
        through_name: None,
        fleet_wide: false,
    })
}

fn get(base: &str, path: &str) -> Result<Value, String> {
    let (status, body) = http::text_with("GET", base, path, None)?;
    if !(200..300).contains(&status) {
        return Err(read_error(&body, format!("{path} answered {status}")));
    }
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

/// `listFleetScreens`: every phone and browser in the Fleet, each once; machines that don't answer are named.
pub fn list_fleet(base: &str) -> Result<Screens, String> {
    let body = get(base, "/api/machine/screens")?;
    let list = |k: &str| body.get(k).and_then(Value::as_array).ok_or_else(|| format!("no {k} in the list"));
    Ok(Screens {
        screens: list("screens")?.iter().filter_map(fleet_screen).collect(),
        not_answering: list("notAnswering")
            .map(|l| l.iter().filter_map(|m| Some(NotAnswering { id: text(m, "id")?, name: text(m, "name")? })).collect())
            .unwrap_or_default(),
    })
}

/// `listDevices`, minus the machines (`screens`).
pub fn list_own(base: &str) -> Screens {
    let devices = get(base, "/api/machine/devices").ok();
    let screens = devices
        .as_ref()
        .and_then(|b| b.get("devices").and_then(Value::as_array))
        .map(|l| l.iter().filter_map(own_device).filter(|d| !d.is_machine).collect())
        .unwrap_or_default();
    Screens { screens, not_answering: Vec::new() }
}

/// `refreshDevices`: the Fleet-wide list, else this machine's own. An older Fleet has neither; the list stays empty.
pub fn load(base: &str) -> Screens {
    list_fleet(base).unwrap_or_else(|_| list_own(base))
}

/// `removeFleetScreen`: the phone or browser loses its keys on every machine. A 404 is as good as done.
pub fn remove(base: &str, id: &str) -> Result<(), String> {
    let fallback = "Couldn't remove it.";
    let (status, text) =
        http::text_with("DELETE", base, &format!("/api/machine/screens/{}", http::escape(id)), None).map_err(|_| fallback.to_string())?;
    if (200..300).contains(&status) || status == 404 { Ok(()) } else { Err(read_error(&text, fallback.into())) }
}
