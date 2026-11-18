//! The terminal drawer's scenes (component 13, piece A): a recorded byte stream of an invented project ("lantern", recorded
//! by `tests/fixtures/terminal/record.sh` at 100 × 30) replayed into the terminal, as if the shell had printed it just now.
use crate::State;

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
}
