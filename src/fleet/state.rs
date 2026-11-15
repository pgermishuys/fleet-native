//! How what the connection learns reaches the state. Every method here runs on the UI thread, from
//! an `AppHandle::update` closure, and does no I/O.
use super::events::{Event, Kind};
use super::link::Connection;
use super::model::{Msg, StreamState};
use super::prompts::{self, Confirm, PromptStatus};
use super::reducer;
use super::sessions;
use super::view;
use crate::State;
use crate::mock::{Project, Status};
use crate::view::conversation::Message;
use rune::Tx;

/// Where an event came from, for `State::subscribe` (`onGlobalEvent` is `Sessions`, `onDomainEvent`
/// is `Session`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Topic {
    Sessions,
    Session(String),
}

type Listener = Box<dyn FnMut(&Topic, &Event) + Send>;

/// Who wants events the reducer doesn't apply. Components 3-22 attach here, so none of them
/// touches `link.rs`.
#[derive(Default)]
pub struct Subscribers(Vec<(Option<Kind>, Listener)>);

impl State {
    /// Calls `listener` with every event that isn't the open stream's own to apply: on the `sessions`
    /// topic, and on the open session's. `only` limits it to one type. Runs on the UI thread, so it
    /// must not block. (A component that needs another session's topic asks the link for it; the
    /// Files canvas will, in component 11.)
    pub fn subscribe(&mut self, only: Option<Kind>, listener: impl FnMut(&Topic, &Event) + Send + 'static) {
        self.subscribers.0.push((only, Box::new(listener)));
    }

    /// Hands events to the subscribers. Events the reducer applies go too, so a listener can follow
    /// them without a second path; `Cursor` marks are internal and skipped.
    pub fn dispatch(&mut self, topic: &Topic, events: &[Event]) {
        for event in events.iter().filter(|e| !matches!(e, Event::Cursor(_))) {
            let kind = event.kind();
            for (only, listener) in &mut self.subscribers.0 {
                if only.is_none() || *only == kind {
                    listener(topic, event);
                }
            }
        }
    }

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
        if let Some(id) = self.open_id()
            && self.link.is_some()
            && self.stream_of.as_deref() != Some(id.as_str())
        {
            self.keep_current_stream();
            // What was kept shows at once, with the status the list has now; the snapshot that follows replaces it.
            let shown = self.kept.get(&id).cloned().map(|mut kept| {
                let activity = if self.session().status == Status::Working { "busy" } else { "idle" };
                reducer::with_activity_status(&mut kept, activity);
                kept
            });
            self.stream = shown;
            self.stream_of = Some(id.clone());
            self.loading = true;
            if let Some(link) = &self.link {
                link.open(&id);
            }
            self.sync_open();
        }
        tx.scroll_to_bottom("stream");
    }

    /// Keeps the conversation being left, if it came from a snapshot (`keepCurrentStream`).
    fn keep_current_stream(&mut self) {
        let (id, stream) = (self.stream_of.take(), self.stream.take());
        if let (Some(id), Some(stream), false) = (id, stream, self.loading) {
            self.kept.keep(&id, stream);
        }
    }

    /// The connection came back: every conversation shown is out of date until its new snapshot.
    pub fn on_reconnected(&mut self) {
        if self.stream.is_some() {
            self.loading = true;
        }
    }

    /// The session list arrived, or changed. Keeps which projects are folded and which session is open.
    pub fn on_list(&mut self, mut projects: Vec<Project>, tx: &mut Tx) {
        if self.notice.as_deref().is_some_and(|n| n.starts_with("Couldn't load the session list")) {
            self.notice = None;
        }
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
                self.stream_of = None;
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
        self.prompts.on_messages(id, &state.messages);
        self.stream = Some(state);
        self.stream_of = Some(id.to_string());
        self.loading = false;
        self.sync_open();
        tx.scroll_to_bottom("stream");
    }

    /// The server took a prompt: the receipt confirms it, as the committed event does (whichever is first).
    pub fn on_prompt_receipt(&mut self, session: &str, correlation_id: &str, receipt: &serde_json::Value, _tx: &mut Tx) {
        let confirm = Confirm {
            correlation_id: Some(receipt.get("correlationId").and_then(|c| c.as_str()).unwrap_or(correlation_id).to_string()),
            event_id: receipt.get("eventId").and_then(|e| e.as_i64()),
            server_message_id: None,
        };
        self.prompts.confirm(session, &confirm);
        self.notice = None;
        self.sync_open();
    }

    /// The server refused a prompt, or it never got there: it leaves the conversation and its text goes
    /// back to the composer, unless something new was typed meanwhile.
    pub fn on_prompt_refused(&mut self, session: &str, correlation_id: &str, why: String, _tx: &mut Tx) {
        if let Some(prompt) = self.prompts.remove(session, correlation_id)
            && self.draft.text.trim().is_empty()
            && self.open_id().as_deref() == Some(session)
        {
            self.draft = rune::TextEdit::new(prompt.body);
        }
        self.notice = Some(format!("Couldn't send the message: {why}"));
        self.sync_open();
    }

    /// The composer's Send against a server: the prompt joins the conversation as pending, and goes.
    pub fn send_to_server(&mut self, session: &str, text: &str) {
        let Some(link) = &self.link else { return };
        let Some(correlation_id) = self.prompts.add(session, text, super::link::now_ms(), self.clock) else { return };
        link.send_prompt(session, &correlation_id, text.trim());
        self.sync_open();
    }

    /// A frame's worth of the open session's events.
    pub fn on_events(&mut self, id: &str, events: Vec<Event>, tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        self.dispatch(&Topic::Session(id.to_string()), &events);
        self.on_stream_events(id, events, tx);
    }

    /// Events the open stream applies without telling the subscribers: the ones that came on the
    /// `sessions` topic (already dispatched there), and the others after `dispatch`.
    pub fn on_stream_events(&mut self, id: &str, events: Vec<Event>, _tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) || events.is_empty() {
            return;
        }
        for event in &events {
            if let Event::PromptCommitted { life, correlation_id, event_id } = event {
                let confirm = Confirm {
                    correlation_id: correlation_id.clone(),
                    event_id: *event_id,
                    server_message_id: life.info.get("id").and_then(|i| i.as_str()).map(str::to_string),
                };
                self.prompts.confirm(id, &confirm);
            }
        }
        let Some(state) = self.stream.as_mut() else { return };
        let was_working = state.session_status.working();
        for event in &events {
            reducer::apply(state, event);
        }
        if !was_working && state.session_status.working() {
            self.working_since = self.clock;
        }
        self.prompts.on_messages(id, &state.messages);
        // No scroll to the end here: the conversation follows the end by itself while the person is
        // there, and a person who scrolled up to read is left where they are.
        self.sync_open();
    }

    /// Redraws the open session from the reducer's state. Only the messages that changed are
    /// rebuilt: a streaming reply touches the last one, and copying all two thousand every frame
    /// is what a long conversation can't afford. The last message is always rebuilt, so an edit
    /// in place there is never missed; an edit in place to an earlier one that keeps its size is
    /// (the reducer doesn't make those).
    pub(crate) fn sync_open(&mut self) {
        let Some(stream) = &self.stream else { return };
        let shown: Vec<&Msg> = stream.messages.iter().filter(|m| !m.compaction_summary).collect();
        let (p, i) = self.open;
        let sent = self.projects.get(p).and_then(|p| p.sessions.get(i)).map(|s| s.id.clone()).unwrap_or_default();
        // The prompts the conversation doesn't hold yet, after the delivered messages (`mergeMessagesByTimestamp`).
        let optimistic: Vec<Message> = self
            .prompts
            .of(&sent)
            .iter()
            .filter(|prompt| !shown.iter().any(|m| m.id == prompt.id))
            .map(|prompt| {
                let mut m = Message::user(format!("optimistic-{}", prompt.correlation_id), prompt.body.clone());
                m.needs_retry = prompt.status == PromptStatus::NeedsRetry;
                m
            })
            .collect();
        // Not `session_mut()`: that would borrow all of `self` while `stream` is still read.
        let Some(session) = self.projects.get_mut(p).and_then(|p| p.sessions.get_mut(i)) else { return };
        let n = shown.len();
        let same_shape = session.messages.len() == n + optimistic.len()
            && session.messages.iter().zip(&shown).all(|(a, b)| a.id == b.id)
            && session.messages[n..].iter().zip(&optimistic).all(|(a, b)| a.id == b.id);
        if same_shape {
            let last = n.saturating_sub(1);
            for (i, (old, new)) in session.messages.iter_mut().zip(&shown).enumerate() {
                if i == last || old.version() != view::version(new) {
                    *old = view::message(new);
                }
            }
            // A few rows at most, and one may have just turned to needing a retry.
            session.messages.truncate(n);
            session.messages.extend(optimistic);
        } else {
            session.messages = prompts::merge_by_timestamp(shown.into_iter().map(view::message).collect(), optimistic);
        }
        // The row shows the derived status: a sub-agent's question is the amber diamond, every other
        // kind of work the working glyph.
        let status = sessions::stream_status_of(stream.session_status);
        if status != Status::Idle {
            session.status = status;
        } else if session.status == Status::Working {
            // Not NeedsInput: that may be the session's own question, from the list.
            session.status = Status::Idle;
            session.age = "now".into();
        }
    }

    /// A scene: the open session's row as it reads once the reducer has derived its status from a
    /// snapshot with one sub-agent, working (`waiting` false) or stopped on a question.
    pub fn show_sub_agent(&mut self, waiting: bool) {
        let child = if waiting { "waiting_input" } else { "busy" };
        let snapshot = serde_json::json!({
            "messages": [],
            "activityStatus": "idle",
            "delegations": [{"delegationId": "d1", "childSessionId": "c1", "title": "Check the tests", "status": "running", "childActivityStatus": child}],
        });
        let status = sessions::stream_status_of(reducer::create_state(&snapshot).session_status);
        if let Some(session) = self.session_mut() {
            session.status = status;
        }
        if status == Status::Working {
            self.working_since = self.clock;
        }
    }

    /// A scene: the harness was unavailable, so the snapshot is the cached one (`isPartial`).
    pub fn show_partial(&mut self) {
        // No conversation yet, so the banner at the top of the scroll is in view.
        self.open = (usize::MAX, 0);
        self.stream = Some(reducer::create_state(&serde_json::json!({"messages": [], "isPartial": true})));
    }

    fn session_mut(&mut self) -> Option<&mut crate::mock::Session> {
        let (p, i) = self.open;
        self.projects.get_mut(p)?.sessions.get_mut(i)
    }

    /// The open conversation is the harness's cached data, not live (`isPartial`).
    pub fn is_partial(&self) -> bool {
        self.stream.as_ref().is_some_and(|s| s.is_partial)
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
