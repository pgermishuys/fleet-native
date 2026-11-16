//! Invented machines for the session list's scenes and tests (component 17, piece C): home ("atlas"), a machine that answers
//! ("falcon"), one that stopped answering with its last list kept ("harbor") and one still being read for the first time
//! ("lantern"). Names, sessions and projects are made up.
use super::{Project, Session, Status, session};
use crate::State;
use crate::fleet::groups::{Listing, ProjectInfo, ProjectKind};
use crate::fleet::machine::Machine;
use crate::fleet::machines::{Info, MachineSessions, Source};
use serde_json::json;

/// What a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Home with two sessions, falcon with three (one pinned, a fork under its parent), harbor away, lantern loading.
    Several,
    /// Home and one machine that stopped answering, with its last list kept.
    Down,
    /// Home and one machine whose first read hasn't come back.
    Loading,
    /// A machine whose name is far wider than the panel, with its count still showing.
    LongName,
    /// A machine that answered with no sessions.
    Empty,
}

pub fn machine(id: &str, name: &str) -> Machine {
    Machine {
        id: id.into(),
        name: name.into(),
        base_url: format!("http://{name}.example.test:2113"),
        addresses: Vec::new(),
        token: None,
        os: Some("linux".into()),
        added_at: "2026-01-01T00:00:00Z".into(),
        ssh_destination: None,
        tunnel_up: None,
    }
}

pub fn listed(
    id: &str,
    title: &str,
    status: Status,
    age: &'static str,
    project: Option<(&str, &str)>,
    edit: impl FnOnce(&mut Listing),
) -> Session {
    let mut s = session("", status, age, "main", Vec::new());
    s.title = title.into();
    s.id = id.into();
    s.row.lifecycle = match status {
        Status::Working => "active",
        Status::NeedsInput => "waiting_input",
        Status::Idle => "idle",
    }
    .into();
    s.row.updated = super::rows::NOW - s.ago_secs.unwrap_or(0) * 1000;
    s.ago_secs = None;
    s.listing = Listing {
        project_id: project.map(|p| p.0.into()),
        project_name: project.map(|p| p.1.into()),
        workspace_directory: "/srv/work".into(),
        ..Default::default()
    };
    edit(&mut s.listing);
    s
}

fn project(id: &str, name: &str, position: i64) -> ProjectInfo {
    ProjectInfo { id: id.into(), name: name.into(), kind: ProjectKind::User, position }
}

fn answered(sessions: Vec<Session>, projects: Vec<ProjectInfo>) -> MachineSessions {
    MachineSessions { sessions, projects, error: None, loaded_at: Some(super::rows::NOW), loading: false }
}

fn falcon() -> MachineSessions {
    let lighthouse = Some(("p-lighthouse", "Lighthouse"));
    let garden = Some(("p-garden", "Garden"));
    answered(
        vec![
            listed("f-fork", "Try the other lens model", Status::Working, "now", lighthouse, |l| {
                l.forked_from = Some("f-plan".into());
                l.spawn_kind = Some("fork".into());
            }),
            listed("f-plan", "Plan the lamp rewiring", Status::Idle, "12m", lighthouse, |l| l.pin_order = Some(1.0)),
            listed("f-fence", "Paint the fence", Status::NeedsInput, "3h", lighthouse, |_| {}),
            listed("f-tomato", "Water the tomatoes", Status::Idle, "2d", garden, |_| {}),
        ],
        vec![project("p-lighthouse", "Lighthouse", 1), project("p-garden", "Garden", 2)],
    )
}

fn harbor() -> MachineSessions {
    let docks = Some(("p-docks", "Docks"));
    let mut state = answered(
        vec![
            listed("h-crane", "Calibrate the crane sensors", Status::Working, "5m", docks, |_| {}),
            listed("h-ledger", "Reconcile the tide ledger", Status::Idle, "1d", docks, |_| {}),
        ],
        vec![project("p-docks", "Docks", 1)],
    );
    state.error = Some("Can't reach harbor.".into());
    state
}

impl State {
    /// Home and its machines as a scene shows them. Home's own rows are two invented sessions.
    pub fn show_machines(&mut self, case: Case, dark: bool) {
        self.dark = Some(dark);
        self.now_ms = Some(super::rows::NOW);
        self.projects = vec![Project {
            id: "mock-orchard".into(),
            kind: ProjectKind::User,
            position: 1,
            name: "Orchard".into(),
            path: "~/code/orchard".into(),
            open: true,
            sessions: vec![
                listed("a-cache", "Move the image cache to disk", Status::Working, "now", None, |_| {}),
                listed("a-sizes", "Hero image sizes", Status::Idle, "40m", None, |_| {}),
            ],
        }];
        self.set_open((0, 1));
        let others: Vec<(Machine, MachineSessions)> = match case {
            Case::Several => vec![
                (machine("m-falcon", "falcon"), falcon()),
                (machine("m-harbor", "harbor"), harbor()),
                (machine("m-lantern", "lantern"), MachineSessions { loading: true, ..Default::default() }),
            ],
            Case::Down => vec![(machine("m-harbor", "harbor"), harbor())],
            Case::Loading => vec![(machine("m-lantern", "lantern"), MachineSessions { loading: true, ..Default::default() })],
            Case::LongName => {
                vec![(machine("m-long", "the-observatory-workstation-in-the-north-tower"), falcon())]
            }
            Case::Empty => vec![(machine("m-quiet", "quiet"), answered(Vec::new(), Vec::new()))],
        };
        let m = &mut self.machines;
        m.source = Source::Owner;
        m.home = Info::from_json(&json!({ "id": "home-id", "name": "atlas", "os": "linux" }));
        for (machine, state) in others {
            m.others.insert(machine.id.clone(), state);
            m.list.push(machine);
        }
    }
}
