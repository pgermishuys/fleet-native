//! Invented machines for Fleet's scenes and tests (component 23, piece D): home ("atlas"), which answers, and "harbor", which
//! stopped answering with its last asks kept. Between them four sessions wait on the user: a permission, a question with one
//! pick, a question with several and a session that stopped with an error. Names, folders and words are made up.
use super::machines::{listed, machine};
use super::{Session, Status};
use crate::State;
use crate::asks::question::{QuestionInfo, QuestionOption};
use crate::fleet::machines::Info;
use crate::fleet::mission::{Ask, FeedStatus, MissionMachine};
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
