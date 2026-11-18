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
    /// The size the view's box fits, once it has been laid out.
    pub fitted: Option<(u16, u16)>,
    /// The box has the keyboard.
    pub focused: bool,
    /// The drawer's height for a scene; 0 is the remembered one (`Terminals::height`).
    pub height: f32,
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
        let Some(session) = self.open_id().filter(|_| self.terminals_enabled()) else { return };
        self.follow_terminal_session();
        // Normally the list came when the session opened; ask again if it never did (terminals were switched on since). It is
        // asked before the drawer opens, so an empty answer is not read as "left open on a session with no shells".
        if !self.terminals.is_open(&session) && !self.terminals.is_loaded(&session) {
            self.load_terminals(&session);
        }
        self.terminals.toggle_open(&session);
        self.persist_terminals();
        if self.terminals.is_open(&session) {
            self.terminal.error = None;
            tx.focus(VIEW_KEY, false);
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
        let Some(link) = self.link_for(session) else { return };
        let fit = self.terminal.fitted.unwrap_or(START_SIZE);
        let term = Arc::new(Terminal::new(fit.0, fit.1));
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
    fn sync_terminal(&mut self) -> bool {
        let mut changed = self.follow_terminal_session();
        let Some(session) = self.terminal.session.clone().filter(|_| !self.terminal.replaying) else { return changed };
        if !self.terminals.is_open(&session) || !self.terminals_enabled() {
            return changed;
        }
        match self.terminals.active_for(&session).cloned() {
            Some(active) if self.terminal.live.as_ref().is_none_or(|l| l.id != active.id) => {
                self.terminal.error = None;
                self.attach_terminal(&session, &active);
                changed = true;
            }
            Some(_) => {}
            None => {
                changed |= self.terminal.live.take().is_some();
                if self.terminals.is_loaded(&session) && !self.terminal.starting && self.terminal.error.is_none() {
                    self.open_new_terminal();
                    changed = true;
                }
            }
        }
        changed
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
        let pretend_ended = match self.terminal.live.as_ref().filter(|l| l.pretend) {
            Some(live) => self.terminal.shells.open(&live.id).exit_code().is_some(),
            None => false,
        };
        if pretend_ended || matches!(self.terminal.status(), Some(Status::Ended(_))) {
            self.end_terminal();
            changed = true;
        }
        let focused = self.terminal_open() && tx.focused().is_some_and(|k| k == VIEW_KEY);
        changed | (std::mem::replace(&mut self.terminal.focused, focused) != focused)
    }
}
