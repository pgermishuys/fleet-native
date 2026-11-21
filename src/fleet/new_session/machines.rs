//! What the machine chip and its menu show and do on the New session page (`NSD/MachinePicker.vue`, the machine parts of
//! `NewSessionComposer.vue`; rows B5, A18). Plain state changes: nothing here draws. Which machines there are and what each
//! last said is component 17's `fleet::machines`; this only words it for the menu and moves the draft.
use crate::State;
use crate::conv::time::{DAY, MONTHS, civil, clock, local_offset_secs};
use crate::fleet::link::now_ms;

pub const MACHINE_POPOVER: &str = "machine";
pub const MACHINE_CHIP: &str = "new-session-machine";

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const WEEK: i64 = 7 * DAY;

/// One row of the menu (`Row` in `MachinePicker.vue`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRow {
    pub key: String,
    pub name: String,
    pub reachable: bool,
    /// The second line: `Working here · Linux`, `Linux · reached 3 minutes ago`, `Unreachable · last seen 2 days ago`.
    pub detail: String,
    /// When Fleet last heard from it, exactly: the detail's tooltip (B5.12).
    pub heard_at: Option<String>,
    pub selected: bool,
}

/// `OS_NAMES`: the names of the systems a machine reports; any other is left out.
pub fn os_name(os: &str) -> Option<&'static str> {
    match os {
        "linux" => Some("Linux"),
        "macos" => Some("macOS"),
        "windows" => Some("Windows"),
        _ => None,
    }
}

/// `formatRelative` in English: `just now` under a minute, then whole minutes, hours, days and weeks as
/// `Intl.RelativeTimeFormat` writes them, and from 8 weeks on the date (B5.11).
pub fn format_relative(at: i64, now: i64, offset_secs: i64) -> String {
    let span = (now - at).abs();
    if span < MINUTE {
        return "just now".into();
    }
    if span >= 8 * WEEK {
        let (y, m, d, ..) = civil(at, offset_secs);
        let (ny, ..) = civil(now, offset_secs);
        return if y == ny { format!("{} {d}", MONTHS[m]) } else { format!("{} {d}, {y}", MONTHS[m]) };
    }
    let (value, unit) = if span < HOUR {
        (span / MINUTE, "minute")
    } else if span < DAY {
        (span / HOUR, "hour")
    } else if span < WEEK {
        (span / DAY, "day")
    } else {
        (span / WEEK, "week")
    };
    let words = format!("{value} {unit}{}", if value == 1 { "" } else { "s" });
    if now >= at { format!("{words} ago") } else { format!("in {words}") }
}

/// `formatDateTime`: `Oct 12, 2:07 PM`, with the year when it is not this one.
pub fn format_date_time(at: i64, now: i64, offset_secs: i64) -> String {
    let (y, m, d, h, mi) = civil(at, offset_secs);
    let (ny, ..) = civil(now, offset_secs);
    let day = if y == ny { format!("{} {d}", MONTHS[m]) } else { format!("{} {d}, {y}", MONTHS[m]) };
    format!("{day}, {}", clock(h, mi))
}

impl State {
    fn clock_now(&self) -> i64 {
        self.now_ms.unwrap_or_else(now_ms)
    }

    /// `rows`: every machine, home first, with whether it answers and what to say of it (B5.7-B5.10, B5.17).
    pub fn machine_rows(&self) -> Vec<MachineRow> {
        let (now, offset) = (self.clock_now(), local_offset_secs());
        let live = self.machines.current_key().to_string();
        let selected = self.target_key();
        self.machines
            .entries()
            .into_iter()
            .map(|e| {
                let os = e.os.as_deref().and_then(os_name);
                let (reachable, detail, heard_at) = if e.key == live {
                    // The live machine answers while the app has its hub (`liveReachable`).
                    let words = ["Working here"].into_iter().chain(os).collect::<Vec<_>>().join(" · ");
                    (true, words, None)
                } else {
                    let heard = self.machines.others.get(&e.key);
                    let reachable = heard.is_none_or(|h| h.error.is_none());
                    let at = heard.and_then(|h| h.loaded_at).filter(|at| *at > 0);
                    let when = at.map(|at| format_relative(at, now, offset));
                    let detail = if reachable {
                        os.map(str::to_string).into_iter().chain(when.map(|w| format!("reached {w}"))).collect::<Vec<_>>().join(" · ")
                    } else {
                        match when {
                            Some(w) => format!("Unreachable · last seen {w}"),
                            None => "Unreachable".into(),
                        }
                    };
                    (reachable, detail, at.map(|at| format_date_time(at, now, offset)))
                };
                MachineRow { selected: e.key == selected, key: e.key, name: e.name, reachable, detail, heard_at }
            })
            .collect()
    }

    pub fn machine_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(MACHINE_POPOVER))
    }

    pub fn open_machine_menu(&mut self) {
        self.ui.open = Some(crate::ui::Open::Popover(MACHINE_POPOVER));
    }

    pub fn close_machine_menu(&mut self) {
        if self.machine_menu_open() {
            self.ui.open = None;
        }
    }

    /// An unreachable machine can't be picked (B5.13); the live one is the same as none (A21.204).
    pub fn pick_machine(&mut self, key: &str) {
        if !self.machine_rows().iter().any(|r| r.key == key && r.reachable) {
            return;
        }
        self.close_machine_menu();
        if self.picker_host() == super::host::Host::Automation {
            return self.automation_pick_machine(key);
        }
        let live = self.machines.current_key().to_string();
        let before = self.target_key();
        self.new_draft.set_machine(Some(key), &live);
        if self.target_key() != before {
            // Fleet rebuilds the box for the machine: everything in it asks that machine now (A18.3).
            self.read_folder_feeds();
            self.read_harness_feeds();
        }
    }

    /// Add a machine… (B5.15): Settings, on Machines, with the add form asked for. Settings is component 16's.
    pub fn add_machine_from_menu(&mut self, _tx: &mut crate::Tx) {
        self.close_machine_menu();
        self.add_machine();
    }
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// A state of the machine menu for the pictures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The strip with only this machine, as a Fleet with no other machines shows it.
    One,
    /// The menu with only this machine: it ends with Add a machine….
    OneMenu,
    /// Three machines: this one, one that answers and one that stopped answering.
    Menu,
    /// The strip after the draft moved to the machine that answers.
    Moved,
}

impl State {
    pub(crate) fn show_machine_scene(&mut self, scene: Scene) {
        if !matches!(scene, Scene::One | Scene::OneMenu) {
            self.show_machines_only(crate::mock::machines::Case::Several);
            // Lantern is still being read for the first time; the menu would show a machine with no news, so keep the picture to
            // the three that say something.
            self.machines.list.retain(|m| m.id != "m-lantern");
            self.machines.others.remove("m-lantern");
            // What each last said, a while ago, so the menu's second lines read as they do beside a real machine.
            let ago = |minutes: i64| Some(crate::mock::rows::NOW - minutes * 60_000);
            for (id, at) in [("m-falcon", ago(3)), ("m-harbor", ago(2 * 24 * 60))] {
                if let Some(m) = self.machines.others.get_mut(id) {
                    m.loaded_at = at;
                }
            }
        } else {
            self.machines.home =
                crate::fleet::machines::Info::from_json(&serde_json::json!({ "id": "home-id", "name": "atlas", "os": "linux" }));
        }
        self.now_ms = Some(crate::mock::rows::NOW);
        self.open_new_session_page(None);
        if scene == Scene::Moved {
            self.pick_machine("m-falcon");
        }
        if matches!(scene, Scene::OneMenu | Scene::Menu) {
            self.open_machine_menu();
        }
    }
}
