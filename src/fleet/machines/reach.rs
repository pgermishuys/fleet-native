//! How other devices reach a machine, in words and in requests: `lib/reach-api.ts` (`GET`/`PUT /api/machine/reach`),
//! `lib/reach-choices.ts` (the choices in "Let others reach <name>?"), `lib/machine-reach-line.ts` (the line under a machine
//! in the list), `lib/phone-reach.ts` (`onlyOnTailscale`) and `lib/phone-readiness.ts` (`reachLines`).
//!
//! The words and the rules are pure; the two requests block and run on a thread, on home's link (`State::load_reach`).
use crate::fleet::http;
use crate::fleet::machine::{Kind, kind_of};
use crate::fleet::machines::read_error;
use serde_json::{Value, json};

/// Who the desktop app lets reach it (`ReachMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReachMode {
    Off,
    TailscaleAndHome,
    Home,
    Own,
}

impl ReachMode {
    pub fn parse(word: &str) -> Option<ReachMode> {
        Some(match word {
            "off" => ReachMode::Off,
            "tailscale-and-home" => ReachMode::TailscaleAndHome,
            "home" => ReachMode::Home,
            "own" => ReachMode::Own,
            _ => return None,
        })
    }

    /// The word the server uses.
    pub fn word(self) -> &'static str {
        match self {
            ReachMode::Off => "off",
            ReachMode::TailscaleAndHome => "tailscale-and-home",
            ReachMode::Home => "home",
            ReachMode::Own => "own",
        }
    }
}

/// An address the server found or listens on (`ReachAddress`): only Tailscale, home network and host-name kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachAddress {
    pub url: String,
    pub kind: Kind,
}

/// What Fleet found about Tailscale on this machine (`TailscaleCheck`). `can_turn_on`: Tailscale is fine and only the listener
/// is missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailscaleCheck {
    /// "missing", "stopped", "signed-out" or "running".
    pub state: String,
    pub headline: String,
    pub detail: String,
    /// The reach listener already covers the Tailscale address.
    pub covered: bool,
    pub can_turn_on: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachState {
    /// False for a Fleet that isn't the desktop app's own: nothing can be changed.
    pub available: bool,
    pub mode: ReachMode,
    pub own_address: Option<String>,
    /// The loopback port a tunnel or proxy sends to for "my own address"; it always asks for a key.
    pub own_port: u16,
    /// Where other devices reach it now, best first; empty while off.
    pub listening: Vec<ReachAddress>,
    /// The addresses on this machine's networks, Tailscale first: what the choices would use.
    pub found: Vec<ReachAddress>,
    /// None for a Fleet that can't change reach, and for an older one.
    pub tailscale: Option<TailscaleCheck>,
}

fn addresses(v: &Value, key: &str) -> Vec<ReachAddress> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let url = a.get("url")?.as_str()?.to_string();
                    let kind = a.get("kind").and_then(Value::as_str).map_or_else(|| kind_of(&url), |k| kind_from(k, &url));
                    Some(ReachAddress { url, kind })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn kind_from(word: &str, url: &str) -> Kind {
    match word {
        "tailnet" => Kind::Tailnet,
        "lan" => Kind::Lan,
        "hostname" => Kind::Hostname,
        _ => kind_of(url),
    }
}

impl ReachState {
    pub fn from_json(v: &Value) -> ReachState {
        let text = |k: &str| v.get(k).and_then(Value::as_str);
        ReachState {
            available: v.get("available").and_then(Value::as_bool).unwrap_or(false),
            mode: text("mode").and_then(ReachMode::parse).unwrap_or(ReachMode::Off),
            own_address: text("ownAddress").map(str::to_string),
            own_port: v.get("ownPort").and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok()).unwrap_or(0),
            listening: addresses(v, "listening"),
            found: addresses(v, "found"),
            tailscale: v.get("tailscale").filter(|t| t.is_object()).map(|t| {
                let word = |k: &str| t.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let flag = |k: &str| t.get(k).and_then(Value::as_bool).unwrap_or(false);
                TailscaleCheck {
                    state: word("state"),
                    headline: word("headline"),
                    detail: word("detail"),
                    covered: flag("covered"),
                    can_turn_on: flag("canTurnOn"),
                }
            }),
        }
    }
}

/// `loadReach`: None for a Fleet too old to know (404) or one this app may not manage (403, a paired device).
pub fn load(base: &str) -> Result<Option<ReachState>, String> {
    let fallback = "Couldn't read who can reach this machine.";
    let (status, text) = http::text_with("GET", base, "/api/machine/reach", None).map_err(|_| fallback.to_string())?;
    match status {
        404 | 403 => Ok(None),
        200..=299 => Ok(serde_json::from_str::<Value>(&text).ok().as_ref().map(ReachState::from_json)),
        _ => Err(read_error(&text, fallback.into())),
    }
}

/// `saveReach`: the new state, or the server's own words for a refusal.
pub fn save(base: &str, mode: ReachMode, own_address: Option<&str>) -> Result<ReachState, String> {
    let fallback = "Couldn't change who can reach this machine.";
    let body = json!({ "mode": mode.word(), "ownAddress": own_address });
    let (status, text) = http::text_with("PUT", base, "/api/machine/reach", Some(&body)).map_err(|_| fallback.to_string())?;
    if !(200..300).contains(&status) {
        return Err(read_error(&text, fallback.into()));
    }
    serde_json::from_str::<Value>(&text).ok().as_ref().map(ReachState::from_json).ok_or_else(|| fallback.to_string())
}

// ---- the words ----------------------------------------------------------------------------------------------------------

/// `onlyOnTailscale`: every address the machine has is Tailscale's, so nothing else can ever reach it.
pub fn only_on_tailscale(kinds: &[Kind]) -> bool {
    !kinds.is_empty() && kinds.iter().all(|k| k.is_tailscale())
}

/// How the line reads (`ReachLine.tone`): `Out` is a machine that doesn't answer, `Limited` one a phone away from home can't
/// use, `Ok` the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Limited,
    Out,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachLine {
    pub text: String,
    pub tone: Tone,
}

pub struct ReachLineInput<'a> {
    /// Whether the machine answers this app now.
    pub answering: bool,
    /// When it stopped answering (ms since 1970), if this app saw it happen.
    pub offline_since: Option<i64>,
    /// The kinds of the addresses a phone could use to reach it.
    pub addresses: &'a [Kind],
    /// For the machine this app runs inside: who the desktop app lets reach it. None for the rest.
    pub reach_mode: Option<ReachMode>,
    /// The person's own address, when the mode is `Own`.
    pub own_address: Option<&'a str>,
}

/// What the machine's addresses reach: Tailscale, the home network, or something else (`viaWords`).
fn via_words(kinds: &[Kind]) -> Option<&'static str> {
    let tailscale = kinds.iter().any(|k| k.is_tailscale());
    let home = kinds.iter().any(|k| matches!(k, Kind::Lan | Kind::Hostname));
    let other = kinds.contains(&Kind::Other);
    match (tailscale, home, other) {
        (true, true, _) => Some("over Tailscale and the home network"),
        (true, false, _) => Some("over Tailscale"),
        (false, true, _) => Some("on the home network"),
        (false, false, true) => Some("by its own address"),
        _ => None,
    }
}

/// `formatTime` for a time in ms: "8:40 AM", in the zone `offset_secs` east of UTC.
pub fn clock_of(ms: i64, offset_secs: i64) -> String {
    let (.., hour, minute) = crate::conv::time::civil(ms, offset_secs);
    crate::conv::time::clock(hour, minute)
}

/// `reachLine`: the one line under a machine in Settings → Machines. `offset_secs` is the zone the clock is shown in.
pub fn reach_line(input: &ReachLineInput, offset_secs: i64) -> ReachLine {
    let line = |text: String, tone| ReachLine { text, tone };
    // Not answering is told first; "only on Tailscale" is for a machine that answers here but not from a phone away from home.
    if !input.answering {
        return match input.offline_since.filter(|&t| t != 0) {
            Some(at) => line(format!("Off since {}", clock_of(at, offset_secs)), Tone::Out),
            None => line("Not reachable".into(), Tone::Out),
        };
    }
    match input.reach_mode {
        Some(ReachMode::Off) => return line("Only this machine".into(), Tone::Limited),
        Some(ReachMode::Own) => {
            return line(
                match input.own_address.filter(|a| !a.is_empty()) {
                    Some(own) => format!("Reaches your phone at {own}"),
                    None => "Reaches your phone at your own address".into(),
                },
                Tone::Ok,
            );
        }
        Some(ReachMode::Home) => return line("Reaches your phone on the home network".into(), Tone::Ok),
        _ => {}
    }
    if only_on_tailscale(input.addresses) {
        return line("Only on Tailscale".into(), Tone::Limited);
    }
    match via_words(input.addresses) {
        Some(via) => line(format!("Reaches your phone {via}"), Tone::Ok),
        None => line("Only this machine".into(), Tone::Limited),
    }
}

// ---- the choices in "Let others reach <name>?" ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceAddress {
    pub url: String,
    pub kind: Kind,
    /// The host as shown: `studio.tail4f2a.ts.net`, `192.168.1.20`.
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachChoice {
    /// Never `Off`: that is the dialog's Turn off.
    pub mode: ReachMode,
    pub label: &'static str,
    pub recommended: bool,
    /// Nothing to listen on for this choice.
    pub disabled: bool,
    /// What it will listen on.
    pub addresses: Vec<ChoiceAddress>,
    tailnet: bool,
    lan: bool,
}

impl ReachChoice {
    /// What it means, in a sentence (`hint(machineName)`).
    pub fn hint(&self, machine: &str) -> String {
        match self.mode {
            ReachMode::TailscaleAndHome if self.tailnet => format!("Your phone reaches {machine} at home and away."),
            ReachMode::TailscaleAndHome => {
                format!("Tailscale wasn't found on this machine. Until it is, your phone reaches {machine} at home only.")
            }
            ReachMode::Home if self.lan => format!("At home only. Away from home, the phone shows {machine} as out of reach."),
            ReachMode::Home => "Fleet didn't find a home network address on this machine.".into(),
            _ => "A tunnel or a domain you run, such as https://fleet.example.com.".into(),
        }
    }
}

/// The host of a URL as Fleet's `new URL(url).hostname` gives it, else the text itself.
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let rest = rest.rsplit_once('@').map_or(rest, |(_, h)| h);
    let host = if let Some(v6) = rest.strip_prefix('[') {
        v6.find(']').map_or(rest, |end| &rest[..end + 2])
    } else {
        rest.split(':').next().unwrap_or(rest)
    };
    if url.contains("://") && !host.is_empty() { host.to_ascii_lowercase() } else { url.to_string() }
}

fn is_ip(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// `reachChoices`: the choices, worked out from the addresses Fleet found on this machine.
pub fn reach_choices(found: &[ReachAddress]) -> Vec<ReachChoice> {
    let shown: Vec<ChoiceAddress> = found
        .iter()
        .filter(|a| matches!(a.kind, Kind::Tailnet | Kind::Lan))
        .map(|a| ChoiceAddress { url: a.url.clone(), kind: a.kind, label: host_of(&a.url) })
        .collect();
    let lan: Vec<ChoiceAddress> = shown.iter().filter(|a| a.kind == Kind::Lan).cloned().collect();
    let tailnet: Vec<ChoiceAddress> = shown.iter().filter(|a| a.kind == Kind::Tailnet).cloned().collect();
    // Tailscale's name says more than its IP, so the IP only shows when there is no name.
    let named: Vec<ChoiceAddress> = tailnet.iter().filter(|a| !is_ip(&a.label)).cloned().collect();
    let tailnet_shown = if named.is_empty() { tailnet.clone() } else { named };
    let both = ReachChoice {
        mode: ReachMode::TailscaleAndHome,
        label: "Tailscale and home network",
        recommended: !tailnet.is_empty(),
        disabled: tailnet.len() + lan.len() == 0,
        addresses: tailnet_shown.into_iter().chain(lan.iter().cloned()).collect(),
        tailnet: !tailnet.is_empty(),
        lan: !lan.is_empty(),
    };
    let home = ReachChoice {
        mode: ReachMode::Home,
        label: "Home network only",
        recommended: false,
        disabled: lan.is_empty(),
        addresses: lan.clone(),
        tailnet: !tailnet.is_empty(),
        lan: !lan.is_empty(),
    };
    let own = ReachChoice {
        mode: ReachMode::Own,
        label: "My own address",
        recommended: false,
        disabled: false,
        addresses: Vec::new(),
        tailnet: false,
        lan: false,
    };
    vec![both, home, own]
}

// ---- "Your phone will reach <name>" --------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WayIn {
    Home,
    Tailscale,
    Own,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayLine {
    pub id: WayIn,
    /// What the phone needs for this one: "At home, on your Wi-Fi".
    pub title: &'static str,
    pub hosts: Vec<String>,
}

fn unique(hosts: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for host in hosts {
        if !out.contains(&host) {
            out.push(host);
        }
    }
    out
}

/// `reachLines`: one line for each way the phone can reach the machine, home network first, then Tailscale, then an address of
/// its own. A line appears only when there is an address behind it; loopback addresses mean nothing to a phone.
pub fn reach_lines(urls: &[&str]) -> Vec<WayLine> {
    let by_kind = |kinds: &[Kind]| -> Vec<String> { urls.iter().filter(|u| kinds.contains(&kind_of(u))).map(|u| host_of(u)).collect() };
    let lan = by_kind(&[Kind::Lan]);
    let home = if lan.is_empty() { by_kind(&[Kind::Hostname]) } else { lan };
    let named = by_kind(&[Kind::Tailnet]);
    let tailnet = if named.is_empty() { by_kind(&[Kind::TailnetIp]) } else { named };
    let own: Vec<String> = urls
        .iter()
        .filter(|u| kind_of(u) == Kind::Other)
        .map(|u| host_of(u))
        .filter(|h| {
            let h = h.to_ascii_lowercase();
            !(h.starts_with("localhost") || h.starts_with("127.") || h.starts_with("[::1]"))
        })
        .collect();

    let mut lines = Vec::new();
    if !home.is_empty() {
        lines.push(WayLine { id: WayIn::Home, title: "At home, on your Wi-Fi", hosts: unique(home) });
    }
    if !tailnet.is_empty() {
        lines.push(WayLine { id: WayIn::Tailscale, title: "Away from home, with Tailscale on the phone", hosts: unique(tailnet) });
    }
    if !own.is_empty() {
        lines.push(WayLine { id: WayIn::Own, title: "From anywhere, at your own address", hosts: unique(own) });
    }
    lines
}
