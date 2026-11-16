//! Invented sessions for the header's scenes and tests (component 8, piece B): a fork, a retrying session, an archived
//! one, a stopped one, a lost connection, one on a machine that stopped answering, a very long title and path, and a
//! subagent's. Names and paths are made up.
use super::machines::listed;
use super::{Project, Status};
use crate::State;
use crate::fleet::groups::ProjectKind;
use crate::fleet::rows::Retrying;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    Fork,
    Retry,
    Archived,
    Stopped,
    Disconnected,
    /// On a machine that stopped answering, mid-turn.
    MachineDown,
    /// A fork with a title, a branch and a path far wider than the header.
    LongText,
    Subagent,
}

const PARENT: &str = "Move the image cache to disk";

impl State {
    /// One session open under its parent. `list` leaves the session list on screen, as the app does; without it the
    /// header's glyph shows.
    pub fn show_header(&mut self, case: Case, dark: bool, list: bool) {
        self.dark = Some(dark);
        self.now_ms = Some(super::rows::NOW);
        self.sidebar_open = list;
        let messages = super::projects().swap_remove(0).sessions.swap_remove(0).messages;
        let parent = listed("a-cache", PARENT, Status::Idle, "40m", None, |_| {});
        let mut open = listed("a-open", "Try a smaller cache limit", Status::Idle, "now", None, |l| {
            if matches!(case, Case::Fork | Case::LongText | Case::Retry) {
                l.forked_from = Some("a-cache".into());
                l.spawn_kind = Some("fork".into());
            }
            if case == Case::Subagent {
                l.parent = Some("a-cache".into());
            }
        });
        open.messages = messages;
        open.row.directory = "/srv/work/orchard-worktrees/fleet-session-4c2d1e90".into();
        open.branch = "fix/cache-limit".into();
        let working = |s: &mut super::Session, activity: &str| {
            s.status = Status::Working;
            s.row.lifecycle = "active".into();
            s.row.activity = activity.into();
        };
        match case {
            Case::Fork => {}
            Case::Retry => {
                working(&mut open, "retry");
                open.row.retrying = Retrying {
                    attempt: Some(3),
                    max_attempts: Some(10),
                    message: Some("API overloaded (529)".into()),
                    next: Some("2024-10-04T12:00:12Z".into()),
                };
            }
            Case::Archived => open.row.archived = true,
            Case::Stopped => open.row.lifecycle = "stopped".into(),
            Case::Disconnected => open.row.lifecycle = "disconnected".into(),
            Case::MachineDown => working(&mut open, "busy"),
            Case::LongText => {
                open.title = "Try a much smaller cache limit and see whether the thumbnails still load quickly on a slow connection".into();
                open.branch = "fix/cache-limit-for-thumbnails-on-slow-connections".into();
                open.row.directory = "/srv/work/orchard-worktrees/some/very/deeply/nested/folders/that/go/on/fleet-session-4c2d1e90".into();
            }
            Case::Subagent => {}
        }
        self.projects = vec![Project {
            id: "mock-orchard".into(),
            kind: ProjectKind::User,
            position: 1,
            name: "Orchard".into(),
            path: "~/code/orchard".into(),
            open: true,
            sessions: vec![open, parent],
        }];
        self.set_open((0, 0));
        if case == Case::MachineDown {
            self.show_machines_only(super::machines::Case::Down);
            self.machines.session_machines.insert("a-open".into(), "m-harbor".into());
        }
    }
}
