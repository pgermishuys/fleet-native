//! The terminal under a harness's setup row (component 13, piece I; rows 13.80 to 13.82 of `docs/parity/13-terminal.md`). Fleet
//! hosts `TerminalView` there outside any session: base path `/api/setup/terminals`, session id `setup`, with the installer's
//! command as `initialInput` and `ended` closing it. Here the one terminal slot (`terminal_slot.rs`) holds it: the setup rows
//! show in a window of their own over the app, so only one terminal is on screen at a time. While it is held the session's
//! drawer isn't drawn and its views are dropped (their sockets close; the shells keep running and replay when the drawer is
//! back), which is the one difference from Fleet, where both stay mounted.
use std::cell::RefCell;
use std::sync::Arc;

use rune::Tx;

use super::harness_setup_rows::{TERMINAL_COLS, TERMINAL_ROWS};
use super::http;
use super::settings::HOME_MACHINE_KEY;
use super::terminal_slot::{Live, Slot, VIEW_KEY};
use super::terminal_socket::{Socket, Status};
use crate::State;
use crate::term::Terminal;

/// `SETUP_TERMINALS_PATH`.
pub const SETUP_TERMINALS_PATH: &str = "/api/setup/terminals";

impl State {
    /// The shell was made: its view mounts (`v-if="active.terminalId"`), connects at 100 × 14 until it is measured, types
    /// `command` once on the first `ready`, and takes the keyboard (`:shown="true"`).
    pub(super) fn attach_setup_terminal(&mut self, id: &str, command: &str, tx: &mut Tx) {
        let Some(link) = self.link_for_machine(HOME_MACHINE_KEY) else { return };
        let size = (TERMINAL_COLS as u16, TERMINAL_ROWS as u16);
        let term = Arc::new(Terminal::new(size.0, size.1));
        let path = format!("{SETUP_TERMINALS_PATH}/{}", http::escape(id));
        let changed = link.post();
        let socket = Socket::open_typing(&link.base, &path, size, command, term.clone(), move || changed.update(|_, _| {}));
        self.terminal = Slot {
            setup: Some(id.to_string()),
            live: Some(Live {
                id: id.to_string(),
                title: "setup".into(),
                term,
                screen: RefCell::default(),
                socket: Some(socket),
                pretend: false,
            }),
            ..Default::default()
        };
        tx.focus(VIEW_KEY, false);
    }

    /// The view unmounts: its socket closes. The session's drawer takes the slot again on the next frame.
    pub(super) fn release_setup_terminal(&mut self) {
        if self.terminal.setup.is_some() {
            self.terminal = Slot::default();
        }
    }

    /// Whether the slot holds a setup terminal rather than a session's.
    pub fn setup_terminal_shown(&self) -> bool {
        self.terminal.setup.is_some()
    }

    /// The view's `@ended`: the shell ended (its `exit`, or the server's normal close), so the setup terminal closes.
    pub(super) fn keep_setup_terminal(&mut self) -> bool {
        if self.terminal.setup.is_some() && matches!(self.terminal.status(), Some(Status::Ended(_))) {
            self.close_setup_terminal();
            return true;
        }
        false
    }
}
