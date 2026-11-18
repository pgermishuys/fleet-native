//! The open session's showing terminal, end to end (component 13, pieces A and C): the active tab's socket is opened and the
//! grid reads what the shell printed. Which tabs there are, which one is active and whether the drawer is open is
//! `fleet::terminals` (loaded and kept by `session_terminals.rs`); piece D draws the drawer around it.
use std::cell::RefCell;
use std::sync::Arc;

use rune::Tx;

use super::http::{self, Terminal as Listed};
use super::terminal_socket::{Socket, Status};
use crate::State;
use crate::term::Terminal;
use crate::term::fit::fit_box;
use crate::term::screen::Screen;

/// What a shell is started with while the view hasn't measured itself yet (`openNewTerminal`'s 120 × 14).
pub const START_SIZE: (u16, u16) = (120, 14);

/// The key of the terminal's focusable box.
pub const VIEW_KEY: &str = "terminal-view";

/// One attached shell: what the socket feeds, what the view reads, and the socket itself.
pub struct Live {
    pub id: String,
    pub title: String,
    pub term: Arc<Terminal>,
    pub screen: RefCell<Screen>,
    pub socket: Option<Socket>,
    /// A pretend shell (`mock::terminal`, in `Slot::shells`) answers typing: there is no server.
    pub pretend: bool,
}

#[derive(Default)]
pub struct Slot {
    /// The session this belongs to; None for the invented data's scenes.
    pub session: Option<String>,
    /// The pretend shells there have been, so reopening one replays its output.
    pub shells: crate::mock::terminal::Shells,
    /// A recorded stream or a pretend shell is shown, with no session behind it (the scenes).
    pub replaying: bool,
    /// The create call is out.
    pub starting: bool,
    /// Why no shell started, in the server's words.
    pub error: Option<String>,
    pub live: Option<Live>,
    /// The other tabs' views, connected and kept while the session is open (13.51): switching tabs doesn't reconnect, and
    /// the shell's output keeps landing in them.
    pub parked: Vec<Live>,
    /// The size the view's box fits, once it has been laid out.
    pub fitted: Option<(u16, u16)>,
    /// The box has the keyboard.
    pub focused: bool,
    /// The drawer's height for a scene; 0 is the remembered one (`Terminals::height`).
    pub height: f32,
    /// The grip is held (drawn lit while it is).
    pub dragging: bool,
}

impl Slot {
    /// A terminal fed by `bytes` instead of a socket: the invented data's scenes and the tests.
    pub fn replay(&mut self, cols: u16, rows: u16, bytes: &[u8]) {
        let term = Arc::new(Terminal::new(cols, rows));
        term.feed(bytes);
        self.live = Some(Live { id: "replay".into(), title: "zsh".into(), term, screen: RefCell::default(), socket: None, pretend: false });
        self.fitted = Some((cols, rows));
        self.replaying = true;
    }

    pub fn status(&self) -> Option<Status> {
        self.live.as_ref().and_then(|l| l.socket.as_ref()).map(Socket::status)
    }
}

impl State {
    /// Ctrl/Cmd+J and the header's terminal button (`toggle-terminal`): shows or hides the open session's drawer, which is
    /// remembered (13.39). Nothing happens while Fleet has terminals turned off (13.87).
    pub fn toggle_terminal(&mut self, tx: &mut Tx) {
        if self.terminal.replaying {
            self.terminal.replaying = false;
            return;
        }
        let Some(session) = self.open_id().filter(|_| self.terminals_shown()) else { return };
        self.follow_terminal_session();
        if self.pretend_terminals() && !self.terminals.is_loaded(&session) {
            self.terminals.set_terminals(&session, vec![]);
        }
        // Normally the list came when the session opened; ask again if it never did (terminals were switched on since). It is
        // asked before the drawer opens, so an empty answer is not read as "left open on a session with no shells".
        if !self.terminals.is_open(&session) && !self.terminals.is_loaded(&session) {
            self.load_terminals(&session);
        }
        self.terminals.toggle_open(&session);
        self.persist_terminals();
        // Closing the drawer clears the error it showed (13.50).
        self.terminal.error = None;
        if self.terminals.is_open(&session) {
            self.sync_terminal();
            tx.focus(VIEW_KEY, false);
        } else {
            self.terminal.focused = false;
        }
    }

    /// The drawer's Hide: the shells keep running (13.45).
    pub fn hide_terminal(&mut self) {
        if let Some(session) = self.terminal.session.clone() {
            self.terminals.set_open(&session, false);
            self.persist_terminals();
        }
        self.terminal.replaying = false;
        self.terminal.error = None;
        self.terminal.focused = false;
    }

    /// A tab pressed: it shows, and its view takes the keyboard (`activate`, and the view's `shown`).
    pub fn show_terminal_tab(&mut self, id: &str, tx: &mut Tx) {
        if let Some(session) = self.terminal.session.clone() {
            self.terminals.set_active(&session, id);
        }
        self.sync_terminal();
        tx.focus(VIEW_KEY, false);
    }

    /// Copy (Ctrl Shift C, ⌘ C): puts the box's selection on the clipboard. Nothing can be selected until piece F builds
    /// selection, so the key is taken (the shell never gets ^C from it) and does nothing yet.
    pub fn copy_terminal_selection(&mut self, _tx: &mut Tx) {}

    /// A tab picked from the keyboard: it shows, and the focus stays on the tab strip (`activate`).
    pub fn activate_terminal_tab(&mut self, id: &str) {
        if let Some(session) = self.terminal.session.clone() {
            self.terminals.set_active(&session, id);
        }
        self.sync_terminal();
    }

    /// The grip moved the drawer: a new height, whole pixels, never under the minimum. A scene's own height gives way. A drag keeps it
    /// in the settings file when it ends, not at every move.
    pub fn set_drawer_height(&mut self, height: f32) {
        self.terminal.height = 0.0;
        self.terminals.set_height(height);
    }

    /// The drawer's Clear (`clearActive`): the screen and the scrollback go from here and, for everyone else, from the server.
    pub fn clear_terminal(&mut self) {
        let Some(live) = self.terminal.live.as_ref() else { return };
        live.term.clear();
        if live.pretend {
            self.terminal.shells.open(&live.id).clear();
        }
        if let Some(socket) = &live.socket {
            socket.clear();
        }
    }

    /// `useCommands`' `terminalSessionId`: the open session, which the toggle acts on.
    pub fn terminals_shown(&self) -> bool {
        self.terminals_enabled() || self.pretend_terminals()
    }

    /// No server: the invented session gets the pretend shell (13.42), unless the server's answer was "off".
    pub fn pretend_terminals(&self) -> bool {
        self.is_mock() && self.terminals_allowed != Some(false)
    }

    /// The header button's state: (the drawer is open, shells are running with it hidden) (13.55).
    pub fn terminal_toggle_state(&self) -> (bool, bool) {
        let open = self.terminal_open();
        let running = !open && self.open_id().is_some_and(|id| !self.terminals.terminals_for(&id).is_empty());
        (open, running)
    }

    /// `TerminalToggleButton.vue`'s label and tooltip.
    pub fn terminal_toggle_words(&self) -> String {
        let key = if cfg!(target_os = "macos") { "\u{2318} J" } else { "Ctrl J" };
        match self.terminal_toggle_state() {
            (true, _) => format!("Hide terminal ({key})"),
            (_, true) => format!("Show terminal: shells are running ({key})"),
            _ => format!("Show terminal ({key})"),
        }
    }

    /// The tabs the drawer draws: the session's list, or for a scene's stream the one tab it is.
    pub fn drawer_tabs(&self) -> Vec<(String, String)> {
        let listed: Vec<_> = self
            .terminal
            .session
            .as_deref()
            .map(|id| self.terminals.terminals_for(id).iter().map(|t| (t.id.clone(), t.title.clone())).collect())
            .unwrap_or_default();
        match (&self.terminal.live, listed.is_empty()) {
            (Some(live), true) if self.terminal.replaying => vec![(live.id.clone(), live.title.clone())],
            _ => listed,
        }
    }

    /// The id of the tab showing.
    pub fn drawer_active(&self) -> Option<String> {
        match (&self.terminal.live, self.terminal.replaying) {
            (Some(live), true) => Some(live.id.clone()),
            _ => self.terminal.session.as_deref().and_then(|id| self.terminals.active_for(id)).map(|t| t.id.clone()),
        }
    }

    /// Whether the drawer is showing: the open session's is open, or a scene's stream is on screen.
    pub fn terminal_open(&self) -> bool {
        self.terminal.replaying || self.terminal.session.as_deref().is_some_and(|id| self.terminals.is_open(id))
    }

    /// The slot belongs to the open session: coming back to a session, or opening another, drops the views and closes their
    /// sockets (13.41); the shells keep running on the server, and the scrollback replays when the view connects again.
    fn follow_terminal_session(&mut self) -> bool {
        let open = self.open_id();
        if self.terminal.replaying || self.terminal.session == open {
            return false;
        }
        self.terminal = Slot { session: open, fitted: self.terminal.fitted, ..Default::default() };
        true
    }

    /// Opens the socket of `listed` in the slot, replacing what it showed.
    fn attach_terminal(&mut self, session: &str, listed: &Listed) {
        let fit = self.terminal.fitted.unwrap_or(START_SIZE);
        let term = Arc::new(Terminal::new(fit.0, fit.1));
        if self.pretend_terminals() {
            term.feed(self.terminal.shells.open(&listed.id).replay());
            let (id, title) = (listed.id.clone(), listed.title.clone());
            self.terminal.live = Some(Live { id, title, term, screen: RefCell::default(), socket: None, pretend: true });
            return;
        }
        let Some(link) = self.link_for(session) else { return };
        let path = format!("/api/sessions/{}/terminals/{}", http::escape(session), http::escape(&listed.id));
        let changed = link.post();
        let socket = Socket::open(&link.base, &path, fit, term.clone(), move || changed.update(|_, _| {}));
        self.terminal.live = Some(Live {
            id: listed.id.clone(),
            title: listed.title.clone(),
            term,
            screen: RefCell::default(),
            socket: Some(socket),
            pretend: false,
        });
    }

    /// Keeps what the slot shows in step with the session's tabs: the active tab's view is connected, a view whose tab went is
    /// dropped, and a drawer that is open on a loaded list with no tab starts a shell (13.35). True when it changed.
    pub(super) fn sync_terminal(&mut self) -> bool {
        let mut changed = self.follow_terminal_session();
        let Some(session) = self.terminal.session.clone().filter(|_| !self.terminal.replaying) else { return changed };
        if !self.terminals.is_open(&session) || !self.terminals_shown() {
            return changed;
        }
        match self.terminals.active_for(&session).cloned() {
            Some(active) if self.terminal.live.as_ref().is_none_or(|l| l.id != active.id) => {
                self.terminal.error = None;
                // The tab that was showing stays connected; coming back to one that was shown before takes it from there (13.51).
                if let Some(was) = self.terminal.live.take() {
                    self.terminal.parked.push(was);
                }
                match self.terminal.parked.iter().position(|l| l.id == active.id) {
                    Some(at) => {
                        let back = self.terminal.parked.swap_remove(at);
                        if let (Some(fit), Some(socket)) = (self.terminal.fitted, &back.socket)
                            && back.term.size() != fit
                        {
                            back.term.resize(fit.0, fit.1);
                            socket.resize(fit.0, fit.1);
                        }
                        self.terminal.live = Some(back);
                    }
                    None => self.attach_terminal(&session, &active),
                }
                changed = true;
            }
            Some(_) => {}
            None => {
                changed |= self.terminal.live.take().is_some();
                self.terminal.parked.clear();
                if self.terminals.is_loaded(&session) && !self.terminal.starting && self.terminal.error.is_none() {
                    self.open_new_terminal();
                    changed = true;
                }
            }
        }
        // A view whose tab went (closed here, or in another window) lets go of its socket.
        let listed = self.terminals.terminals_for(&session);
        let before = self.terminal.parked.len();
        self.terminal.parked.retain(|l| listed.iter().any(|t| t.id == l.id));
        changed | (before != self.terminal.parked.len())
    }

    /// The box was laid out at `w` × `h`: fit the terminal to it and tell the shell (13.2, 13.4, 13.5).
    pub fn fit_terminal(&mut self, w: f32, h: f32) {
        let fit = fit_box(w, h);
        if self.terminal.fitted == Some(fit) {
            return;
        }
        self.terminal.fitted = Some(fit);
        // A replayed stream (a scene) keeps the size it was recorded at.
        if let Some((live, socket)) = self.terminal.live.as_ref().and_then(|l| l.socket.as_ref().map(|s| (l, s))) {
            live.term.resize(fit.0, fit.1);
            socket.resize(fit.0, fit.1);
        } else if let Some(live) = self.terminal.live.as_ref().filter(|l| l.pretend && !self.terminal.replaying) {
            live.term.resize(fit.0, fit.1);
        }
    }

    /// Bytes for the shell: typed keys and pastes. A pretend shell (no server) answers into the grid itself.
    pub fn terminal_write(&mut self, bytes: Vec<u8>) {
        let Some(live) = self.terminal.live.as_ref() else { return };
        if let Some(socket) = &live.socket {
            socket.write(bytes);
        } else if live.pretend {
            live.term.feed(&self.terminal.shells.open(&live.id).write(&bytes));
        }
    }

    /// The tab ends (13.34): its shell ended, so the server has nothing left to close. It goes from the list, the neighbour
    /// shows, and the drawer closes when it was the last.
    pub fn end_terminal(&mut self) {
        self.terminal.replaying = false;
        let (Some(session), Some(id)) = (self.terminal.session.clone(), self.terminal.live.take().map(|l| l.id)) else {
            self.terminal.focused = false;
            return;
        };
        self.terminals.remove(&session, &id);
        if self.terminals.terminals_for(&session).is_empty() {
            self.terminals.set_open(&session, false);
            self.terminal.focused = false;
        }
        self.persist_terminals();
    }

    /// Once a frame: the slot follows the tabs, the shell that ended takes its tab, and whether the box has the keyboard (the
    /// cursor is a block while it does, an outline when not). True when something on screen changed.
    pub fn keep_terminal(&mut self, tx: &mut Tx) -> bool {
        let mut changed = self.sync_terminal();
        // A shell that ended behind the tab showing takes its tab too (`onEnded` of a view that isn't the active one).
        if let Some(session) = self.terminal.session.clone() {
            let ended: Vec<String> = self
                .terminal
                .parked
                .iter()
                .filter(|l| l.socket.as_ref().is_some_and(|s| matches!(s.status(), Status::Ended(_))))
                .map(|l| l.id.clone())
                .collect();
            for id in ended {
                self.terminal.parked.retain(|l| l.id != id);
                self.terminals.remove(&session, &id);
                changed = true;
            }
        }
        let pretend_ended = match self.terminal.live.as_ref().filter(|l| l.pretend) {
            Some(live) => self.terminal.shells.open(&live.id).exit_code().is_some(),
            None => false,
        };
        if pretend_ended || matches!(self.terminal.status(), Some(Status::Ended(_))) {
            self.end_terminal();
            changed = true;
        }
        let focused = self.terminal_open() && tx.focused().is_some_and(|k| k == VIEW_KEY);
        let was = std::mem::replace(&mut self.terminal.focused, focused);
        if focused && !was {
            // One shell has one size: another client that attached since may have resized it for everyone, so whoever takes
            // the keyboard says its own size again (`onFocus`).
            if let Some((live, socket)) = self.terminal.live.as_ref().and_then(|l| l.socket.as_ref().map(|s| (l, s))) {
                let (cols, rows) = live.term.size();
                socket.resize(cols, rows);
            }
        }
        changed | (was != focused)
    }
}
