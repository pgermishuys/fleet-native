//! Mouse reports, as xterm encodes them (ctlseqs: "Mouse Tracking") and xterm.js sends them (component 13, piece H).
//! Pure: which events a mode wants, and the bytes for one. The view decides when to ask.
use alacritty_terminal::term::TermMode;

/// A button as the program hears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

impl Button {
    fn code(self) -> u8 {
        match self {
            Button::Left => 0,
            Button::Middle => 1,
            Button::Right => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Press(Button),
    Release(Button),
    /// The pointer moved, with this button held (None: no button).
    Motion(Option<Button>),
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub kind: Kind,
    /// The cell, from 0.
    pub col: usize,
    pub row: usize,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// Whether the program asked for the mouse at all (modes 1000, 1002, 1003).
pub fn reporting(mode: TermMode) -> bool {
    mode.intersects(TermMode::MOUSE_MODE)
}

/// Whether this mode wants `kind`: 1000 presses, releases and the wheel; 1002 adds motion with a button held; 1003 all motion.
pub fn wants(mode: TermMode, kind: Kind) -> bool {
    match kind {
        Kind::Motion(Some(_)) => mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION),
        Kind::Motion(None) => mode.contains(TermMode::MOUSE_MOTION),
        _ => reporting(mode),
    }
}

/// The bytes for a report in the encoding the program chose (SGR 1006, UTF-8 1005, else the default), or None when the mode
/// doesn't want it or the default encoding can't reach the cell (past column or row 222).
pub fn encode(mode: TermMode, r: &Report) -> Option<Vec<u8>> {
    if !wants(mode, r.kind) {
        return None;
    }
    let sgr = mode.contains(TermMode::SGR_MOUSE);
    let (mut code, release) = match r.kind {
        Kind::Press(b) => (b.code(), false),
        // The default encodings can't say which button let go.
        Kind::Release(b) => (if sgr { b.code() } else { 3 }, true),
        Kind::Motion(b) => (b.map_or(3, Button::code) + 32, false),
        Kind::WheelUp => (64, false),
        Kind::WheelDown => (65, false),
    };
    code += 4 * r.shift as u8 + 8 * r.alt as u8 + 16 * r.ctrl as u8;
    let (x, y) = (r.col + 1, r.row + 1);
    if sgr {
        return Some(format!("\x1b[<{code};{x};{y}{}", if release { 'm' } else { 'M' }).into_bytes());
    }
    let mut out = b"\x1b[M".to_vec();
    out.push(32 + code);
    if mode.contains(TermMode::UTF8_MOUSE) {
        // Positions up to 2015, each as the character of that code.
        for v in [x, y] {
            out.extend(char::from_u32(32 + v as u32).filter(|_| v <= 2015)?.to_string().into_bytes());
        }
    } else {
        for v in [x, y] {
            out.push(u8::try_from(32 + v).ok().filter(|&b| b != 0xff)?);
        }
    }
    Some(out)
}

/// The wheel in the alternate screen when no mouse mode is on: `lines` arrow presses (xterm.js sends one per line scrolled),
/// in the form the program's cursor-key mode wants.
pub fn wheel_arrows(up: bool, lines: usize, app_cursor: bool) -> Vec<u8> {
    let one = format!("\x1b{}{}", if app_cursor { 'O' } else { '[' }, if up { 'A' } else { 'B' });
    one.repeat(lines).into_bytes()
}

/// Focus reporting (mode 1004).
pub fn focus(gained: bool) -> &'static [u8] {
    if gained { b"\x1b[I" } else { b"\x1b[O" }
}
