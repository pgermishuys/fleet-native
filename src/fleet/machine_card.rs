//! What a card on Settings → Machines says, as plain data (`MachinesSection.vue`: `stateLine`, `capabilityLine`,
//! `readyHarnesses`, `harnessVersions`, `showsAgentWork`, `location`; `stores/machines.ts`: `agentWorkLevelName`). The view only
//! draws it, so every word is testable without a window.
use super::machine_diagnosis::Diagnosis;
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
                }
            })
            .collect()
    }
}

impl State {
    pub fn machine_cards(&self) -> Vec<Card> {
        self.machines.cards()
    }
}
