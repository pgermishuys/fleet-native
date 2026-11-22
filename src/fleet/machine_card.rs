//! What a card on Settings → Machines says, as plain data (`MachinesSection.vue`: `stateLine`, `capabilityLine`,
//! `readyHarnesses`, `harnessVersions`, `showsAgentWork`, `location`; `stores/machines.ts`: `agentWorkLevelName`). The view only
//! draws it, so every word is testable without a window.
use super::machine::{Address, AddressResult, Kind, kind_of};
use super::machine_diagnosis::Diagnosis;
use super::machines::reach::{ReachLine, ReachLineInput};
use super::machines::{AgentWork, Capabilities, Entry, Machines, Source};
use crate::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Bad,
    Muted,
}

/// One machine's card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub key: String,
    pub name: String,
    pub os: Option<String>,
    pub is_home: bool,
    /// The live machine, tagged "Live": always home here (decision 1 of docs/parity/17-machines.md).
    pub live: bool,
    /// The address in use, "this machine" for home, or "Over SSH as user@host".
    pub location: String,
    pub state: String,
    pub tone: Tone,
    /// "Ready: OpenCode, Claude Code · 1 working · 2 need you"; none for a Fleet too old to say or one not answering.
    pub capabilities: Option<String>,
    /// The tooltip of that line: each ready harness with its version.
    pub versions: Option<String>,
    /// The "Agents here can send it work" setting and the level the work runs under; only on another machine's card.
    pub agent_work: Option<(AgentWork, String)>,
    /// Why the machine isn't answering, for the Details panel.
    pub diagnosis: Option<Diagnosis>,
    /// "Connected on your network" and, only when Fleet fell back off Tailscale, why (`routeLine`); only while it answers.
    pub route: Option<Route>,
    /// Where the machine can be reached, in the order Fleet tries them (`addressesOf`); none for home or an SSH machine.
    pub addresses: Vec<AddressRow>,
    /// How a phone reaches it (`reachOf`); set by `State::machine_cards`, which knows home's access and reach.
    pub reach: Option<ReachLine>,
}

/// `routeLine`: how this app is connected, from the kind of the address in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub text: String,
    pub note: Option<String>,
}

/// One row of the address list (`MachineAddressList.vue`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddressRow {
    pub url: String,
    /// `KIND_LABELS`.
    pub kind: &'static str,
    /// The machine reported it itself; false for the address it was added with.
    pub learned: bool,
    /// "In use", "Answered", "No answer", "Another device" or "Not tried" (`statusOf`).
    pub status: &'static str,
    pub in_use: bool,
}

impl AddressRow {
    fn of(a: &Address, in_use: &str) -> AddressRow {
        let used = a.url == in_use;
        let status = if used {
            "In use"
        } else {
            match a.last_result {
                Some(AddressResult::Ok) => "Answered",
                Some(AddressResult::NoAnswer) => "No answer",
                Some(AddressResult::OtherMachine) => "Another device",
                None => "Not tried",
            }
        };
        AddressRow { url: a.url.clone(), kind: a.kind.label(), learned: a.learned, status, in_use: used }
    }
}

/// `routeLine`: `base` is the address in use, `addresses` the machine's, first to try first.
fn route_line(base: &str, addresses: &[Address]) -> Option<Route> {
    let first = addresses.first()?;
    let kind = kind_of(base);
    let text = match kind {
        k if k.is_tailscale() => "Connected over Tailscale".to_string(),
        Kind::Lan => "Connected on your network".to_string(),
        Kind::Hostname => "Connected by host name".to_string(),
        _ => format!("Connected at {base}"),
    };
    // Only when Fleet fell back off Tailscale: on its IP after its name failed, Tailscale itself works.
    let note = (first.url != base && first.kind.is_tailscale() && !kind.is_tailscale()).then(|| "Tailscale isn't answering".to_string());
    Some(Route { text, note })
}

/// The names of the permission levels (`PERMISSION_LEVEL_NAMES`); "unknown" for a Fleet too old to say.
pub fn level_name(level: Option<&str>) -> &'static str {
    match level {
        Some("ask") => "Ask before changes",
        Some("edits") => "Allow edits",
        Some("all") => "Allow everything",
        _ => "unknown",
    }
}

/// The machine's ready harnesses: available and enabled (`readyHarnesses`).
fn ready(c: &Capabilities) -> impl Iterator<Item = &super::machines::Harness> {
    c.harnesses.iter().flatten().filter(|h| h.available && h.enabled)
}

/// `capabilityLine`: what the machine can run and how busy it is.
pub fn capability_line(c: &Capabilities) -> String {
    let mut parts = Vec::new();
    if c.harnesses.is_some() {
        let names: Vec<&str> = ready(c).map(|h| h.name.as_str()).collect();
        parts.push(if names.is_empty() { "No harness ready".to_string() } else { format!("Ready: {}", names.join(", ")) });
    }
    parts.push(format!("{} working", c.working));
    if c.needs_you > 0 {
        parts.push(format!("{} {} you", c.needs_you, if c.needs_you == 1 { "needs" } else { "need" }));
    }
    parts.join(" · ")
}

/// `harnessVersions`: "OpenCode 1.18.32, Claude Code 2.1.0"; none when nothing is ready.
pub fn harness_versions(c: &Capabilities) -> Option<String> {
    let all: Vec<String> = ready(c).map(|h| h.version.as_ref().map_or_else(|| h.name.clone(), |v| format!("{} {v}", h.name))).collect();
    (!all.is_empty()).then(|| all.join(", "))
}

impl Machines {
    /// Why a card says its machine isn't answering: its SSH tunnel being down, else what home or this app found.
    fn card_diagnosis(&self, entry: &Entry) -> Option<Diagnosis> {
        let m = entry.machine.as_ref()?;
        if m.tunnel_up == Some(false) {
            return Some(Diagnosis::tunnel_down(&m.name, m.ssh_destination.as_deref()));
        }
        self.why(&entry.key).cloned()
    }

    /// `stateLine`: the words and the tone under a machine's name. Home is the live machine, which answers.
    fn state_line(&self, entry: &Entry, why: Option<&Diagnosis>) -> (String, Tone) {
        if let Some(d) = why.filter(|_| entry.machine.as_ref().is_some_and(|m| m.tunnel_up == Some(false))) {
            return (d.headline.clone(), Tone::Bad);
        }
        if entry.is_home {
            return ("live".into(), Tone::Ok);
        }
        let state = self.others.get(&entry.key);
        if let Some(error) = state.and_then(|s| s.error.as_ref()) {
            return (why.map_or_else(|| error.clone(), |d| d.headline.clone()), Tone::Bad);
        }
        if let Some(state) = state.filter(|s| s.loaded_at.is_some()) {
            let n = state.sessions.len();
            return (format!("reachable · {n} {}", if n == 1 { "session" } else { "sessions" }), Tone::Ok);
        }
        ("checking…".into(), Tone::Muted)
    }

    /// One card per machine, home first then the others in home's order.
    pub fn cards(&self) -> Vec<Card> {
        self.entries()
            .into_iter()
            .map(|entry| {
                let why = self.card_diagnosis(&entry);
                let (state, tone) = self.state_line(&entry, why.as_ref());
                let answering = entry.is_home || self.is_answering(&entry.key);
                let caps = entry.capabilities().filter(|_| answering).cloned();
                // Home and an SSH machine have no addresses to order.
                let by_url = entry.machine.as_ref().filter(|m| !m.addresses.is_empty() && m.ssh_destination.is_none());
                let addresses: Vec<AddressRow> =
                    by_url.map(|m| m.addresses.iter().map(|a| AddressRow::of(a, m.base())).collect()).unwrap_or_default();
                let route = by_url.filter(|_| tone == Tone::Ok).and_then(|m| route_line(m.base(), &m.addresses));
                let location = match entry.machine.as_ref().and_then(|m| m.ssh_destination.as_ref()) {
                    Some(dest) => format!("Over SSH as {dest}"),
                    None => entry.machine.as_ref().map_or_else(|| "this machine".to_string(), |m| m.base().to_string()),
                };
                // Only another machine in home's own list, and only to the owner who sees that list.
                let agent_work = (!entry.is_home && self.source == Source::Owner)
                    .then(|| self.agent_work.get(&entry.key).copied())
                    .flatten()
                    .map(|w| (w, level_name(entry.capabilities().and_then(|c| c.permission_level.as_deref())).to_string()));
                Card {
                    live: entry.is_home,
                    key: entry.key,
                    name: entry.name,
                    os: entry.os,
                    is_home: entry.is_home,
                    location,
                    state,
                    tone,
                    capabilities: caps.as_ref().map(capability_line),
                    versions: caps.as_ref().and_then(harness_versions),
                    agent_work,
                    diagnosis: why,
                    route,
                    addresses,
                    reach: None,
                }
            })
            .collect()
    }
}

impl State {
    /// `reachOf` (`use-machine-lines.ts`): home's addresses come from its access, another machine's from its own list.
    fn reach_of(&self, card: &Card, offset: impl Fn(i64) -> i64) -> ReachLine {
        use super::machine_actions::AccessState;
        let kinds: Vec<Kind> = if card.is_home {
            match &self.machine_page.access {
                AccessState::Loaded(a) => a.addresses.iter().map(|x| kind_of(&x.url)).collect(),
                _ => Vec::new(),
            }
        } else {
            self.machines.machine(&card.key).map(|m| m.addresses.iter().map(|a| a.kind).collect()).unwrap_or_default()
        };
        let state = self.machine_data.reach.as_ref().filter(|r| card.is_home && r.available);
        let loaded_at = self.machines.others.get(&card.key).and_then(|o| o.loaded_at);
        let input = ReachLineInput {
            answering: self.machines.is_answering(&card.key),
            offline_since: loaded_at,
            addresses: &kinds,
            reach_mode: state.map(|r| r.mode),
            own_address: self.machine_data.reach.as_ref().and_then(|r| r.own_address.as_deref()),
        };
        super::machines::reach::reach_line(&input, offset(loaded_at.unwrap_or(0)))
    }

    /// The cards, with an address list drawn in the order just asked for while that order is being saved.
    pub fn machine_cards(&self) -> Vec<Card> {
        let mut cards = self.machines.cards();
        let offset = |ms: i64| crate::conv::time::local_offset_at(ms);
        for card in &mut cards {
            card.reach = Some(self.reach_of(card, offset));
            if let Some(order) = self.machine_page.addresses.order.get(&card.key) {
                card.addresses.sort_by_key(|a| order.iter().position(|u| *u == a.url).unwrap_or(usize::MAX));
            }
        }
        cards
    }
}
