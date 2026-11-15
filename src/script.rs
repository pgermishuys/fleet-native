//! The scripted reply in the "Long conversation" session: with no server, the agent there answers
//! with `mock::long::STREAM_REPLY` at a fixed rate, over and over, so a scroll can be measured
//! while a reply streams in. It runs on the app's clock, so every run draws the same thing.
use crate::State;
use crate::mock::{Status, long};
use crate::view::conversation::{Message, Part};

/// Seconds between one reply ending and the next question.
const PAUSE: f64 = 1.5;

pub struct Script {
    /// The (project, session) the reply is written into.
    at: (usize, usize),
    /// When the reply began, on the app's clock.
    started: f64,
    /// Bytes of the reply shown so far.
    shown: usize,
}

impl State {
    /// Where the long conversation sits in the mock list, if it's there.
    pub fn long_session(&self) -> Option<(usize, usize)> {
        self.projects
            .iter()
            .enumerate()
            .find_map(|(p, project)| project.sessions.iter().position(|s| s.title == long::TITLE).map(|i| (p, i)))
    }

    /// Starts the next reply: the question, then an empty answer for `tick` to fill.
    fn start_script(&mut self, at: (usize, usize)) {
        let session = &mut self.projects[at.0].sessions[at.1];
        let n = session.messages.len() / 2;
        session.messages.push(Message::user(format!("u{n}"), "Where would you start on the search index?"));
        session.messages.push(Message::assistant(format!("a{n}"), vec![Part::text(format!("a{n}-p0"), "")]));
        session.status = Status::Working;
        session.touch(crate::fleet::link::now_ms());
        self.working_since = self.clock;
        self.script = Some(Script { at, started: self.clock, shown: 0 });
    }

    /// Shows the reply's text up to now. True when the picture changed.
    pub fn tick_script(&mut self) -> bool {
        let Some(at) = self.long_session() else { return false };
        // Only while a person is looking at it, and not on a server's sessions.
        if self.link.is_some() || self.open_at() != at {
            return false;
        }
        let Some(script) = &mut self.script else {
            if self.clock >= self.script_next {
                self.start_script(at);
                return true;
            }
            return false;
        };
        let wanted = (((self.clock - script.started) * long::STREAM_CPS) as usize).min(long::STREAM_REPLY.len());
        if wanted == script.shown {
            return false;
        }
        script.shown = wanted;
        let session = &mut self.projects[script.at.0].sessions[script.at.1];
        if let Some(Part::Text { text, .. }) = session.messages.last_mut().and_then(|m| m.parts.last_mut()) {
            text.clear();
            text.push_str(&long::STREAM_REPLY[..wanted]);
        }
        if wanted == long::STREAM_REPLY.len() {
            session.status = Status::Idle;
            self.script = None;
            self.script_next = self.clock + PAUSE;
        }
        true
    }

    /// Opens the long conversation with its reply held back, so a picture shows it at rest.
    pub fn open_long(&mut self) {
        if let Some(at) = self.long_session() {
            self.set_open(at);
            self.projects[at.0].open = true;
            self.script_next = f64::INFINITY;
        }
    }

    /// Opens the long conversation with its reply about to stream, as it does for a person.
    pub fn open_long_live(&mut self) {
        self.open_long();
        self.script_next = 0.0;
    }

    /// Puts the long conversation partway through a reply and holds it there, for a picture: the
    /// text is fixed, not read off the clock, so the picture is the same on every run.
    pub fn show_streaming(&mut self, shown: usize) {
        let Some(at) = self.long_session() else { return };
        self.set_open(at);
        self.projects[at.0].open = true;
        self.start_script(at);
        self.script = None;
        self.script_next = f64::INFINITY;
        let shown = shown.min(long::STREAM_REPLY.len());
        if let Some(Part::Text { text, .. }) = self.projects[at.0].sessions[at.1].messages.last_mut().and_then(|m| m.parts.last_mut()) {
            text.push_str(&long::STREAM_REPLY[..shown]);
        }
    }
}
