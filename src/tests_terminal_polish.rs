//! Component 26, piece S (rows 2.89): what 13's pieces F and G left in the terminal's box: the bubble of a selection that ends at a
//! row's start, the scrollbar's track click and fade, a drag held past an edge scrolling on a timer, and a drag that starts on a link.
use crate::term::fit::{CELL_H, PAD};
use crate::tests::FRAME;
use crate::tests_terminal_select::{at, scrolled_stream, shown, term};
use crate::view::terminal_bar::{FADE, LINGER, offset_at, opacity};

const BUILD: &[u8] = b"$ npm run build\r\nnpm ERR! missing script: build\r\nsee the log for details\r\n$ ";

fn run(rt: &mut rune::Runtime<crate::State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

#[test]
fn a_selection_that_ends_at_a_rows_start_puts_the_bubble_at_that_row_and_column_0() {
    let mut rt = shown(40, 6, BUILD);
    let (x0, y0) = at(&rt, 2, 1, 1.0);
    let (x1, y1) = at(&rt, 0, 2, 1.0);
    rt.pointer_move(x0, y0);
    rt.pointer_down(x0, y0);
    rt.pointer_move(x1, y1);
    rt.frame(FRAME);
    let span = term(&rt).selection_span().expect("a selection");
    assert_eq!((span.last_row, span.end_x), (2, 0), "xterm's end is column 0 of the row the pointer is on");
    assert_eq!(span.first_row, 1);
    // A drag that ends mid-row is as before: the cell after the last one picked.
    let (x2, y2) = at(&rt, 6, 2, 1.0);
    rt.pointer_move(x2, y2);
    rt.frame(FRAME);
    let span = term(&rt).selection_span().unwrap();
    assert_eq!((span.last_row, span.end_x), (2, 7));
}

#[test]
fn the_scrollbar_fades_after_the_pointer_leaves_and_a_click_on_its_track_moves_the_view() {
    assert_eq!(opacity(true, 9.0), 1.0);
    assert_eq!(opacity(false, LINGER), 1.0);
    assert!((opacity(false, LINGER + FADE / 2.0) - 0.5).abs() < 1e-6);
    assert_eq!(opacity(false, LINGER + FADE), 0.0);
    // The slider centres on the pointer: the top of the track is the oldest line, the bottom the live screen.
    assert_eq!(offset_at(0.0, 100.0, 40, 6), 40);
    assert_eq!(offset_at(100.0, 100.0, 40, 6), 0);

    let mut rt = shown(40, 6, &scrolled_stream());
    let (x, y) = at(&rt, 3, 2, 1.0);
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    let (bx, by) = rt.find("terminal-scrollbar").expect("the bar");
    let track = 6.0 * CELL_H;
    // Press near the track's top, away from the slider (which is at the bottom while the view is live).
    rt.pointer_move(bx, by - track / 2.0 + 2.0);
    rt.pointer_down(bx, by - track / 2.0 + 2.0);
    rt.pointer_up(bx, by - track / 2.0 + 2.0);
    rt.frame(FRAME);
    assert!(term(&rt).display_offset() > 20, "went most of the way back: {}", term(&rt).display_offset());
    // Leaving the box: it stays 500 ms, fades over 800 ms, then goes.
    rt.pointer_move(1.0, 1.0);
    run(&mut rt, 0.3);
    assert_eq!(rt.presented("terminal-scrollbar").map(|p| p.1), Some(1.0));
    run(&mut rt, LINGER + FADE / 2.0);
    let half = rt.presented("terminal-scrollbar").expect("fading").1;
    assert!(half > 0.2 && half < 0.8, "half way: {half}");
    run(&mut rt, FADE);
    assert!(rt.find("terminal-scrollbar").is_none(), "gone");
}

#[test]
fn a_drag_held_past_the_top_scrolls_on_a_timer_and_stops_when_let_go() {
    let mut rt = shown(40, 6, &scrolled_stream());
    let (x0, y0) = at(&rt, 2, 3, 1.0);
    let (x1, _) = at(&rt, 6, 0, 1.0);
    rt.pointer_move(x0, y0);
    rt.pointer_down(x0, y0);
    // One move past the top edge; the pointer then stays still.
    let top = y0 - 3.0 * CELL_H - CELL_H / 2.0 - PAD[0];
    rt.pointer_move(x1, top - 2.0);
    rt.frame(FRAME);
    let first = term(&rt).display_offset();
    run(&mut rt, 0.5);
    let later = term(&rt).display_offset();
    assert!(later >= first + 8, "about 10 ticks in 0.5 s: {first} -> {later}");
    rt.pointer_up(x1, top - 2.0);
    run(&mut rt, 0.3);
    assert_eq!(term(&rt).display_offset(), later, "it stops when the button is let go");
    assert!(term(&rt).has_selection());
}

#[test]
fn a_drag_that_starts_on_a_link_selects_and_a_click_on_it_opens_it() {
    let text: &[u8] = b"docs https://example.com/one and more\r\nsecond row\r\n";
    let mut rt = shown(40, 6, text);
    let (x0, y0) = at(&rt, 8, 0, 1.0);
    rt.pointer_move(x0, y0);
    rt.frame(FRAME);
    assert!(rt.state.terminal.link.is_some(), "the pointer is over the address");
    let (x1, y1) = at(&rt, 3, 1, 1.0);
    rt.pointer_down(x0, y0);
    rt.pointer_move(x1, y1);
    rt.pointer_up(x1, y1);
    rt.frame(FRAME);
    assert!(term(&rt).selected_text().is_some_and(|t| t.starts_with("ps://")), "selected from inside the link");
    assert!(rt.take_url_requests().is_empty(), "a drag opens nothing");
    // The click that never moved opens it.
    rt.pointer_move(x0, y0);
    rt.pointer_down(x0, y0);
    rt.pointer_up(x0, y0);
    rt.frame(FRAME);
    assert_eq!(rt.take_url_requests(), ["https://example.com/one"]);
}
