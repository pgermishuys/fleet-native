//! The open session's terminal, end to end (component 13, piece A): the first of its shells is listed or started, its
//! socket is opened, and the grid reads what the shell printed. One terminal, no tabs yet (piece C keeps the per-session
//! list, piece D draws the drawer around it).
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
const START_SIZE: (u16, u16) = (120, 14);

/// The key of the terminal's focusable box.
pub const VIEW_KEY: &str = "terminal-view";

/// One attached shell: what the socket feeds, what the view reads, and the socket itself.
pub struct Live {
    pub id: String,
    pub title: String,
    pub term: Arc<Terminal>,
    pub screen: RefCell<Screen>,
    pub socket: Option<Socket>,
}

#[derive(Default)]
pub struct Slot {
    /// The session this belongs to; None for the invented data's scenes.
    pub session: Option<String>,
    pub open: bool,
    /// The list or the create call is out.
    pub starting: bool,
    /// Why no shell started, in the server's words.
    pub error: Option<String>,
    pub live: Option<Live>,
    /// The size the view's box fits, once it has been laid out.
    pub fitted: Option<(u16, u16)>,
    /// The box has the keyboard.
    pub focused: bool,
    /// The drawer's height; 0 is `TerminalDrawer.vue`'s 272. A scene of a taller screen sets it.
    pub height: f32,
}

impl Slot {
    /// A terminal fed by `bytes` instead of a socket: the invented data's scenes and the tests.
    pub fn replay(&mut self, cols: u16, rows: u16, bytes: &[u8]) {
        let term = Arc::new(Terminal::new(cols, rows));
        term.feed(bytes);
        self.live = Some(Live { id: "replay".into(), title: "zsh".into(), term, screen: RefCell::default(), socket: None });
        self.fitted = Some((cols, rows));
        self.open = true;
    }

    pub fn status(&self) -> Option<Status> {
        self.live.as_ref().and_then(|l| l.socket.as_ref()).map(Socket::status)
    }
}

impl State {
    /// Ctrl/Cmd+J and the header's terminal button (`toggle-terminal`): shows or hides the open session's terminal.
    pub fn toggle_terminal(&mut self, tx: &mut Tx) {
        let open = self.open_id();
        if open.is_none() && self.terminal.session.is_none() && self.terminal.live.is_none() {
            return;
        }
        // Another session's shell isn't this one's.
        if self.terminal.session != open {
            self.terminal = Default::default();
            self.terminal.session = open;
        }
        self.terminal.open = !self.terminal.open;
        if !self.terminal.open {
            return;
        }
        tx.focus(VIEW_KEY, false);
        if self.terminal.live.is_none() && !self.terminal.starting {
            self.start_terminal();
        }
    }

    /// Lists the session's terminals and uses the first, else starts one (`POST {cols, rows}`), then attaches.
    fn start_terminal(&mut self) {
        let (Some(session), Some(link)) =
            (self.terminal.session.clone(), self.terminal.session.as_deref().and_then(|id| self.link_for(id)))
        else {
            return;
        };
        self.terminal.starting = true;
        self.terminal.error = None;
        let size = self.terminal.fitted.unwrap_or(START_SIZE);
        let (s, post, base) = (session.clone(), link.post(), link.base.clone());
        link.off_thread(
            move |base| match http::terminals(base, &s)? {
                list if !list.is_empty() => Ok(list.into_iter().next().expect("one terminal")),
                _ => http::terminal_create(base, &s, size.0 as u32, size.1 as u32),
            },
            move |state: &mut State, _, result: Result<Listed, String>| {
                if state.terminal.session.as_deref() != Some(session.as_str()) {
                    return;
                }
                state.terminal.starting = false;
                match result {
                    Ok(listed) => {
                        let fit = state.terminal.fitted.unwrap_or(START_SIZE);
                        let term = Arc::new(Terminal::new(fit.0, fit.1));
                        let path = format!("/api/sessions/{}/terminals/{}", http::escape(&session), http::escape(&listed.id));
                        let changed = post.clone();
                        let socket = Socket::open(&base, &path, fit, term.clone(), move || changed.update(|_, _| {}));
                        state.terminal.live =
                            Some(Live { id: listed.id, title: listed.title, term, screen: RefCell::default(), socket: Some(socket) });
                    }
                    Err(why) => state.terminal.error = Some(why),
                }
            },
        );
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

    /// Bytes for the shell: typed keys and pastes.
    pub fn terminal_write(&self, bytes: Vec<u8>) {
        if let Some(socket) = self.terminal.live.as_ref().and_then(|l| l.socket.as_ref()) {
            socket.write(bytes);
        }
    }

    /// The tab ends (13.34): the shell ended, or "Close tab" on the failed panel. The socket is dropped (a normal close) and the
    /// drawer hides, as it does when a session's last terminal goes. Closing it on the server is piece C's.
    pub fn end_terminal(&mut self) {
        self.terminal.live = None;
        self.terminal.open = false;
        self.terminal.focused = false;
    }

    /// Once a frame: whether the box has the keyboard (the cursor is a block while it does, an outline when not), and
    /// whether the shell ended (the tab goes).
    pub fn keep_terminal(&mut self, tx: &mut Tx) -> bool {
        if matches!(self.terminal.status(), Some(Status::Ended(_))) {
            self.end_terminal();
            return true;
        }
        let focused = self.terminal.open && tx.focused().is_some_and(|k| k == VIEW_KEY);
        std::mem::replace(&mut self.terminal.focused, focused) != focused
    }
}
