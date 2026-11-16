//! Which machine a call goes to (component 17, piece D; `stores/machines.ts` `sessionTarget`, `targetOfKey`).
//!
//! Home's link is `State::link`, started with the app. Another machine's link is started the first time a session of that
//! machine needs one and kept, so a conversation open on it keeps its hub. Every call about a session goes through
//! `State::link_for(session)`, and its address with it, so a request can never carry one machine's token to another: the token
//! is picked by the address the request is about to go to (`auth::middleware`).
use super::link::{Connection, Link};
use super::machine::Target;
use super::settings::HOME_MACHINE_KEY;
use crate::State;
use std::collections::HashMap;
use std::sync::Mutex;

/// The links to the machines other than home, by machine id.
#[derive(Default)]
pub struct Links(Mutex<HashMap<String, Link>>);

impl Links {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Link>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.lock().keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn all(&self) -> Vec<Link> {
        self.lock().values().cloned().collect()
    }
}

impl State {
    /// The machine listed under `key`; home for a key that isn't listed (a machine that's gone: its sessions are asked of
    /// home again). `None` while the app has no Fleet to ask.
    pub fn target_of_key(&self, key: &str) -> Option<Target> {
        let home = self.link.as_ref()?;
        match self.machines.machine(key) {
            Some(machine) if key != HOME_MACHINE_KEY => Some(machine.target()),
            _ => Some(Target::home(&home.base)),
        }
    }

    /// The machine a session is on: where it was opened (`elsewhere`), else where a list said it lives, else home.
    pub fn target_of(&self, session: &str) -> Option<Target> {
        self.target_of_key(self.machine_key_of(session))
    }

    /// The key of the machine a session is on, as far as the app knows; home's for any it doesn't know. Needs no link.
    pub fn machine_key_of(&self, session: &str) -> &str {
        self.store
            .machine_of(session)
            .or_else(|| self.machines.session_machines.get(session).map(String::as_str))
            .filter(|k| *k == HOME_MACHINE_KEY || self.machines.machine(k).is_some())
            .unwrap_or(HOME_MACHINE_KEY)
    }

    /// The link a call about `session` goes through, started if its machine has none yet. `None` without a Fleet.
    pub fn link_for(&self, session: &str) -> Option<Link> {
        let target = self.target_of(session)?;
        self.link_of(&target)
    }

    fn link_of(&self, target: &Target) -> Option<Link> {
        let home = self.link.as_ref()?;
        if target.is_home() {
            return Some(home.clone());
        }
        let mut links = self.links.lock();
        // A machine that moved to another address gets a new link; the old one's hub ends when it is dropped.
        if links.get(&target.key).is_none_or(|l| l.base != target.base) {
            links.insert(target.key.clone(), Link::start_machine(target.key.clone(), target.base.clone(), home.post()));
        }
        links.get(&target.key).cloned()
    }

    /// Lets go of the links of machines home no longer lists.
    pub fn prune_links(&mut self) {
        let ids: Vec<String> = self.machines.list.iter().map(|m| m.id.clone()).collect();
        self.links.lock().retain(|key, _| ids.contains(key));
        self.machines.hubs.retain(|key, _| ids.contains(key));
    }

    /// A hub's state: home's is the app's connection, another machine's is kept by its id (and not for one that's gone).
    pub fn set_connection(&mut self, key: &str, connection: Connection) {
        if key == HOME_MACHINE_KEY {
            self.connection = connection;
        } else if self.machines.machine(key).is_some() {
            self.machines.hubs.insert(key.to_string(), connection);
        }
    }

    /// The link for the open session, or home's when none is open (a call about no session in particular).
    pub fn open_link(&self) -> Option<Link> {
        match self.open_id() {
            Some(id) => self.link_for(&id),
            None => self.link.clone(),
        }
    }

    /// The window gained or lost the keyboard, or went off screen: every hub says so to its own Fleet.
    pub fn each_link(&self, f: impl Fn(&Link)) {
        if let Some(home) = &self.link {
            f(home);
        }
        for link in self.links.all() {
            f(&link);
        }
    }

    /// Makes `id`'s link the one that holds the open session, and tells the others to let go of theirs.
    pub fn open_on_link(&self, id: &str) {
        let Some(target) = self.target_of(id) else { return };
        if let Some(link) = self.link_of(&target) {
            for other in self.links.all().iter().chain(self.link.as_ref()) {
                if other.key != link.key {
                    other.close_open();
                }
            }
            link.open(id);
        }
    }
}

impl State {
    /// A good read of `key`'s sessions: the rows of its sessions kept as opened here take the new status and title, and keep
    /// their conversation (`sessionById` reads `elsewhere` first, so a stale row would outlive the list).
    pub fn refresh_elsewhere(&mut self, key: &str) {
        let Some(listed) = self.machines.others.get(key) else { return };
        let kept: Vec<&crate::mock::Session> = listed.sessions.iter().filter(|s| self.store.machine_of(&s.id) == Some(key)).collect();
        for fresh in kept {
            self.store.patch(&mut [], &fresh.id, |old| {
                let messages = std::mem::take(&mut old.messages);
                *old = fresh.row_copy();
                old.messages = messages;
            });
        }
    }

    /// Whether the open session is `id` on `machine`: the open mark sits on that machine's row only (`openSessionMachineKey`).
    pub fn is_open_on(&self, machine: &str, id: &str) -> bool {
        self.open_id().as_deref() == Some(id) && self.machine_key_of(id) == machine
    }

    /// The name of the machine the open session is on, when it isn't home (the session header's chip reads it).
    pub fn machine_name(&self) -> Option<String> {
        let key = self.machine_key_of(&self.open_id()?);
        self.machines.machine(key).map(|m| m.name.clone())
    }

    /// Why a message to the open session can't go: its machine isn't answering. The words carry the machine's name.
    pub fn machine_refusal(&self) -> Option<String> {
        let key = self.machine_key_of(&self.open_id()?);
        if self.machines.is_answering(key) {
            return None;
        }
        let name = self.machines.machine(key).map(|m| m.name.as_str()).unwrap_or("That machine");
        Some(format!("{name} isn't answering."))
    }
}
