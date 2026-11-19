//! Keeps the open session's terminal tabs in the store (component 13, piece C; rows 13.35 to 13.38 of
//! `docs/parity/13-terminal.md`). Ports `composables/use-session-terminals.ts`: the list loads when the session opens and after
//! the hub reconnects, only while Fleet has terminals turned on; `terminal.opened` and `terminal.closed` on the session's topic
//! keep it; events that arrive while a load is out are applied again on top of it.
use super::events::{Event, Kind};
use super::http;
use super::state::Topic;
use super::terminal_slot::START_SIZE;
use super::terminals::TerminalEvent;
use crate::State;
use serde_json::Value;

/// The load in flight, and the events that came while it was out.
#[derive(Debug, Default)]
pub struct Loads {
    /// The newest load asked for; an answer carrying an older number is dropped (`loadId`).
    id: u64,
    pending: Option<Pending>,
}

#[derive(Debug)]
struct Pending {
    session: String,
    buffered: Vec<TerminalEvent>,
}

/// `terminal.opened` / `terminal.closed` as the store reads them; `None` for any other event or one missing its ids.
pub fn read_terminal_event(event: &Event) -> Option<TerminalEvent> {
    let Event::Other(kind, payload) = event else { return None };
    let text = |k: &str| payload.get(k).and_then(Value::as_str).map(str::to_string);
    let (session, terminal) = (text("sessionId")?, text("terminalId")?);
    match kind {
        Kind::TerminalOpened => Some(TerminalEvent::Opened { session, terminal, title: text("title").unwrap_or_default() }),
        Kind::TerminalClosed => Some(TerminalEvent::Closed { session, terminal }),
        _ => None,
    }
}

impl State {
    /// `terminalEnabled` of `/api/config/client`: off until the server has said it is on (13.87).
    pub fn terminals_enabled(&self) -> bool {
        self.terminals_allowed == Some(true)
    }

    /// Writes the drawer's open sessions and height to the settings file when they changed (13.39).
    pub(crate) fn persist_terminals(&mut self) {
        if self.terminals.take_dirty() {
            let open: Vec<String> = self.terminals.open_sessions().cloned().collect();
            self.sidebar.save_terminals(open, self.terminals.height());
            self.save_settings();
        }
    }

    /// No load is out (for a test that waits for one to land).
    #[cfg(test)]
    pub(crate) fn terminal_load_idle(&self) -> bool {
        self.terminal_loads.pending.is_none()
    }

    /// Reads the session's terminals (`load`). A failed load is not shown; the next open or reconnect loads again.
    pub fn load_terminals(&mut self, session: &str) {
        if !self.terminals_enabled() {
            return;
        }
        let Some(link) = self.link_for(session) else { return };
        self.terminal_loads.id += 1;
        let mine = self.terminal_loads.id;
        self.terminal_loads.pending = Some(Pending { session: session.to_string(), buffered: Vec::new() });
        // A drawer left open on a session whose shells are gone shouldn't show empty. One opened while this list is on its way is
        // the user asking for a shell, and the list coming back empty doesn't answer that.
        let opened_before = self.terminals.is_open(session);
        let (id, asked) = (session.to_string(), session.to_string());
        link.off_thread(
            move |base| http::terminals(base, &asked),
            move |s, _, result| {
                if s.terminal_loads.id != mine {
                    return;
                }
                let pending = s.terminal_loads.pending.take();
                let Ok(list) = result else { return };
                let empty = list.is_empty();
                s.terminals.set_terminals(&id, list);
                for event in pending.map(|p| p.buffered).unwrap_or_default() {
                    s.terminals.apply_event(&event);
                }
                if empty && opened_before {
                    s.terminals.set_open(&id, false);
                }
                s.persist_terminals();
            },
        );
    }

    /// The session's topic told of a terminal: the open session's store takes it, and the drawer closes when it was the last.
    pub fn on_terminal_event(&mut self, topic: &Topic, event: &Event) {
        let Some(event) = read_terminal_event(event) else { return };
        let session = event.session().to_string();
        if !self.terminals_enabled()
            || !matches!(topic, Topic::Session(id) if *id == session)
            || self.open_id().as_deref() != Some(&session)
        {
            return;
        }
        self.terminals.apply_event(&event);
        if matches!(event, TerminalEvent::Closed { .. }) && self.terminals.terminals_for(&session).is_empty() {
            self.terminals.set_open(&session, false);
            self.persist_terminals();
        }
        if let Some(pending) = self.terminal_loads.pending.as_mut().filter(|p| p.session == session) {
            pending.buffered.push(event);
        }
    }

    /// Opens a new terminal in the open session and shows it (`openNewTerminal`): `POST` with the view's size, else 120 × 14.
    /// A refusal is kept in the server's words for the drawer to show.
    pub fn open_new_terminal(&mut self) {
        let Some(session) = self.terminal.session.clone() else { return };
        if self.pretend_terminals() {
            // No server: the next pretend shell is a tab of its own, named as Fleet's are ("zsh", "zsh 2", ...).
            let taken = |n: usize| self.terminals.terminals_for(&session).iter().any(|t| t.id == format!("mock-{n}"));
            let n = (1..).find(|n| !taken(*n)).unwrap_or(1);
            let title = if n == 1 { "zsh".to_string() } else { format!("zsh {n}") };
            self.terminals.add(&session, http::Terminal { id: format!("mock-{n}"), title, running: true }, true);
            self.terminals.set_open(&session, true);
            self.sync_terminal();
            return;
        }
        let Some(link) = self.link_for(&session) else { return };
        let size = self.terminal.fitted.unwrap_or(START_SIZE);
        self.terminal.starting = true;
        self.terminal.error = None;
        let asked = session.clone();
        link.off_thread(
            move |base| http::terminal_create(base, &asked, size.0 as u32, size.1 as u32),
            move |s, _, result| {
                if s.terminal.session.as_deref() != Some(session.as_str()) {
                    return;
                }
                s.terminal.starting = false;
                match result {
                    Ok(terminal) => {
                        s.terminals.add(&session, terminal, true);
                        s.terminals.set_open(&session, true);
                        s.persist_terminals();
                    }
                    Err(why) => s.terminal.error = Some(why),
                }
            },
        );
    }

    /// Closes the tab showing (the failed panel's Close tab).
    pub fn close_showing_terminal(&mut self) {
        // The setup row's view: its `@ended` closes the setup terminal.
        if self.terminal.setup.is_some() {
            return self.close_setup_terminal();
        }
        match self.terminal.live.as_ref().map(|l| l.id.clone()) {
            // A recorded stream has no shell on a server to close.
            Some(id) if !self.terminal.replaying => self.close_terminal_tab(&id),
            _ => self.end_terminal(),
        }
    }

    /// The tab goes at once, the drawer closes if it was the last, then `DELETE`; if the server refuses, the list is read
    /// again so the tab comes back (`closeTerminalTab`).
    pub fn close_terminal_tab(&mut self, terminal: &str) {
        let Some(session) = self.terminal.session.clone() else { return };
        self.terminals.remove(&session, terminal);
        if self.terminals.terminals_for(&session).is_empty() {
            self.terminals.set_open(&session, false);
            self.terminal.focused = false;
        }
        self.persist_terminals();
        let Some(link) = self.link_for(&session) else { return };
        let (asked, id) = (session.clone(), terminal.to_string());
        link.off_thread(
            move |base| http::terminal_close(base, &asked, &id),
            move |s, _, result| {
                if let Err(why) = result {
                    eprintln!("Failed to close terminal: {why}");
                    s.load_terminals(&session);
                }
            },
        );
    }
}
