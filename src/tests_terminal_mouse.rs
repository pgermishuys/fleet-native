//! Component 13, piece H (rows 13.64, 13.77, 13.78): a program that asks for the mouse hears it, the wheel in the alternate
//! screen sends arrows, focus is reported, and an input method's text reaches the shell. The bytes of each mode are checked
//! against `term::mouse`; the view's wiring against what `terminal_write` was given.
use crate::term::Terminal;
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::mouse::{Button, Kind, Report, encode, focus, wheel_arrows};
use crate::tests::FRAME;
use rune::{Ime, Mods, Runtime};

fn report(kind: Kind, col: usize, row: usize) -> Report {
    Report { kind, col, row, shift: false, alt: false, ctrl: false }
}

/// The modes a program gets by sending `escapes`.
fn asked(escapes: &[u8]) -> alacritty_terminal::term::TermMode {
    let t = Terminal::new(80, 24);
    t.feed(escapes);
    t.mode()
}

// ---- the bytes -----------------------------------------------------------------------------------------------------------------

#[test]
fn modes_1000_1002_and_1003_want_presses_then_drags_then_all_motion() {
    let (press, drag, motion) = (Kind::Press(Button::Left), Kind::Motion(Some(Button::Left)), Kind::Motion(None));
    let none = asked(b"");
    assert!(encode(none, &report(press, 0, 0)).is_none(), "no mode, no report");
    let m1000 = asked(b"\x1b[?1000h");
    assert!(encode(m1000, &report(press, 0, 0)).is_some());
    assert!(encode(m1000, &report(Kind::Release(Button::Left), 0, 0)).is_some());
    assert!(encode(m1000, &report(Kind::WheelUp, 0, 0)).is_some());
    assert!(encode(m1000, &report(drag, 0, 0)).is_none(), "1000 hears no motion");
    let m1002 = asked(b"\x1b[?1002h");
    assert!(encode(m1002, &report(drag, 0, 0)).is_some());
    assert!(encode(m1002, &report(motion, 0, 0)).is_none(), "1002 hears no motion without a button");
    let m1003 = asked(b"\x1b[?1003h");
    assert!(encode(m1003, &report(motion, 0, 0)).is_some());
    // Asking again with another mode replaces the first; asking to stop stops.
    assert!(encode(asked(b"\x1b[?1000h\x1b[?1003h\x1b[?1003l"), &report(press, 0, 0)).is_none());
}

#[test]
fn the_default_encoding_adds_32_to_the_button_and_the_cell() {
    let m = asked(b"\x1b[?1000h");
    assert_eq!(encode(m, &report(Kind::Press(Button::Left), 0, 0)).unwrap(), b"\x1b[M !!");
    assert_eq!(encode(m, &report(Kind::Press(Button::Middle), 9, 4)).unwrap(), b"\x1b[M!*%");
    assert_eq!(encode(m, &report(Kind::Press(Button::Right), 0, 0)).unwrap(), b"\x1b[M\"!!");
    // A release can't say which button: 3.
    assert_eq!(encode(m, &report(Kind::Release(Button::Right), 1, 1)).unwrap(), b"\x1b[M#\"\"");
    assert_eq!(encode(m, &report(Kind::WheelUp, 0, 0)).unwrap(), b"\x1b[M`!!");
    assert_eq!(encode(m, &report(Kind::WheelDown, 0, 0)).unwrap(), b"\x1b[Ma!!");
    // A drag adds 32 to the held button, motion with none held is 3 + 32.
    let m = asked(b"\x1b[?1003h");
    assert_eq!(encode(m, &report(Kind::Motion(Some(Button::Left)), 2, 0)).unwrap(), b"\x1b[M@#!");
    assert_eq!(encode(m, &report(Kind::Motion(None), 2, 0)).unwrap(), b"\x1b[MC#!");
}

#[test]
fn modifiers_add_4_for_shift_8_for_alt_and_16_for_ctrl() {
    let m = asked(b"\x1b[?1000h\x1b[?1006h");
    let with = |shift, alt, ctrl| Report { shift, alt, ctrl, ..report(Kind::Press(Button::Left), 0, 0) };
    assert_eq!(encode(m, &with(true, false, false)).unwrap(), b"\x1b[<4;1;1M");
    assert_eq!(encode(m, &with(false, true, false)).unwrap(), b"\x1b[<8;1;1M");
    assert_eq!(encode(m, &with(false, false, true)).unwrap(), b"\x1b[<16;1;1M");
    assert_eq!(encode(m, &with(true, true, true)).unwrap(), b"\x1b[<28;1;1M");
}

#[test]
fn sgr_1006_names_the_cell_in_digits_and_a_release_is_a_lower_case_m_with_its_button() {
    let m = asked(b"\x1b[?1002h\x1b[?1006h");
    assert_eq!(encode(m, &report(Kind::Press(Button::Left), 119, 29)).unwrap(), b"\x1b[<0;120;30M");
    assert_eq!(encode(m, &report(Kind::Release(Button::Left), 119, 29)).unwrap(), b"\x1b[<0;120;30m");
    assert_eq!(encode(m, &report(Kind::Release(Button::Right), 0, 0)).unwrap(), b"\x1b[<2;1;1m");
    assert_eq!(encode(m, &report(Kind::Motion(Some(Button::Middle)), 4, 2)).unwrap(), b"\x1b[<33;5;3M");
    assert_eq!(encode(m, &report(Kind::WheelDown, 0, 0)).unwrap(), b"\x1b[<65;1;1M");
    // Past column 223 the default encoding runs out, SGR does not.
    assert_eq!(encode(m, &report(Kind::Press(Button::Left), 299, 0)).unwrap(), b"\x1b[<0;300;1M");
}

#[test]
fn the_default_encoding_stops_at_column_222_and_utf8_1005_reaches_2015() {
    let plain = asked(b"\x1b[?1000h");
    assert!(encode(plain, &report(Kind::Press(Button::Left), 221, 0)).is_some());
    assert!(encode(plain, &report(Kind::Press(Button::Left), 222, 0)).is_none());
    let utf8 = asked(b"\x1b[?1000h\x1b[?1005h");
    assert_eq!(encode(utf8, &report(Kind::Press(Button::Left), 0, 0)).unwrap(), b"\x1b[M !!");
    // Column 300 (x = 301, 32 + 301 = 333) is the two-byte character U+014D.
    assert_eq!(
        encode(utf8, &report(Kind::Press(Button::Left), 300, 0)).unwrap(),
        "\x1b[M !\u{14d}\u{21}".replace("!\u{14d}", "\u{14d}").as_bytes()
    );
    // SGR wins when both are on, as in xterm.
    let both = asked(b"\x1b[?1000h\x1b[?1005h\x1b[?1006h");
    assert_eq!(encode(both, &report(Kind::Press(Button::Left), 0, 0)).unwrap(), b"\x1b[<0;1;1M");
}

#[test]
fn wheel_arrows_follow_the_cursor_key_mode() {
    assert_eq!(wheel_arrows(true, 2, false), b"\x1b[A\x1b[A");
    assert_eq!(wheel_arrows(false, 1, true), b"\x1bOB");
    assert_eq!(focus(true), b"\x1b[I");
    assert_eq!(focus(false), b"\x1b[O");
}

// ---- the box -------------------------------------------------------------------------------------------------------------------

fn shown(bytes: &[u8]) -> Runtime<crate::State> {
    let mut rt = crate::terminal_hooks(rune::App::new(crate::spec())).headless_runtime((1280.0, 760.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    let bytes = bytes.to_vec();
    rt.update(move |s, _| {
        s.set_session_id((0, 0), "s1");
        s.terminal.session = s.open_id();
        s.terminals.add("s1", crate::fleet::http::Terminal { id: "t1".into(), title: "build shell".into(), running: true }, true);
        s.terminal.replay(80, 11, &bytes);
        s.terminal.live.as_mut().unwrap().id = "t1".into();
        s.terminal.height = 60.0 + 1.0 + 10.0 + 11.0 * 19.0;
        s.terminals.set_open("s1", true);
    });
    rt.frame(FRAME);
    rt
}

fn at(rt: &Runtime<crate::State>, col: usize, row: usize) -> (f32, f32) {
    let r = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("terminal-view")).expect("the box").rect;
    (r[0] + PAD[3] + col as f32 * CELL_W + 2.0, r[1] + PAD[0] + row as f32 * CELL_H + CELL_H / 2.0)
}

fn heard(rt: &mut Runtime<crate::State>) -> Vec<u8> {
    rt.frame(FRAME);
    std::mem::take(&mut rt.state.terminal.written)
}

fn click_at(rt: &mut Runtime<crate::State>, col: usize, row: usize) {
    let (x, y) = at(rt, col, row);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
}

#[test]
fn a_press_and_release_reach_a_program_that_asked_for_the_mouse_and_select_nothing() {
    let mut rt = shown(b"\x1b[?1000h\x1b[?1006hhello");
    heard(&mut rt);
    click_at(&mut rt, 3, 2);
    assert_eq!(heard(&mut rt), b"\x1b[<0;4;3M\x1b[<0;4;3m");
    assert!(!rt.state.terminal.live.as_ref().unwrap().term.has_selection());
}

#[test]
fn the_default_encoding_is_used_when_the_program_asked_for_no_other() {
    let mut rt = shown(b"\x1b[?1000h");
    heard(&mut rt);
    click_at(&mut rt, 0, 0);
    assert_eq!(heard(&mut rt), b"\x1b[M !!\x1b[M#!!");
}

#[test]
fn a_drag_reports_each_new_cell_once_in_mode_1002_and_not_in_1000() {
    for (mode, drags) in [(&b"\x1b[?1002h\x1b[?1006h"[..], true), (&b"\x1b[?1000h\x1b[?1006h"[..], false)] {
        let mut rt = shown(mode);
        heard(&mut rt);
        let (x0, y0) = at(&rt, 1, 1);
        rt.pointer_move(x0, y0);
        rt.pointer_down(x0, y0);
        for col in [2, 2, 3] {
            let (x, y) = at(&rt, col, 1);
            rt.pointer_move(x, y);
        }
        let (x, y) = at(&rt, 3, 1);
        rt.pointer_up(x, y);
        let want: &[u8] = if drags { b"\x1b[<0;2;2M\x1b[<32;3;2M\x1b[<32;4;2M\x1b[<0;4;2m" } else { b"\x1b[<0;2;2M\x1b[<0;4;2m" };
        assert_eq!(heard(&mut rt), want, "drags: {drags}");
    }
}

#[test]
fn plain_motion_is_heard_only_in_mode_1003_once_per_cell() {
    let mut rt = shown(b"\x1b[?1003h\x1b[?1006h");
    heard(&mut rt);
    for col in [4, 4, 5] {
        let (x, y) = at(&rt, col, 0);
        rt.pointer_move(x, y);
    }
    assert_eq!(heard(&mut rt), b"\x1b[<35;5;1M\x1b[<35;6;1M");
    let mut rt = shown(b"\x1b[?1002h\x1b[?1006h");
    heard(&mut rt);
    let (x, y) = at(&rt, 4, 0);
    rt.pointer_move(x, y);
    assert_eq!(heard(&mut rt), b"");
}

#[test]
fn shift_takes_the_pointer_back_for_a_selection() {
    let mut rt = shown(b"\x1b[?1000h\x1b[?1006hhello world");
    heard(&mut rt);
    rt.update(|s, _| s.terminal.mods = Mods { shift: true, ..Mods::default() });
    let (x0, y0) = at(&rt, 0, 0);
    let (x1, y1) = at(&rt, 4, 0);
    rt.pointer_move(x0, y0);
    rt.pointer_down(x0, y0);
    rt.pointer_move(x1, y1);
    rt.pointer_up(x1, y1);
    assert_eq!(heard(&mut rt), b"", "the program heard nothing");
    assert!(rt.state.terminal.live.as_ref().unwrap().term.has_selection());
}

#[test]
fn the_wheel_reports_to_a_program_that_asked_and_does_not_scroll_the_view() {
    let mut text = Vec::new();
    for n in 0..40 {
        text.extend(format!("line {n}\r\n").bytes());
    }
    text.extend(b"\x1b[?1000h\x1b[?1006h");
    let mut rt = shown(&text);
    heard(&mut rt);
    let (x, y) = at(&rt, 6, 3);
    rt.wheel_xy(x, y, 0.0, 100.0);
    rt.wheel_xy(x, y, 0.0, -100.0);
    assert_eq!(heard(&mut rt), b"\x1b[<64;7;4M\x1b[<65;7;4M");
    assert_eq!(rt.state.terminal.live.as_ref().unwrap().term.display_offset(), 0);
    // Shift scrolls the view instead.
    rt.update(|s, _| s.terminal.mods = Mods { shift: true, ..Mods::default() });
    rt.wheel_xy(x, y, 0.0, 100.0);
    assert_eq!(heard(&mut rt), b"");
}

#[test]
fn the_wheel_in_the_alternate_screen_with_no_mouse_mode_sends_three_arrows_a_notch() {
    let mut rt = shown(b"\x1b[?1049hless");
    heard(&mut rt);
    let (x, y) = at(&rt, 1, 1);
    rt.wheel_xy(x, y, 0.0, 100.0);
    assert_eq!(heard(&mut rt), b"\x1b[A\x1b[A\x1b[A");
    rt.wheel_xy(x, y, 0.0, -100.0);
    assert_eq!(heard(&mut rt), b"\x1b[B\x1b[B\x1b[B");
    // In application cursor mode they are SS3 arrows.
    let mut rt = shown(b"\x1b[?1049h\x1b[?1h");
    heard(&mut rt);
    rt.wheel_xy(x, y, 0.0, -100.0);
    assert_eq!(heard(&mut rt), b"\x1bOB\x1bOB\x1bOB");
}

#[test]
fn the_wheel_on_the_normal_screen_with_no_mouse_mode_scrolls_the_view_and_sends_nothing() {
    let mut text = Vec::new();
    for n in 0..40 {
        text.extend(format!("line {n}\r\n").bytes());
    }
    let mut rt = shown(&text);
    heard(&mut rt);
    let (x, y) = at(&rt, 1, 1);
    rt.wheel_xy(x, y, 0.0, 100.0);
    assert_eq!(heard(&mut rt), b"");
    assert_eq!(rt.state.terminal.live.as_ref().unwrap().term.display_offset(), 3);
}

#[test]
fn a_program_that_asked_for_focus_reports_hears_the_box_gain_and_lose_the_keyboard() {
    let mut rt = shown(b"\x1b[?1004h");
    heard(&mut rt);
    rt.update(|_, tx| tx.focus("terminal-view", false));
    rt.frame(FRAME);
    assert_eq!(heard(&mut rt), b"\x1b[I");
    rt.update(|_, tx| tx.focus("composer", false));
    rt.frame(FRAME);
    assert_eq!(heard(&mut rt), b"\x1b[O");
    // Without the mode nothing is sent.
    let mut rt = shown(b"");
    heard(&mut rt);
    rt.update(|_, tx| tx.focus("terminal-view", false));
    rt.frame(FRAME);
    assert_eq!(heard(&mut rt), b"");
}

#[test]
fn text_an_input_method_composed_goes_to_the_shell_when_the_box_has_the_keyboard() {
    let mut rt = shown(b"demo $ ");
    heard(&mut rt);
    rt.ime(Ime::Commit("é".into()));
    assert_eq!(heard(&mut rt), b"", "no keyboard, no text");
    rt.update(|_, tx| tx.focus("terminal-view", false));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("terminal-ime")), "the input method opens at the cursor");
    rt.ime(Ime::Commit("日本語".into()));
    assert_eq!(heard(&mut rt), "日本語".as_bytes());
}
