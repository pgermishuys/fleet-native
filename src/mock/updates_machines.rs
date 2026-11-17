//! Scenes of Settings → Updates & about › Machines: home ("atlas") and three machines in the states the group can show. All names,
//! versions and times are invented; no server is asked and nothing is updated.
use super::machines::Case as Machines;
use crate::State;
use crate::fleet::machines::Info;
use crate::fleet::updates::Rollout;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// A release is out: home and falcon have it downloaded, harbor can't be reached, lantern is current.
    Available,
    /// Every machine runs the newest release.
    Current,
    /// No machine knows of a release.
    NoRelease,
    /// A rollout is under way: falcon waits for two sessions, atlas is next, harbor is skipped.
    Rolling,
    /// A rollout finished, and the last request was refused.
    Finished,
    /// falcon's update didn't start and was undone.
    RolledBack,
}

fn update(status: &str, latest: Option<&str>, staged: Option<&str>, checked: &str, rule: &str) -> Value {
    json!({ "status": status, "latestVersion": latest, "stagedVersion": staged, "checkedAt": checked, "canRestart": true, "rule": rule })
}

fn machine(id: &str, name: &str, os: &str, version: &str, working: u32, update: Value) -> Info {
    Info::from_json(&json!({
        "id": id, "name": name, "hostName": name, "os": os, "version": version, "apiVersion": 1, "authMode": "token",
        "remoteReachable": true, "requiresToken": true, "webApp": true,
        "capabilities": { "permissionLevel": "all", "harnesses": [], "sessions": { "working": working, "needsYou": 0 } },
        "update": update,
    }))
    .expect("an object")
}

fn rollout(status: &str, steps: Value) -> Rollout {
    Rollout::from_json(&json!({ "id": "r1", "status": status, "startedAt": "2026-11-17T09:00:00Z", "steps": steps })).expect("a rollout")
}

impl State {
    pub fn show_updates_machines(&mut self, dark: bool, case: Case) {
        self.show_machines(Machines::Several, dark);
        self.dark = Some(dark);
        self.rail = crate::Rail::Settings;
        let checked = crate::fleet::session_commands::iso_utc(super::rows::NOW - 2 * 3_600_000);
        let staged = |v: &str| update("staged", Some(v), Some(v), &checked, "idle");
        let current = |v: &str| update("uptodate", Some(v), None, &checked, "idle");
        let none = update("unknown", None, None, &checked, "idle");
        let (home, falcon, lantern) = match case {
            Case::Available | Case::Rolling | Case::Finished => {
                (staged("0.73.0"), staged("0.73.0"), update("available", Some("0.73.0"), None, &checked, "ask"))
            }
            Case::Current => (current("0.72.0"), current("0.72.0"), current("0.72.0")),
            Case::NoRelease => (none.clone(), none.clone(), none),
            Case::RolledBack => {
                let mut f = update("uptodate", Some("0.73.0"), None, &checked, "idle");
                f["rolledBack"] = json!({ "from": "0.72.0", "to": "0.73.0", "reason": "the new version didn't start", "at": checked });
                (current("0.72.0"), f, current("0.72.0"))
            }
        };
        let m = &mut self.machines;
        m.home = Some(machine("home-id", "atlas", "linux", "0.72.0", 3, home));
        m.identities.insert("m-falcon".into(), machine("m-falcon", "falcon", "macos", "0.72.0", 0, falcon));
        m.identities.insert("m-lantern".into(), machine("m-lantern", "lantern", "windows", "0.72.0", 0, lantern));
        if let Some(l) = m.others.get_mut("m-lantern") {
            l.loading = false;
            l.error = None;
        }
        match case {
            Case::Rolling => {
                self.updates.rollout = Some(rollout(
                    "running",
                    json!([
                        { "machineId": "m-falcon", "name": "falcon", "state": "waiting", "waitingSessions": 2, "toVersion": "0.73.0" },
                        { "machineId": "home", "name": "atlas", "state": "pending", "waitingSessions": 0, "toVersion": "0.73.0" },
                        { "machineId": "m-harbor", "name": "harbor", "state": "pending", "waitingSessions": 0, "toVersion": "0.73.0" },
                    ]),
                ));
            }
            Case::Finished => {
                self.updates.rollout = Some(rollout(
                    "done",
                    json!([
                        { "machineId": "m-falcon", "name": "falcon", "state": "done", "waitingSessions": 0, "toVersion": "0.73.0" },
                        { "machineId": "m-harbor", "name": "harbor", "state": "skipped", "detail": "it can't be reached", "waitingSessions": 0 },
                    ]),
                ));
                self.updates.action_error = Some("A rollout is already running.".into());
            }
            _ => {}
        }
        self.pickers.prefs.fetched = true;
        self.updates.last_seen.0.insert("m-harbor".into(), "0.71.4".into());
        self.updates.last_seen_loaded_for_scene();
        self.open_settings("updates", Some("machines"));
    }
}
