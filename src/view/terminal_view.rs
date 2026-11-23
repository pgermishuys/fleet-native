//! Ports `TerminalView.vue`'s box (component 13, piece A): the terminal's grid in its padded box, the block cursor, the keys.
//! The drawer around it (tabs, header) is `terminal_drawer`.
use alacritty_terminal::vte::ansi::CursorShape;
use rune::{Cursor, El, Key, KeyEvent, Tx, col, el, grid, row, text};

use crate::State;
use crate::fleet::terminal_keys::{Owner, PASS_THROUGH, terminal_key_owner};
use crate::fleet::terminal_slot::VIEW_KEY;
use crate::fleet::terminal_socket::Status;
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::palette::Palette;
use crate::term::screen::style;
use crate::theme::Theme;

/// `TerminalView.vue`'s box: the grid and, over it, the "Reconnecting…" notice or the failed panel.
pub fn area(s: &State, t: &Theme) -> El {
    let mut area = col()
        .grow(1.0)
        .min_h(0.0)
        .min_w(0.0)
        .on_hover_change(|s: &mut State, inside: bool, _: &mut Tx| s.terminal_hover(inside))
        .child(body(s, t));
    area = area.children(super::terminal_bar::bar(s, t)).children(super::terminal_pop::pop(s, t));
    match s.terminal.status() {
        Some(Status::Reconnecting) => area = area.child(notice(t)),
        Some(Status::Failed) => area = area.child(failed(t)),
        _ => {}
    }
    area
}

/// `.terminal-view__notice`: top 8, right 12, `--idle` at 14 % behind `--idle-text`; it takes no clicks.
fn notice(t: &Theme) -> El {
    row().key("terminal-notice").absolute(0.0, 0.0).w_full().justify_end().pt(8.0).pr(12.0).child(
        el().px(8.0)
            .py(2.0)
            .radius(6.0)
            .fill(t.idle.alpha(0.14))
            .child(text("Reconnecting\u{2026}").font(crate::theme::sans(11.5, 500.0, 1.4)).color(t.idle_text)),
    )
}

/// `.terminal-view__failed`: over the view, `--panel-bg` at 85 %, the words and Close tab.
fn failed(t: &Theme) -> El {
    let button = el()
        .key("terminal-close-tab")
        .h(28.0)
        .px(12.0)
        .center()
        .radius(crate::theme::RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.panel_bg)
        .hover(t.mix(t.panel_bg, t.text, 0.95))
        .cursor(Cursor::Pointer)
        .label("Close tab")
        .on_press(|s: &mut State| s.close_showing_terminal())
        .child(text("Close tab").font(crate::theme::sans(12.5, 400.0, 1.4)).color(t.text));
    col()
        .key("terminal-failed")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .center()
        .gap(10.0)
        .px(16.0)
        .py(16.0)
        .fill(t.panel_bg.alpha(0.85))
        .child(
            text("This terminal can't be reached. Its shell may have ended while Fleet was away.")
                .font(crate::theme::sans(12.5, 400.0, 1.4))
                .color(t.muted)
                .max_w(320.0)
                .align(rune::Align::Center)
                .wrap(),
        )
        .child(button)
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
        .cursor(if slot.link.is_some() { Cursor::Pointer } else { Cursor::Default })
        .on_layout(|s: &mut State, [_, _, w, h]| s.fit_terminal(w, h))
        .on_key(on_key)
        .on_pointer_move_local(|s: &mut State, ev: rune::PointerEvent, _: &mut Tx| s.terminal_pointer(ev.x, ev.y))
        // The middle button reports to a program that asked for the mouse (the secondary one is `on_secondary_press_local`).
        .pointer_buttons([rune::PointerButton::Middle])
        .on_drag(|s: &mut State, d: &mut rune::Drag, tx| s.terminal_drag(d, tx))
        .on_secondary_press_local(|s: &mut State, ev: rune::PointerEvent, _| s.report_right_click(ev.x, ev.y))
        // No `on_paste` here: Rune only calls one on the root or an open modal, never on a focused element. Pasting is the
        // paste key in `on_key` (Fleet has no other way to paste into the terminal), which reads the clipboard itself.
        .on_wheel(|s: &mut State, e: rune::WheelEvent, _: &mut Tx| s.terminal_wheel(&e));
    let Some(live) = &slot.live else {
        // Nothing to draw: the drawer says why ("Starting a shell…" or the error) over this box.
        return host;
    };
    let pal = Palette::new(t);
    let look = look_of(t);
    {
        let mut screen = live.screen.borrow_mut();
        // The selection is `--selection` while the box has the keyboard and a greyer one when it doesn't (13.8).
        screen.selection = Some(if slot.focused { pal.selection } else { pal.selection_inactive });
        live.term.read_damage(|term| screen.read(term, &pal, t.terminal_bg(), look));
    }
    let screen = live.screen.borrow();
    let (cols, rows) = (screen.cols, screen.rows.len());
    let mut stack = col()
        .w(cols as f32 * CELL_W)
        .h(rows as f32 * CELL_H)
        .child(grid(cols as u16, CELL_W, CELL_H, screen.rows.clone()).w(cols as f32 * CELL_W).h(rows as f32 * CELL_H));
    if let Some(link) = &s.terminal.link {
        stack = links(stack, link, &pal);
    }
    if let Some(c) = &screen.cursor {
        // The input method (dead keys, CJK, the emoji picker) opens at the cursor while the box has the keyboard.
        if slot.focused {
            stack = stack
                .child(el().key("terminal-ime").absolute(c.col as f32 * CELL_W, c.line as f32 * CELL_H).size(CELL_W, CELL_H).ime_cursor());
        }
        let w = if c.wide { 2.0 * CELL_W } else { CELL_W };
        let at = |e: El| e.absolute(c.col as f32 * CELL_W, c.line as f32 * CELL_H);
        let outline = || at(el().key("terminal-cursor").size(w, CELL_H).border(1.0, pal.fg).border_layout());
        // Focused: the shape the program asked for, lit or dark with the blink. Not focused: an outline of the cell, steady
        // (xterm's `cursorInactiveStyle`).
        stack = if !slot.focused {
            stack.child(outline())
        } else if slot.blink_off && c.blinking {
            stack
        } else {
            stack.child(match c.shape {
                CursorShape::Beam => at(el().key("terminal-cursor").size(CURSOR_THIN, CELL_H).fill(pal.fg)),
                CursorShape::Underline => el()
                    .key("terminal-cursor")
                    .absolute(c.col as f32 * CELL_W, c.line as f32 * CELL_H + CELL_H - CURSOR_THIN)
                    .size(w, CURSOR_THIN)
                    .fill(pal.fg),
                CursorShape::HollowBlock => outline(),
                _ => at(el()
                    .key("terminal-cursor")
                    .size(w, CELL_H)
                    .fill(pal.fg)
                    .child(text(c.ch.clone()).font(style()).color(pal.cursor_ink))),
            })
        };
    }
    host.child(stack)
}

/// The bar and underline cursors are one pixel thick (xterm's `cursorWidth`).
const CURSOR_THIN: f32 = 1.0;

/// The link under the pointer: its cells get a hand and a line under them, and a click opens it. The line is in the text's
/// colour. The layer takes no pointer: a press on a link starts a selection as anywhere, and a click that didn't move opens it
/// (`terminal_drag`); the hand is the box's own cursor while a link is under the pointer.
fn links(mut stack: El, link: &crate::term::links::Link, pal: &Palette) -> El {
    for (n, &(line, from, to)) in link.spans.iter().enumerate() {
        let w = (to - from) as f32 * CELL_W;
        stack = stack.child(
            el().key(format!("terminal-link-{n}"))
                .absolute(from as f32 * CELL_W, line as f32 * CELL_H)
                .size(w, CELL_H)
                .inert()
                .child(el().absolute(0.0, UNDERLINE_AT).size(w, 1.0).fill(pal.fg)),
        );
    }
    stack
}

/// Where a link's line sits in its row, as an underline does.
const UNDERLINE_AT: f32 = 16.0;

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
/// (toggle the terminal) and Mod+Shift+B (toggle the right panel) (`terminalKeyOwner`); and the copy and paste keys.
fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match terminal_key_owner(ev, cfg!(target_os = "macos"), &PASS_THROUGH) {
        Owner::Fleet => return false,
        Owner::Copy => {
            s.copy_terminal_selection(tx);
            return true;
        }
        Owner::Paste => {
            tx.read_clipboard(|s: &mut State, content, _| {
                if !content.text().is_empty() {
                    paste(s, content.text());
                }
            });
            return true;
        }
        Owner::Shell => {}
    }
    // Shift+PageUp and Shift+PageDown scroll the view and are not sent (xterm's `scrollPages`).
    if ev.mods.shift && !ev.mods.ctrl && !ev.mods.alt && matches!(ev.key, Key::PageUp | Key::PageDown) {
        if let Some(live) = s.terminal.live.as_ref() {
            live.term.scroll_page(ev.key == Key::PageUp);
        }
        return true;
    }
    let app_cursor = s.terminal.live.as_ref().is_some_and(|l| l.term.mode().contains(crate::term::APP_CURSOR));
    if let Some(bytes) = crate::term::keys::encode(ev, app_cursor) {
        if let Some(live) = s.terminal.live.as_ref() {
            live.term.scroll_to_bottom();
        }
        s.terminal_write(bytes);
    }
    // A key that makes no bytes (a lone modifier, a Command shortcut on a Mac) is nobody's: Fleet never sees it.
    true
}

fn paste(s: &mut State, text: &str) {
    let bracketed = s.terminal.live.as_ref().is_some_and(|l| l.term.mode().contains(crate::term::BRACKETED_PASTE));
    s.terminal_write(crate::term::keys::paste_bytes(text, bracketed));
}
