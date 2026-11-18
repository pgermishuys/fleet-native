//! The terminal drawer's scenes (component 13, piece A): a recorded byte stream of an invented project ("lantern", recorded
//! by `tests/fixtures/terminal/record.sh` at 100 × 30) replayed into the terminal, as if the shell had printed it just now.
use crate::State;
use crate::fleet::terminal_socket::{Socket, Status};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stream {
    /// vim with two splits, line numbers and syntax colours (the alternate screen).
    Vim,
    /// `git log --graph` in colour.
    GitLog,
    /// Wide, combining and emoji characters, attributes, 256 colours, true colour and box drawing.
    Unicode,
}

const COLS: u16 = 100;
const ROWS: u16 = 30;

impl Stream {
    fn bytes(self) -> &'static [u8] {
        match self {
            Stream::Vim => include_bytes!("../../tests/fixtures/terminal/vim.bin"),
            Stream::GitLog => include_bytes!("../../tests/fixtures/terminal/git-log.bin"),
            Stream::Unicode => include_bytes!("../../tests/fixtures/terminal/unicode.bin"),
        }
    }
}

impl State {
    /// The terminal open on `stream`, in a box tall enough for all 30 rows it was recorded at.
    pub fn show_terminal(&mut self, stream: Stream, dark: bool) {
        self.dark = Some(dark);
        // The box: its rule, the padding above and below, and the rows.
        self.terminal.height = 1.0 + 10.0 + ROWS as f32 * 19.0;
        self.terminal.replay(COLS, ROWS, stream.bytes());
    }

    /// The same, with the connection in `status` (13.29, 13.30): Reconnecting… over the rows, or the failed panel.
    pub fn show_terminal_status(&mut self, stream: Stream, dark: bool, status: Status) {
        self.show_terminal(stream, dark);
        if let Some(live) = self.terminal.live.as_mut() {
            live.socket = Some(Socket::fixed(status));
        }
    }
}

impl State {
    /// The pretend shell of the app with no server (`mock::terminal`), showing what it printed when it started.
    pub fn show_terminal_mock(&mut self, dark: bool) {
        self.dark = Some(dark);
        let (cols, rows) = (100, 14);
        let replay = self.terminal.shells.open("mock-1").replay().to_vec();
        self.terminal.replay(cols, rows, &replay);
        self.terminal.height = 1.0 + 10.0 + rows as f32 * 19.0;
        if let Some(live) = self.terminal.live.as_mut() {
            live.id = "mock-1".into();
            live.pretend = true;
        }
    }
}

/// The drawer's states (component 13, piece D), each drawn from the pretend shell of the app with no server.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drawer {
    One,
    Two,
    Starting,
    Error,
    Long,
    /// Shells running with the drawer hidden: the header button's dot.
    HiddenDot,
}

impl State {
    pub fn show_drawer(&mut self, kind: Drawer, dark: bool) {
        use crate::fleet::http::Terminal;
        self.dark = Some(dark);
        let at = self.open_at();
        if self.open_id().is_none() {
            self.set_session_id(at, "scene-session");
        }
        let Some(id) = self.open_id() else { return };
        self.terminals_allowed = Some(true);
        self.terminal.session = Some(id.clone());
        let dir = if kind == Drawer::Long {
            "/home/dev/projects/glow-and-wick-studios/lantern-monorepo/packages/glow-renderer/src/shaders"
        } else {
            "/home/dev/lantern"
        };
        self.projects[at.0].sessions[at.1].listing.workspace_directory = dir.into();
        let tab = |n: usize, title: &str| Terminal { id: format!("mock-{n}"), title: title.into(), running: true };
        let tabs = match kind {
            Drawer::One | Drawer::HiddenDot => vec![tab(1, "zsh")],
            Drawer::Two => vec![tab(1, "zsh"), tab(2, "dev server")],
            Drawer::Starting | Drawer::Error => vec![],
            Drawer::Long => (1..=9)
                .map(|n| tab(n, &if n == 3 { "a build shell with a title that goes on and on".to_string() } else { format!("zsh {n}") }))
                .collect(),
        };
        self.terminals.set_terminals(&id, tabs);
        if kind == Drawer::Two {
            self.terminals.set_active(&id, "mock-2");
        }
        self.terminal.starting = kind == Drawer::Starting;
        if kind == Drawer::Error {
            self.terminal.error = Some("The session's folder /home/dev/lantern doesn't exist any more.".into());
        }
        self.terminals.set_open(&id, kind != Drawer::HiddenDot);
        if kind == Drawer::Two {
            // The second tab is a dev server that has been running for a while; the first one was shown before.
            let shell = self.terminal.shells.open("mock-2");
            let _ = shell.write(b"npm run dev\r");
        }
    }
}
