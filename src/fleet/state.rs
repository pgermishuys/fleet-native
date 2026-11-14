//! How what the connection learns reaches the state. Every method here runs on the UI thread, from
//! an `AppHandle::update` closure, and does no I/O.
use super::link::Connection;
use super::model::{Msg, StreamState};
use super::reducer::{self, Event};
use super::view;
use crate::State;
use crate::mock::{Project, Status};
use rune::Tx;

impl State {
    /// The id of the open session on the server; `None` for the invented ones.
    pub fn open_id(&self) -> Option<String> {
        Some(self.projects.get(self.open.0)?.sessions.get(self.open.1)?.id.clone()).filter(|id| !id.is_empty())
    }

    /// Opens a session in the list and, on a server, asks it for the conversation.
    pub fn open_session(&mut self, project: usize, session: usize, tx: &mut Tx) {
        self.open = (project, session);
        if let Some(p) = self.projects.get_mut(project) {
            p.open = true;
        }
        if let (Some(link), Some(id)) = (&self.link, self.open_id()) {
            self.stream = None;
            link.open(&id);
        }
        tx.scroll_to_bottom("stream");
    }

    /// The session list arrived, or changed. Keeps which projects are folded and which session is open.
    pub fn on_list(&mut self, mut projects: Vec<Project>, tx: &mut Tx) {
        let open_id = self.open_id();
        for project in &mut projects {
            if let Some(old) = self.projects.iter().find(|p| p.name == project.name) {
                project.open = old.open;
            }
        }
        // The open session's conversation isn't in the list; carry it over.
        let old_messages = self.session_mut().map(|s| std::mem::take(&mut s.messages)).unwrap_or_default();
        self.projects = projects;
        let found = open_id.as_ref().and_then(|id| {
            self.projects.iter().enumerate().find_map(|(p, project)| project.sessions.iter().position(|s| &s.id == id).map(|i| (p, i)))
        });
        match found {
            Some(at) => {
                self.open = at;
                if let Some(session) = self.session_mut() {
                    session.messages = old_messages;
                }
            }
            None => {
                self.open = (0, 0);
                self.stream = None;
                if !self.projects.is_empty() {
                    self.open_session(0, 0, tx);
                }
            }
        }
        self.sync_open();
    }

    /// A session's activity changed on the sessions topic.
    pub fn on_activity(&mut self, id: &str, status: Status) {
        let open = self.open_id();
        for session in self.projects.iter_mut().flat_map(|p| p.sessions.iter_mut()).filter(|s| s.id == id) {
            if status == Status::Idle && session.status != Status::Idle {
                session.age = "now".into();
            }
            session.status = status;
        }
        if open.as_deref() == Some(id) && status == Status::Working && self.working_since < 0.0 {
            self.working_since = self.clock;
        }
    }

    /// The open session's snapshot arrived.
    pub fn on_snapshot(&mut self, id: &str, state: StreamState, tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        if state.session_status.working() {
            self.working_since = self.clock;
        }
        self.stream = Some(state);
        self.sync_open();
        tx.scroll_to_bottom("stream");
    }

    /// A frame's worth of the open session's events.
    pub fn on_events(&mut self, id: &str, events: Vec<Event>, _tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        let Some(state) = self.stream.as_mut() else { return };
        let was_working = state.session_status.working();
        for event in &events {
            reducer::apply(state, event);
        }
        if !was_working && state.session_status.working() {
            self.working_since = self.clock;
        }
        // No scroll to the end here: the conversation follows the end by itself while the person is
        // there, and a person who scrolled up to read is left where they are.
        self.sync_open();
    }

    /// Redraws the open session from the reducer's state. Only the messages that changed are
    /// rebuilt: a streaming reply touches the last one, and copying all two thousand every frame
    /// is what a long conversation can't afford. The last message is always rebuilt, so an edit
    /// in place there is never missed; an edit in place to an earlier one that keeps its size is
    /// (the reducer doesn't make those).
    fn sync_open(&mut self) {
        let Some(stream) = &self.stream else { return };
        let shown: Vec<&Msg> = stream.messages.iter().filter(|m| !m.compaction_summary).collect();
        let working = stream.session_status.working();
        // Not `session_mut()`: that would borrow all of `self` while `stream` is still read.
        let (p, i) = self.open;
        let Some(session) = self.projects.get_mut(p).and_then(|p| p.sessions.get_mut(i)) else { return };
        let same_shape = session.messages.len() == shown.len() && session.messages.iter().zip(&shown).all(|(a, b)| a.id == b.id);
        if same_shape {
            let last = shown.len().saturating_sub(1);
            for (i, (old, new)) in session.messages.iter_mut().zip(&shown).enumerate() {
                if i == last || old.version() != view::version(new) {
                    *old = view::message(new);
                }
            }
        } else {
            session.messages = shown.into_iter().map(view::message).collect();
        }
        if working {
            session.status = Status::Working;
        } else if session.status == Status::Working {
            session.status = Status::Idle;
            session.age = "now".into();
        }
    }

    fn session_mut(&mut self) -> Option<&mut crate::mock::Session> {
        let (p, i) = self.open;
        self.projects.get_mut(p)?.sessions.get_mut(i)
    }

    /// What the status bar says about the connection; `None` when all is well or there's no server.
    pub fn connection_note(&self) -> Option<String> {
        match (&self.link, &self.connection) {
            (None, _) | (_, Connection::Online) => self.notice.clone(),
            (_, Connection::Connecting) => Some("Connecting…".to_string()),
            (_, Connection::Down(why)) => Some(format!("Reconnecting: {why}")),
        }
    }
}
