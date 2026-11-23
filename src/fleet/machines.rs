//! The machines home lists and what is known of each: ports weave-fleet's `stores/machines.ts` (the list, `readMachine`,
//! `applyRead`, `markUnreachable`, `patchSession`, `upsertSession`, `isAnswering`, `forgetMachine`, `keyOfMachine`).
//! Pure: no Rune and no I/O. The calls are made by `machine_poll`, the file is `machine_cache`.
//!
//! The list is never kept by the app: home keeps it (`GET /api/machines`) and this is a copy of the last answer.
//! Home is `home` here; another machine's key is its id.
pub mod add;
pub mod data;
pub mod found;
pub mod reach;
pub mod screens;

use super::auth;
use super::groups::{ProjectInfo, parse_projects};
use super::machine::Machine;
use super::machine_diagnosis::Diagnosis;
use super::sessions::parse_sessions;
use super::settings::HOME_MACHINE_KEY;
use crate::mock::Session;
use serde_json::Value;
use std::collections::HashMap;

/// A harness as a machine last checked it (`MachineHarness`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Harness {
    pub kind: String,
    pub name: String,
    pub available: bool,
    pub enabled: bool,
    pub version: Option<String>,
}

/// What `GET /api/machine` says a machine can run, and how busy it is (`MachineCapabilities`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// None until the machine has checked its harnesses once.
    pub harnesses: Option<Vec<Harness>>,
    pub working: u32,
    pub needs_you: u32,
    /// Missing from a Fleet too old to say.
    pub permission_level: Option<String>,
}

impl Capabilities {
    fn from_json(v: &Value) -> Option<Capabilities> {
        let count = |k: &str| v.pointer(&format!("/sessions/{k}")).and_then(Value::as_u64).unwrap_or(0).min(u32::MAX as u64) as u32;
        v.get("sessions")?.as_object()?;
        let harnesses = v.get("harnesses").and_then(Value::as_array).map(|list| {
            list.iter()
                .filter_map(|h| {
                    let text = |k: &str| h.get(k).and_then(Value::as_str);
                    Some(Harness {
                        kind: text("type")?.to_string(),
                        name: text("name").unwrap_or_default().to_string(),
                        available: h.get("available").and_then(Value::as_bool).unwrap_or(false),
                        enabled: h.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                        version: text("version").map(str::to_string),
                    })
                })
                .collect()
        });
        Some(Capabilities {
            harnesses,
            working: count("working"),
            needs_you: count("needsYou"),
            permission_level: v.get("permissionLevel").and_then(Value::as_str).map(str::to_string),
        })
    }
}

/// What `GET /api/machine` says a machine is (`MachineInfo`). The `update` block feeds Settings → Updates & about
/// (`fleet::updates`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub id: String,
    pub name: String,
    pub host_name: String,
    pub os: String,
    pub version: String,
    pub api_version: i64,
    pub auth_mode: String,
    pub remote_reachable: bool,
    pub requires_token: bool,
    /// Missing on a Fleet older than the field, which does serve one.
    pub web_app: Option<bool>,
    pub public_url: Option<String>,
    /// Missing (None) for a Fleet too old to say what it can run.
    pub capabilities: Option<Capabilities>,
    /// Where the machine is with Fleet updates; None from a Fleet older than the field.
    pub update: Option<super::updates::MachineUpdate>,
}

impl Info {
    pub fn from_json(v: &Value) -> Option<Info> {
        v.as_object()?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        Some(Info {
            id: text("id"),
            name: text("name"),
            host_name: text("hostName"),
            os: text("os"),
            version: text("version"),
            api_version: v.get("apiVersion").and_then(Value::as_i64).unwrap_or(0),
            auth_mode: text("authMode"),
            remote_reachable: flag("remoteReachable"),
            requires_token: flag("requiresToken"),
            web_app: v.get("webApp").and_then(Value::as_bool),
            public_url: v.get("publicUrl").and_then(Value::as_str).map(str::to_string),
            capabilities: v.get("capabilities").and_then(Capabilities::from_json),
            update: v.get("update").and_then(super::updates::MachineUpdate::from_json),
        })
    }
}

/// Whether agents here can send a machine work (`MachineAgentWork`): ask the user, always, or never.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentWork {
    Ask,
    Always,
    Never,
}

/// Where the list came from (`MachineListSource`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// Not asked yet, or not answered.
    #[default]
    Pending,
    /// Home handed over every machine's token: the app signed in as its owner.
    Owner,
    /// Some token came back empty: the app signed in with a phone's token, which can't reach other machines.
    Device,
    /// A 404, an error or a body that isn't a list: this Fleet can't list machines. No machines, no error shown.
    Cannot,
}

/// What home's `GET /api/machines` said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub source: Source,
    pub machines: Vec<Machine>,
    pub agent_work: HashMap<String, AgentWork>,
}

/// `syncFromServer`'s reading of the body. A machine the app holds no token for is left out (Fleet fills it from a
/// phone's grants, which this app has none of).
pub fn parse_machine_list(status: u16, body: &str) -> Listed {
    let cannot = Listed { source: Source::Cannot, machines: Vec::new(), agent_work: HashMap::new() };
    if !(200..300).contains(&status) {
        return cannot;
    }
    let Some(list) = serde_json::from_str::<Value>(body).ok().and_then(|b| b.get("machines").and_then(Value::as_array).cloned()) else {
        return cannot;
    };
    let owner = list.iter().all(|m| m.get("token").is_some_and(|t| !t.is_null()));
    let machines = list.iter().filter_map(Machine::from_json).filter(|m| m.token.is_some()).collect();
    let agent_work = if owner {
        list.iter()
            .filter_map(|m| {
                let id = m.get("id")?.as_str()?.to_string();
                let work = match m.get("agentWork")?.as_str()? {
                    "ask" => AgentWork::Ask,
                    "always" => AgentWork::Always,
                    "never" => AgentWork::Never,
                    _ => return None,
                };
                Some((id, work))
            })
            .collect()
    } else {
        HashMap::new()
    };
    Listed { source: if owner { Source::Owner } else { Source::Device }, machines, agent_work }
}

/// One row of the machine list: home or a listed machine, with what was last heard from it (`MachineEntry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `home` for home, otherwise the machine's id.
    pub key: String,
    pub name: String,
    pub os: Option<String>,
    /// Where the app reaches it; the Fleet the app is connected to for home.
    pub is_home: bool,
    pub machine: Option<Machine>,
    pub info: Option<Info>,
}

impl Entry {
    pub fn capabilities(&self) -> Option<&Capabilities> {
        self.info.as_ref().and_then(|i| i.capabilities.as_ref())
    }
}

/// The sessions of a machine that isn't home: the last list it returned (`MachineSessions`).
#[derive(Default)]
pub struct MachineSessions {
    pub sessions: Vec<Session>,
    /// Its projects, for their order and names; empty until it first answers.
    pub projects: Vec<ProjectInfo>,
    pub error: Option<String>,
    /// When the list last came back, in ms; None before it ever has.
    pub loaded_at: Option<i64>,
    pub loading: bool,
}

/// One good read of a machine (`MachineRead`): its sessions, and its projects and identity when those answered.
pub struct Read {
    pub sessions: Vec<Session>,
    pub projects: Option<Vec<ProjectInfo>>,
    pub identity: Option<Info>,
}

/// A read to make: which machine, what to call it in an error, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadJob {
    pub key: String,
    pub name: String,
    pub base: String,
    /// For picking another address when the one in use can't be reached.
    pub machine: Machine,
}

/// The words of a failed answer (`readError`): the body's `error`, then `message`, else `fallback`.
pub fn read_error(body: &str, fallback: String) -> String {
    let said = serde_json::from_str::<Value>(body).ok().and_then(|b| {
        ["error", "message"].iter().find_map(|k| b.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string))
    });
    said.unwrap_or(fallback)
}

/// A machine's answers to its three calls, by status and body (`Err` where nothing answered).
pub type Answer = Result<(u16, String), String>;

/// `readMachine`, from the three answers: sessions decide the read; projects and identity may fail without failing it.
pub fn read_machine(name: &str, sessions: Answer, projects: Answer, identity: Answer, now_ms: i64) -> Result<Read, String> {
    let (status, body) = sessions.map_err(|_| format!("Can't reach {name}."))?;
    if status == 401 {
        return Err(format!("{name} didn't accept the token."));
    }
    if !(200..300).contains(&status) {
        return Err(read_error(&body, format!("{name} answered {status}.")));
    }
    let rows: Value = serde_json::from_str(&body).map_err(|_| format!("{name} answered with something that isn't a session list."))?;
    let json = |a: Answer| a.ok().filter(|(s, _)| (200..300).contains(s)).and_then(|(_, b)| serde_json::from_str::<Value>(&b).ok());
    Ok(Read {
        sessions: parse_sessions(&rows, now_ms),
        projects: json(projects).filter(Value::is_array).map(|b| parse_projects(&b)),
        identity: json(identity).as_ref().and_then(Info::from_json),
    })
}

/// What the app knows of the other machines.
#[derive(Default)]
pub struct Machines {
    pub source: Source,
    /// What home said about itself; None until it answers (the heading says "This machine").
    pub home: Option<Info>,
    /// The others, in home's order.
    pub list: Vec<Machine>,
    pub agent_work: HashMap<String, AgentWork>,
    /// What each other machine last said about itself, by id.
    pub identities: HashMap<String, Info>,
    pub others: HashMap<String, MachineSessions>,
    /// Why each machine that isn't answering isn't, by id, from home's `check`; kept only while its error stands.
    pub diagnoses: HashMap<String, Diagnosis>,
    /// Which machine each session is on, by session id: ids only, never dropped (`rememberSessionMachines`).
    pub session_machines: HashMap<String, String>,
    /// The state of each other machine's hub, by id; none until a session there is opened.
    pub hubs: HashMap<String, super::link::Connection>,
    /// The machine of the session open now, or of the last one that was (`currentOpened`); kept for the run.
    current_opened: Option<String>,
}

impl Machines {
    /// The machine you're on: where the open (or last open) session is, else home. A key that isn't listed counts as home
    /// (`currentKey`).
    pub fn current_key(&self) -> &str {
        self.current_opened.as_deref().filter(|k| self.machine(k).is_some()).unwrap_or(HOME_MACHINE_KEY)
    }

    pub fn set_current(&mut self, key: &str) {
        self.current_opened = Some(key.to_string());
    }

    /// Home's answer to `GET /api/machines`. Holds each machine's token under its address and lets go of the tokens of
    /// machines that are no longer listed. The machines' own state (the cached lists too) is kept for the ones still there.
    pub fn apply_list(&mut self, listed: Listed) {
        for gone in self.list.iter().filter(|m| !listed.machines.iter().any(|n| n.id == m.id)) {
            auth::set_for(gone.base(), None);
        }
        for machine in &listed.machines {
            if let Some(old) = self.list.iter().find(|m| m.id == machine.id)
                && old.base() != machine.base()
            {
                auth::set_for(old.base(), None);
            }
            machine.hold_token();
        }
        let ids: Vec<&str> = listed.machines.iter().map(|m| m.id.as_str()).collect();
        self.identities.retain(|id, _| ids.contains(&id.as_str()));
        self.others.retain(|id, _| ids.contains(&id.as_str()));
        self.diagnoses.retain(|id, _| ids.contains(&id.as_str()));
        self.agent_work = listed.agent_work;
        self.list = listed.machines;
        self.source = listed.source;
    }

    /// The cache file was read: each machine's last list shows until its first read comes back. A list read already is
    /// newer than the file; a machine home has already said it doesn't list is not shown from the file.
    pub fn adopt_cache(&mut self, cached: super::machine_cache::Cached) {
        for (key, state) in cached.others {
            let listed = self.source == Source::Pending || self.machine(&key).is_some();
            if listed && self.others.get(&key).is_none_or(|n| n.loaded_at.is_none()) {
                let loading = self.others.get(&key).is_some_and(|n| n.loading);
                self.others.insert(key, MachineSessions { loading, ..state });
            }
        }
        for (id, key) in cached.session_machines {
            self.session_machines.entry(id).or_insert(key);
        }
    }

    /// Whether this Fleet shows any machine besides home: everything machine-shaped hides until it does (`hasMachines`).
    pub fn has_machines(&self) -> bool {
        !self.list.is_empty()
    }

    /// Home first, then the others in the order home lists them (`entries`).
    pub fn entries(&self) -> Vec<Entry> {
        let home = Entry {
            key: HOME_MACHINE_KEY.to_string(),
            name: self.home.as_ref().map_or_else(|| "This machine".to_string(), |i| i.name.clone()),
            os: self.home.as_ref().map(|i| i.os.clone()),
            is_home: true,
            machine: None,
            info: self.home.clone(),
        };
        let others = self.list.iter().map(|m| Entry {
            key: m.id.clone(),
            name: m.name.clone(),
            os: m.os.clone(),
            is_home: false,
            machine: Some(m.clone()),
            info: self.identities.get(&m.id).cloned(),
        });
        std::iter::once(home).chain(others).collect()
    }

    /// The key of the machine with id `machine_id`: home's own id gives `home`, a listed one its id, anything else none
    /// (`keyOfMachine`).
    pub fn key_of_machine(&self, machine_id: &str) -> Option<String> {
        if self.home.as_ref().is_some_and(|h| h.id == machine_id) {
            return Some(HOME_MACHINE_KEY.to_string());
        }
        self.list.iter().any(|m| m.id == machine_id).then(|| machine_id.to_string())
    }

    pub fn machine(&self, key: &str) -> Option<&Machine> {
        self.list.iter().find(|m| m.id == key)
    }

    /// Whether `key`'s machine answers: home always, another while its last read had no error (`isAnswering`).
    pub fn is_answering(&self, key: &str) -> bool {
        key == HOME_MACHINE_KEY || self.others.get(key).is_none_or(|s| s.error.is_none())
    }

    /// The reads to start now: every listed machine that isn't being read already (one at a time per machine). Marks each
    /// as loading (`markLoading`).
    pub fn begin_reads(&mut self) -> Vec<ReadJob> {
        let mut jobs = Vec::new();
        for machine in &self.list {
            let state = self.others.entry(machine.id.clone()).or_default();
            if !state.loading {
                state.loading = true;
                jobs.push(ReadJob {
                    key: machine.id.clone(),
                    name: machine.name.clone(),
                    base: machine.base().to_string(),
                    machine: machine.clone(),
                });
            }
        }
        jobs
    }

    /// A read of one machine now, after a change made on it (`refreshMachine`). `None` for a machine that isn't listed.
    pub fn begin_read(&mut self, key: &str) -> Option<ReadJob> {
        let machine = self.list.iter().find(|m| m.id == key)?;
        let job =
            ReadJob { key: machine.id.clone(), name: machine.name.clone(), base: machine.base().to_string(), machine: machine.clone() };
        self.others.entry(key.to_string()).or_default().loading = true;
        Some(job)
    }

    /// A good read (`applyRead`). Returns whether the machine was not answering before, which is when its open
    /// sessions should connect at once rather than wait out a backoff.
    pub fn apply_read(&mut self, key: &str, read: Read, now_ms: i64) -> bool {
        if self.machine(key).is_none() {
            return false;
        }
        let state = self.others.entry(key.to_string()).or_default();
        let recovered = state.error.take().is_some();
        self.diagnoses.remove(key);
        if let Some(identity) = read.identity {
            self.identities.insert(key.to_string(), identity);
        }
        let ids: Vec<String> = read.sessions.iter().map(|s| s.id.clone()).collect();
        state.projects = read.projects.unwrap_or_else(|| std::mem::take(&mut state.projects));
        state.sessions = read.sessions;
        state.loaded_at = Some(now_ms);
        state.loading = false;
        self.remember_sessions(key, &ids);
        recovered
    }

    /// A failed read keeps the last list and says why (`markUnreachable`).
    pub fn mark_unreachable(&mut self, key: &str, error: String) {
        if self.machine(key).is_none() {
            return;
        }
        let state = self.others.entry(key.to_string()).or_default();
        state.error = Some(error);
        state.loading = false;
    }

    /// Why `key`'s machine isn't answering, for the heading's tooltip: this app's own check when it has run, else home's last
    /// look at it. None while it answers (`why` in `MachineSessionsGroup.vue`).
    pub fn why(&self, key: &str) -> Option<&Diagnosis> {
        if self.is_answering(key) {
            return None;
        }
        self.diagnoses.get(key).or_else(|| self.machine(key)?.diagnosis.as_ref())
    }

    /// Whether home should be asked why `key`'s machine isn't answering: it isn't, nothing says why yet, and the SSH tunnel
    /// isn't known to be down (the card says that itself). Only home can look, and only an owner's app reaches it.
    pub fn wants_diagnosis(&self, key: &str) -> bool {
        self.source == Source::Owner
            && !self.is_answering(key)
            && self.machine(key).is_some_and(|m| m.tunnel_up != Some(false))
            && !self.diagnoses.contains_key(key)
    }

    /// Home's answer to `check`, kept only while the machine still has an error (`diagnose`).
    pub fn set_diagnosis(&mut self, key: &str, diagnosis: Option<Diagnosis>) {
        if self.is_answering(key) {
            return;
        }
        match diagnosis {
            Some(d) => self.diagnoses.insert(key.to_string(), d),
            None => self.diagnoses.remove(key),
        };
    }

    /// Records that the sessions are on `key`'s machine, so a link to one finds it (`rememberSessions`).
    pub fn remember_sessions(&mut self, key: &str, ids: &[String]) {
        for id in ids {
            self.session_machines.insert(id.clone(), key.to_string());
        }
    }

    /// Changes one listed session of another machine at once, before its next list says so (`patchSession`). A session it
    /// doesn't list is left alone.
    pub fn patch_session(&mut self, key: &str, id: &str, change: impl FnOnce(&mut Session)) {
        if let Some(s) = self.others.get_mut(key).and_then(|m| m.sessions.iter_mut().find(|s| s.id == id)) {
            change(s);
        }
    }

    /// Shows a session just started on another machine at the top of that machine's list, before its next list has it
    /// (`upsertSession`). A session the list already has is replaced in place.
    pub fn upsert_session(&mut self, key: &str, row: Session) {
        let id = row.id.clone();
        let state = self.others.entry(key.to_string()).or_default();
        match state.sessions.iter_mut().find(|s| s.id == id) {
            Some(known) => *known = row,
            None => state.sessions.insert(0, row),
        }
        self.remember_sessions(key, &[id]);
    }

    /// Forgets a machine home no longer lists: its group, its cached rows and its token go at once. The sessions it was
    /// asked for stay on the machine; the session to machine map keeps its ids. Whether it was listed.
    pub fn forget(&mut self, id: &str) -> bool {
        let Some(at) = self.list.iter().position(|m| m.id == id) else { return false };
        let gone = self.list.remove(at);
        auth::set_for(gone.base(), None);
        self.others.remove(id);
        self.diagnoses.remove(id);
        self.identities.remove(id);
        self.agent_work.remove(id);
        true
    }

    /// The scheme of every listed address once, for the log: whether the machines are reached over https
    /// (decision 8). Only the scheme, never a host or a token.
    pub fn address_schemes(&self) -> Vec<String> {
        let mut schemes: Vec<String> = self
            .list
            .iter()
            .flat_map(|m| std::iter::once(m.base().to_string()).chain(m.addresses.iter().map(|a| a.url.clone())))
            .map(|url| url.split_once("://").map_or("http", |(scheme, _)| scheme).to_ascii_lowercase())
            .collect();
        schemes.sort();
        schemes.dedup();
        schemes
    }
}
