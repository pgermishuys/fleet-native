//! Ports `TerminalView.vue`'s box (component 13, piece A): the terminal's grid in its padded box, the block cursor, the keys.
//! The drawer around it (tabs, header, grip) is piece D's; here it is a plain 272 px box under the composer
//! (`TerminalDrawer.vue`'s default height).
use rune::{El, Key, KeyEvent, Tx, col, el, grid, text};

use crate::State;
use crate::fleet::terminal_slot::VIEW_KEY;
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::palette::Palette;
use crate::term::screen::style;
use crate::theme::Theme;

/// `TerminalDrawer.vue`'s default height.
pub const HEIGHT: f32 = 272.0;

/// The drawer's box, or nothing while no terminal is shown for the open session.
pub fn drawer(s: &State, t: &Theme) -> Option<El> {
    let slot = &s.terminal;
    if !slot.open || slot.session != s.open_id() {
        return None;
    }
    Some(
        col()
            .key("terminal-drawer")
            .h(if slot.height > 0.0 { slot.height } else { HEIGHT })
            .fill(t.terminal_bg())
            .child(el().h(1.0).fill(t.border))
            .child(body(s, t)),
    )
}

fn body(s: &State, t: &Theme) -> El {
    let slot = &s.terminal;
    let host = col()
        .key(VIEW_KEY)
        .grow(1.0)
        .min_h(0.0)
        .min_w(0.0)
        .clip_rounded(0.0)
        .pt(PAD[0])
        .pr(PAD[1])
        .pb(PAD[2])
        .pl(PAD[3])
        .focusable(true)
        .no_focus_ring()
        .on_layout(|s: &mut State, [_, _, w, h]| s.fit_terminal(w, h))
        .on_key(on_key)
        .on_paste(|s: &mut State, text: &str, _| paste(s, text));
    let Some(live) = &slot.live else {
        let words = match (&slot.error, slot.starting) {
            (Some(why), _) => why.clone(),
            _ => "Starting a shell\u{2026}".to_string(),
        };
        return host.child(text(words).font(crate::theme::sans(12.5, 400.0, 1.4)).color(t.muted));
    };
    let pal = Palette::new(t);
    let look = look_of(t);
    {
        let mut screen = live.screen.borrow_mut();
        live.term.read(|term| screen.read(term, &pal, t.terminal_bg(), look));
    }
    let screen = live.screen.borrow();
    let (cols, rows) = (screen.cols, screen.rows.len());
    let mut stack = col()
        .w(cols as f32 * CELL_W)
        .h(rows as f32 * CELL_H)
        .child(grid(cols as u16, CELL_W, CELL_H, screen.rows.clone()).w(cols as f32 * CELL_W).h(rows as f32 * CELL_H));
    if let Some(c) = &screen.cursor {
        let w = if c.wide { 2.0 * CELL_W } else { CELL_W };
        let at = |e: El| e.absolute(c.col as f32 * CELL_W, c.line as f32 * CELL_H);
        // A block while the box has the keyboard, an outline when not (xterm's `cursorBlink` and inactive cursor).
        stack = stack.child(if slot.focused {
            at(el().key("terminal-cursor").size(w, CELL_H).fill(pal.fg).child(text(c.ch.clone()).font(style()).color(pal.cursor_ink)))
        } else {
            at(el().key("terminal-cursor").size(w, CELL_H).border(1.0, pal.fg).border_layout())
        });
    }
    host.child(stack)
}

/// What the rows were built with: another theme builds them again.
fn look_of(t: &Theme) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    for c in [t.text, t.panel_bg, t.error, t.running, t.idle, t.complete, t.queued, t.accent] {
        [c.r, c.g, c.b].map(f32::to_bits).hash(&mut h);
    }
    h.finish()
}

/// Everything typed goes to the shell and never to Fleet's own shortcuts, except the two Fleet passes through: Mod+J
/// (toggle the terminal) and Mod+Shift+B (toggle the right panel) (`terminalKeyOwner`).
fn on_key(s: &mut State, ev: &KeyEvent, _tx: &mut Tx) -> bool {
    if ev.is_command('j') || (ev.is_command('b') && ev.mods.shift) {
        return false;
    }
    let app_cursor = s.terminal.live.as_ref().is_some_and(|l| l.term.mode().contains(crate::term::APP_CURSOR));
    match crate::term::keys::encode(ev, app_cursor) {
        Some(bytes) => {
            s.terminal_write(bytes);
            true
        }
        // A key that makes no bytes (a lone modifier) is nobody's.
        None => matches!(ev.key, Key::Modifier { .. }),
    }
}

fn paste(s: &mut State, text: &str) {
    // Newlines become `\r`, as a terminal's paste does; bracketed when the program asked for it.
    let body = text.replace("\r\n", "\r").replace('\n', "\r");
    let bracketed = s.terminal.live.as_ref().is_some_and(|l| l.term.mode().contains(crate::term::BRACKETED_PASTE));
    let bytes = if bracketed { [&b"\x1b[200~"[..], body.as_bytes(), b"\x1b[201~"].concat() } else { body.into_bytes() };
    s.terminal_write(bytes);
}
