//! Component 13, piece F: selecting, copying, pasting and "Add lines to message" in the terminal's box. Fleet's own test is
//! `TerminalDrawer.test.ts` "puts selected lines in the session's draft and asks the composer to take focus"; the rest is ours
//! (xterm's selection is xterm's: wrapped rows joined, the column-0 end, blanks give none, the bubble above or under, the 130 px
//! clamp, bracketed paste, the wheel, the scrollbar).
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::keys::paste_bytes;
use crate::term::selection::{Pick, Span};
use crate::term::{Terminal, text_of};
use crate::tests::{FRAME, app, click};
use crate::view::terminal_bar::slider;
use crate::view::terminal_pop::{ADD, COPY_KEY, POP, place};
use rune::Runtime;
#[cfg(not(target_os = "macos"))]
use rune::{Key, KeyEvent, Mods};

/// The open session's drawer on a terminal that was fed `bytes`, `cols` × `rows`, as the scenes show one.
pub(crate) fn shown(cols: u16, rows: u16, bytes: &[u8]) -> Runtime<crate::State> {
    let mut rt = app();
    let bytes = bytes.to_vec();
    rt.update(move |s, _| {
        s.set_session_id((0, 0), "s1");
        s.terminal.session = s.open_id();
        s.terminals.add("s1", crate::fleet::http::Terminal { id: "t1".into(), title: "build shell".into(), running: true }, true);
        s.terminal.replay(cols, rows, &bytes);
        s.terminal.live.as_mut().unwrap().id = "t1".into();
        s.terminal.live.as_mut().unwrap().title = "build shell".into();
        // The header and the rule above the box, then the box's padding and the rows.
        s.terminal.height = 60.0 + 1.0 + 10.0 + rows as f32 * 19.0;
        s.terminals.set_open("s1", true);
    });
    rt.frame(FRAME);
    rt
}

pub(crate) fn term(rt: &Runtime<crate::State>) -> std::sync::Arc<Terminal> {
    rt.state.terminal.live.as_ref().unwrap().term.clone()
}

/// The window point of a cell's left edge (`dx` of a cell in) and middle of its row.
pub(crate) fn at(rt: &Runtime<crate::State>, col: usize, row: usize, dx: f32) -> (f32, f32) {
    let r = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("terminal-view")).expect("the box").rect;
    (r[0] + PAD[3] + col as f32 * CELL_W + dx, r[1] + PAD[0] + row as f32 * CELL_H + CELL_H / 2.0)
}

fn drag(rt: &mut Runtime<crate::State>, from: (usize, usize), to: (usize, usize)) {
    let (x0, y0) = at(rt, from.0, from.1, 1.0);
    let (x1, y1) = at(rt, to.0, to.1, 5.0);
    rt.pointer_move(x0, y0);
    rt.pointer_down(x0, y0);
    rt.pointer_move(x1, y1);
    rt.pointer_up(x1, y1);
    rt.frame(FRAME);
}

fn lines(t: &Terminal) -> Option<(usize, usize, String)> {
    t.selected_lines().map(|l| (l.from, l.to, l.text))
}

const BUILD: &[u8] = b"$ npm run build\r\nnpm ERR! missing script: build\r\nsee the log for details\r\n$ ";

// ---- the mouse ----------------------------------------------------------------------------------------------------------------

#[test]
fn dragging_selects_text_and_a_click_selects_nothing() {
    let mut rt = shown(60, 6, BUILD);
    drag(&mut rt, (4, 1), (8, 2));
    // From "ERR!" on row 1 to column 7 of row 2 (the pointer is in that cell's right half).
    assert_eq!(term(&rt).selected_text().as_deref(), Some("ERR! missing script: build\nsee the l"));
    assert!(term(&rt).has_selection());
    // A click with no movement takes the selection away.
    let (x, y) = at(&rt, 3, 0, 1.0);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    assert!(!term(&rt).has_selection());
    assert!(rt.find(POP).is_none());
}

#[test]
fn a_double_click_selects_a_word_and_a_triple_click_the_line() {
    let mut rt = shown(60, 6, BUILD);
    let (x, y) = at(&rt, 9, 1, 2.0);
    rt.pointer_move(x, y);
    for _ in 0..2 {
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
    }
    rt.frame(FRAME);
    assert_eq!(term(&rt).selected_text().as_deref(), Some("missing"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    assert_eq!(term(&rt).selected_text().as_deref(), Some("npm ERR! missing script: build"));
}

#[test]
fn only_the_rows_the_selection_touches_are_drawn_again_and_the_colour_decides_their_look() {
    use crate::term::palette::Palette;
    use crate::term::screen::Screen;
    let t = Terminal::new(30, 5);
    t.feed(b"one\r\ntwo\r\nthree\r\nfour\r\n");
    let theme = crate::theme::Theme::of(true);
    let pal = Palette::new(&theme);
    let mut screen = Screen::default();
    let read = |screen: &mut Screen, color| {
        screen.selection = color;
        t.read_damage(|term| screen.read(term, &pal, theme.terminal_bg(), 1));
    };
    read(&mut screen, Some(pal.selection));
    let before = screen.rows.clone();
    t.select((0, 1), (2, 2));
    read(&mut screen, Some(pal.selection));
    let same: Vec<bool> = before.iter().zip(&screen.rows).map(|(a, b)| std::rc::Rc::ptr_eq(a, b)).collect();
    assert_eq!(same, [true, false, false, true, true], "rows 1 and 2 only");
    // Nothing changed: nothing is built.
    read(&mut screen, Some(pal.selection));
    assert_eq!(screen.built, 0);
    // The inactive colour (no keyboard) is another look for the same two rows, and for no others.
    read(&mut screen, Some(pal.selection_inactive));
    assert_eq!(screen.built, 2);
    // Clearing puts them back.
    t.clear_selection();
    read(&mut screen, Some(pal.selection_inactive));
    assert_eq!(screen.built, 2);
}

// ---- the lines (13.71) --------------------------------------------------------------------------------------------------------

#[test]
fn whole_rows_are_added_numbered_from_the_top_of_the_scrollback() {
    // 30 lines on a 5-row screen: 26 rows are in the scrollback.
    let mut out = String::new();
    for i in 1..=30 {
        out.push_str(&format!("line {i}\r\n"));
    }
    let t = Terminal::new(40, 5);
    t.feed(out.as_bytes());
    // The screen's rows are lines 27-30 and the empty row after them; select 28 and 29 from the middle of each.
    t.select((3, 1), (3, 2));
    assert_eq!(lines(&t), Some((28, 29, "line 28\nline 29".into())));
    // Scrolled back, the same screen row is another line.
    t.scroll_by(10);
    t.select((0, 0), (2, 0));
    assert_eq!(lines(&t), Some((17, 17, "line 17".into())));
}

#[test]
fn a_row_that_continues_a_wrapped_line_is_joined_onto_it() {
    let t = Terminal::new(10, 5);
    t.feed(b"abcdefghijklmnop\r\nnext\r\n");
    t.select((2, 0), (2, 2));
    // Two rows of one line, then the next line; the selected text itself is just the part dragged over.
    assert_eq!(lines(&t), Some((1, 3, "abcdefghijklmnop\nnext".into())));
}

#[test]
fn a_selection_that_ends_at_column_0_of_a_later_row_leaves_that_row_out() {
    let t = Terminal::new(30, 5);
    t.feed(b"one\r\ntwo\r\nthree\r\n");
    t.select_start(Pick::Chars, 1, 0, false);
    t.select_to(0, 2, false);
    assert_eq!(lines(&t), Some((1, 2, "one\ntwo".into())));
    // One more column in and the row counts.
    t.select_to(1, 2, true);
    assert_eq!(lines(&t), Some((1, 3, "one\ntwo\nthree".into())));
}

#[test]
fn trailing_blanks_go_and_nothing_but_blanks_gives_none() {
    let t = Terminal::new(30, 5);
    t.feed(b"one   \r\n\r\n\r\ntwo\r\n");
    t.select((0, 0), (10, 0));
    assert_eq!(lines(&t), Some((1, 1, "one".into())));
    t.select((0, 1), (10, 2));
    assert_eq!(lines(&t), None, "two blank rows");
    // A selection of blank rows has no bubble either.
    let mut rt = shown(30, 5, b"one\r\n\r\n\r\ntwo\r\n");
    drag(&mut rt, (0, 1), (10, 2));
    assert!(rt.find(POP).is_none());
}

// ---- the bubble (13.69, 13.70) ------------------------------------------------------------------------------------------------

#[test]
fn the_bubble_is_above_the_first_row_or_under_the_last_when_that_is_near_the_top() {
    let size = (200.0, 34.0);
    // Row 5: above it, 4 px clear of the row's top + 6.
    let (_, top, below) = place(Span { first_row: 5, last_row: 6, end_x: 20 }, 11, 800.0, size);
    assert!(!below);
    assert_eq!(top, 5.0 * 19.0 + 6.0 - 34.0 - 4.0);
    // Row 1 is 19 px from the top: under the last row instead.
    let (_, top, below) = place(Span { first_row: 1, last_row: 3, end_x: 20 }, 11, 800.0, size);
    assert!(below);
    assert_eq!(top, 4.0 * 19.0 + 6.0);
    // The first row counts as near the top too, and a last row past the screen is held to the screen.
    let (_, top, _) = place(Span { first_row: 0, last_row: 40, end_x: 20 }, 11, 800.0, size);
    assert_eq!(top, 11.0 * 19.0 + 6.0);
}

#[test]
fn the_bubble_keeps_130_px_from_either_side_of_the_box() {
    let size = (200.0, 34.0);
    let centre = |end_x: usize| {
        let (left, ..) = place(Span { first_row: 5, last_row: 5, end_x }, 11, 820.0, size);
        left + size.0 / 2.0
    };
    assert_eq!(centre(0), 130.0);
    assert_eq!(centre(10), 130.0);
    assert_eq!(centre(40), 280.0);
    assert_eq!(centre(115), 690.0, "820 - 130");
}

#[test]
fn the_bubble_offers_add_lines_and_copy_over_a_selection() {
    let mut rt = shown(60, 6, BUILD);
    assert!(rt.find(POP).is_none(), "nothing selected, no bubble");
    drag(&mut rt, (0, 1), (5, 2));
    assert!(rt.find(POP).is_some());
    let label =
        |rt: &Runtime<crate::State>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label);
    assert_eq!(label(&rt, ADD).as_deref(), Some("Add lines 2\u{2013}3 to message"));
    assert_eq!(label(&rt, COPY_KEY).as_deref(), Some("Copy"));
    drag(&mut rt, (0, 1), (5, 1));
    assert_eq!(label(&rt, ADD).as_deref(), Some("Add line 2 to message"));
}

// "puts selected lines in the session's draft and asks the composer to take focus"
#[test]
fn puts_selected_lines_in_the_sessions_draft_and_asks_the_composer_to_take_focus() {
    let mut rt = shown(60, 6, BUILD);
    drag(&mut rt, (0, 1), (5, 2));
    click(&mut rt, ADD);
    let owner = rt.state.draft_owner().expect("an open session");
    let held = rt.state.held.terminals(&owner);
    assert_eq!(held.len(), 1);
    assert_eq!((held[0].terminal_id.as_str(), held[0].label.as_str(), held[0].from, held[0].to), ("t1", "build shell", 2, 3));
    assert_eq!(held[0].text, "npm ERR! missing script: build\nsee the log for details");
    assert!(held[0].id.starts_with("t1:2-3:"), "{}", held[0].id);
    assert!(!term(&rt).has_selection(), "the selection clears");
    assert!(rt.find(POP).is_none(), "and the bubble goes");
    let focused = rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused.as_deref(), Some("composer"));

    // The same lines of the same terminal are one chip.
    drag(&mut rt, (0, 1), (5, 2));
    click(&mut rt, ADD);
    assert_eq!(rt.state.held.terminals(&owner).len(), 1);
}

#[test]
fn the_bubbles_copy_puts_the_text_on_the_clipboard_and_clears_the_selection() {
    let clipboard = rune::clipboard::memory();
    let mut rt = shown(60, 6, BUILD);
    rt.use_clipboard(&clipboard);
    drag(&mut rt, (4, 1), (8, 2));
    click(&mut rt, COPY_KEY);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("ERR! missing script: build\nsee the l"));
    assert!(!term(&rt).has_selection());
}

// ---- keys (13.67, 13.68) ------------------------------------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
fn chord(key: Key) -> KeyEvent {
    KeyEvent { key, mods: Mods { ctrl: true, shift: true, ..Mods::default() } }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn ctrl_shift_c_copies_the_selection_and_keeps_it() {
    let clipboard = rune::clipboard::memory();
    let mut rt = shown(60, 6, BUILD);
    rt.use_clipboard(&clipboard);
    drag(&mut rt, (4, 1), (8, 2));
    click(&mut rt, "terminal-view");
    // A click would have cleared it; select again, with the box focused.
    drag(&mut rt, (4, 1), (8, 2));
    rt.key(chord(Key::Command('c')));
    rt.frame(FRAME);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("ERR! missing script: build\nsee the l"));
    assert!(term(&rt).has_selection(), "the keys leave the selection (only the bubble's Copy clears it)");
}

#[test]
fn a_paste_is_bracketed_only_when_the_program_asked_and_newlines_are_returns() {
    assert_eq!(paste_bytes("one\ntwo\r\nthree", false), b"one\rtwo\rthree");
    assert_eq!(paste_bytes("ls", true), b"\x1b[200~ls\x1b[201~");
    assert_eq!(paste_bytes("a\nb", true), b"\x1b[200~a\rb\x1b[201~");
    // The program asks with CSI ? 2004 h and stops asking with CSI ? 2004 l.
    let t = Terminal::new(20, 3);
    assert!(!t.mode().contains(crate::term::BRACKETED_PASTE));
    t.feed(b"\x1b[?2004h");
    assert!(t.mode().contains(crate::term::BRACKETED_PASTE));
    t.feed(b"\x1b[?2004l");
    assert!(!t.mode().contains(crate::term::BRACKETED_PASTE));
}

// ---- the wheel and the scrollbar (13.6, 13.7) ---------------------------------------------------------------------------------

pub(crate) fn scrolled_stream() -> Vec<u8> {
    (1..=40).flat_map(|i| format!("line {i}\r\n").into_bytes()).collect()
}

#[test]
fn the_wheel_scrolls_the_view_and_new_output_leaves_it_where_it_is() {
    let mut rt = shown(40, 6, &scrolled_stream());
    let t = term(&rt);
    assert_eq!(t.display_offset(), 0);
    let (x, y) = at(&rt, 3, 2, 1.0);
    rt.wheel_xy(x, y, 0.0, 100.0);
    rt.frame(FRAME);
    assert_eq!(t.display_offset(), 3, "a notch is three lines");
    rt.wheel_xy(x, y, 0.0, -100.0);
    rt.frame(FRAME);
    assert_eq!(t.display_offset(), 0);
    // Scrolled back, output arrives: the same lines stay in view.
    rt.wheel_xy(x, y, 0.0, 100.0);
    rt.frame(FRAME);
    let before = text_of(&t);
    let _ = before;
    let offset = t.display_offset();
    t.feed(b"another\r\n");
    assert_eq!(t.display_offset(), offset + 1, "the view stays on the same lines, so it moves one further back");
}

#[test]
fn the_scrollbar_shows_over_the_box_with_scrollback_and_its_slider_follows_the_view() {
    // The slider: proportional to the rows over the whole, never under 20 px, at the bottom while the view is live.
    let (top, height) = slider(209.0, 37, 0, 11);
    assert!((height - 209.0 * 11.0 / 48.0).abs() < 0.01);
    assert!((top - (209.0 - height)).abs() < 0.01, "bottom at the live screen");
    assert_eq!(slider(209.0, 37, 37, 11).0, 0.0, "top at the oldest line");
    assert_eq!(slider(209.0, 5000, 0, 11).1, 20.0);

    let mut rt = shown(40, 6, &scrolled_stream());
    assert!(rt.find("terminal-scrollbar").is_none(), "hidden until the pointer is over the box");
    let (x, y) = at(&rt, 3, 2, 1.0);
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert!(rt.find("terminal-scrollbar").is_some());
    // Dragging the slider up moves the view back.
    let (sx, sy) = rt.find("terminal-slider").expect("the slider");
    rt.pointer_move(sx, sy);
    rt.pointer_down(sx, sy);
    rt.pointer_move(sx, sy - 20.0);
    rt.frame(FRAME);
    assert!(rt.state.terminal.bar_held);
    assert!(term(&rt).display_offset() > 0);
    rt.pointer_up(sx, sy - 20.0);
    rt.frame(FRAME);
    assert!(!rt.state.terminal.bar_held);
    // No scrollback, no bar.
    let mut rt = shown(40, 6, b"one\r\n");
    let (x, y) = at(&rt, 3, 2, 1.0);
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert!(rt.find("terminal-scrollbar").is_none());
}

#[test]
fn clicking_the_box_still_gives_it_the_keyboard() {
    let mut rt = shown(40, 6, BUILD);
    click(&mut rt, "terminal-view");
    let focused = rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused.as_deref(), Some("terminal-view"));
}
