//! Invented machines for Fleet's scenes and tests (component 23, piece D): home ("atlas"), which answers, and "harbor", which
//! stopped answering with its last asks kept. Between them four sessions wait on the user: a permission, a question with one
//! pick, a question with several and a session that stopped with an error. Names, folders and words are made up.
use super::machines::{listed, machine};
use super::{Session, Status};
use crate::State;
use crate::asks::question::{QuestionInfo, QuestionOption};
use crate::fleet::machines::Info;
use crate::fleet::mission::peek::PeekEvent;
use crate::fleet::mission::peeking::{Helper, Peek};
use crate::fleet::mission::{Ask, FeedStatus, Grouping, MissionMachine, NowKind, SessionNow};
use crate::fleet::row_chips::Progress;
use crate::fleet::settings::HOME_MACHINE_KEY;
use serde_json::json;
use std::collections::HashMap;

/// What a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Four cards: a permission, a one-pick question, a many-pick question (home), an error (home) and harbor's.
    Cards,
    /// Every machine answered and nothing waits.
    Empty,
    /// No machine has answered yet.
    Loading,
    /// The cards, with the permission's answer refused.
    Failed,
}

fn option(label: &str, description: &str) -> QuestionOption {
    QuestionOption { label: label.into(), description: description.into() }
}

fn question(text: &str, options: Vec<QuestionOption>, multiple: bool) -> Ask {
    Ask::Question {
        request_id: "que_1".into(),
        question: QuestionInfo { header: String::new(), question: text.into(), options, multiple, custom: true },
        more: 0,
    }
}

fn session(id: &str, title: &str, status: Status, age: &'static str, folder: &str) -> Session {
    let mut s = listed(id, title, status, age, Some((&format!("p-{folder}"), folder)), |_| {});
    s.row.harness_type = Some("opencode2".into());
    s
}

/// Atlas (home): a permission, a question with one pick, one with several, an error, and a session at work.
fn atlas(answered: bool) -> MissionMachine {
    let mut ask = crate::mock::asks::read(crate::mock::asks::Case::Command);
    let mut sessions = vec![
        session("a-migrate", "Add cursor paging to the order list", Status::NeedsInput, "2m", "orders"),
        session("a-pick", "Choose the retry budget", Status::NeedsInput, "9m", "ledger"),
        session("a-many", "Plan the archive clean-up", Status::NeedsInput, "21m", "ledger"),
        session("a-crash", "Rebuild the image cache", Status::Idle, "35m", "orders"),
        session("a-work", "Rename the tide table", Status::Working, "now", "tides"),
    ];
    sessions[3].row.lifecycle = "error".into();
    ask.session_id = "a-migrate".into();
    let mut asks = HashMap::new();
    asks.insert("a-migrate".to_string(), Ask::Permission(Box::new(ask)));
    asks.insert(
        "a-pick".to_string(),
        question(
            "Retry three times or five?",
            vec![option("Three", "Fail fast and tell the caller."), option("Five", "Ride out a slow minute.")],
            false,
        ),
    );
    asks.insert(
        "a-many".to_string(),
        question("Which folders may the clean-up remove?", vec![option("tmp", ""), option("cache", ""), option("logs", "")], true),
    );
    if !answered {
        sessions.clear();
        asks.clear();
    }
    MissionMachine {
        id: HOME_MACHINE_KEY.into(),
        name: "atlas".into(),
        status: if answered { FeedStatus::Live } else { FeedStatus::Connecting },
        last_heard_at: answered.then_some(super::rows::NOW),
        sessions,
        asks,
        now: HashMap::new(),
    }
}

/// Harbor: stopped answering; what it last said is kept.
fn harbor(answered: bool) -> MissionMachine {
    let mut asks = HashMap::new();
    let mut sessions = Vec::new();
    if answered {
        sessions.push(session("h-crane", "Calibrate the crane sensors", Status::NeedsInput, "14m", "docks"));
        asks.insert("h-crane".to_string(), question("Which sensor first?", vec![option("North", ""), option("South", "")], false));
    }
    MissionMachine {
        id: "m-harbor".into(),
        name: "harbor".into(),
        status: if answered { FeedStatus::Unreachable } else { FeedStatus::Connecting },
        last_heard_at: answered.then_some(super::rows::NOW - 600_000),
        sessions,
        asks,
        now: HashMap::new(),
    }
}

/// The machines of a case.
pub fn machines(case: Case) -> Vec<MissionMachine> {
    match case {
        Case::Cards | Case::Failed => vec![atlas(true), harbor(true)],
        Case::Empty => {
            let mut home = atlas(true);
            home.asks.clear();
            home.sessions.retain(|s| s.id == "a-work");
            vec![home, MissionMachine { status: FeedStatus::Live, ..harbor(false) }.with_heard()]
        }
        Case::Loading => vec![atlas(false), harbor(false)],
    }
}

impl MissionMachine {
    fn with_heard(mut self) -> Self {
        self.last_heard_at = Some(super::rows::NOW);
        self
    }
}

impl State {
    /// The Fleet view on Needs you with `case`'s machines in place of feeds.
    pub fn show_mission(&mut self, case: Case, dark: bool) {
        self.dark = Some(dark);
        self.now_ms = Some(super::rows::NOW);
        self.pickers.prefs.adopt(&json!({ "features.mission-control.enabled": "true" }));
        let m = &mut self.machines;
        m.home = Info::from_json(&json!({ "id": "home-id", "name": "atlas", "os": "linux" }));
        m.list.push(machine("m-harbor", "harbor"));
        self.mission.fixed = Some(machines(case));
        self.open_fleet_address("/fleet");
        if case == Case::Failed {
            let key = format!("{HOME_MACHINE_KEY}:a-migrate");
            self.mission.errors.insert(key, "It answered 500.".into());
        }
    }
}

/// What a scene of a machine's view shows (component 23, piece E).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MachineCase {
    /// Atlas's sessions grouped by folder.
    Tiles,
    /// The same, grouped by state.
    ByState,
    /// A working tile picked: the peek beside the list.
    Peek,
    /// Atlas hasn't answered yet.
    Asking,
    /// Harbor stopped answering and has no list to keep.
    Unreachable,
    /// Atlas answered and has nothing working or waiting.
    Empty,
    /// The address names a machine that isn't listed.
    Gone,
}

fn now(text: &str, kind: NowKind) -> SessionNow {
    SessionNow { text: text.into(), kind, at: String::new() }
}

/// Atlas as a machine's view shows it: the cards' sessions, one at work with todos and two helpers, one that just finished.
fn atlas_busy() -> MissionMachine {
    let mut m = atlas(true);
    let mut done = session("a-done", "Tidy the release notes", Status::Idle, "40m", "tides");
    done.row.selected_model = Some(crate::composer::choice::ModelReference::new("anthropic", "claude-sonnet-4-5"));
    m.sessions.push(done);
    for session in &mut m.sessions {
        match session.id.as_str() {
            "a-work" => {
                session.row.progress = Some(Progress { kind: "todos".into(), done: 3, total: 7, current: None });
                session.row.running_work = 2;
                session.row.selected_model = Some(crate::composer::choice::ModelReference::new("anthropic", "claude-opus-4-1"));
                session.branch = "feature/tide-table".into();
            }
            "a-migrate" => session.branch = "orders/paging".into(),
            _ => {}
        }
    }
    m.now.insert("a-work".into(), now("edit · src/tides/table.rs", NowKind::Tool));
    m.now.insert("a-done".into(), now("Release notes tidied; nothing else changed.", NowKind::Reply));
    m
}

fn peek_of_work() -> Peek {
    let at = |n: &str| PeekEvent { key: format!("m{n}:0"), at: None, text: String::new() };
    let events = [
        "read · src/tides/table.rs",
        "grep · rename_column",
        "edit · src/tides/table.rs",
        "bash · cargo test tides",
        "Renamed the column; the tests for the tide table pass.",
        "edit · src/tides/mod.rs",
    ]
    .iter()
    .enumerate()
    .map(|(n, words)| PeekEvent { text: (*words).into(), ..at(&n.to_string()) })
    .collect();
    let helpers = vec![
        Helper { id: "w1".into(), title: "explore".into(), label: Some("find every use of the old name".into()) },
        Helper { id: "w2".into(), title: "bash".into(), label: Some("cargo test --workspace".into()) },
    ];
    Peek::shown("home:a-work", events, helpers)
}

impl State {
    /// A machine's view of `case`, with its machines in place of feeds.
    pub fn show_machine(&mut self, case: MachineCase, dark: bool) {
        self.show_mission(Case::Cards, dark);
        let mut list = vec![atlas_busy(), harbor(true)];
        let mut path = "/fleet/machines/home";
        match case {
            MachineCase::Tiles | MachineCase::Gone => {}
            MachineCase::ByState => self.mission.grouping = Grouping::State,
            MachineCase::Peek => {
                self.mission.picked = Some("home:a-work".into());
                self.mission.peek = peek_of_work();
            }
            MachineCase::Asking => list = machines(Case::Loading),
            MachineCase::Unreachable => {
                list[1] = MissionMachine { sessions: Vec::new(), asks: HashMap::new(), last_heard_at: None, ..harbor(true) };
                path = "/fleet/machines/m-harbor";
            }
            MachineCase::Empty => {
                list[0].sessions.clear();
                list[0].asks.clear();
            }
        }
        if case == MachineCase::Gone {
            path = "/fleet/machines/nowhere";
        }
        self.mission.fixed = Some(list);
        self.open_fleet_address(path);
    }
}
