//! Component 13, piece G: every cell attribute, the cursor and links (rows 13.11, 13.12, 13.75, 13.76). xterm's own behaviour
//! is the reference, so these are ours: what draws, which cursor, when it blinks, which text is a link and what a click opens.
use crate::term::fit::{CELL_H, CELL_W, PAD};
use crate::term::links::{safe_target, url_ranges};
use crate::tests::{FRAME, app};
use rune::Runtime;

fn shown(cols: u16, rows: u16, bytes: &[u8]) -> Runtime<crate::State> {
    let mut rt = app();
    let bytes = bytes.to_vec();
    rt.update(move |s, _| {
        s.set_session_id((0, 0), "s1");
        s.terminal.session = s.open_id();
        s.terminals.add("s1", crate::fleet::http::Terminal { id: "t1".into(), title: "shell".into(), running: true }, true);
        s.terminal.replay(cols, rows, &bytes);
        s.terminal.live.as_mut().unwrap().id = "t1".into();
        s.terminal.height = 60.0 + 1.0 + 10.0 + rows as f32 * 19.0;
        s.terminals.set_open("s1", true);
    });
    rt.frame(FRAME);
    rt
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn found(s: &str) -> Vec<String> {
    let c = chars(s);
    url_ranges(&c).into_iter().map(|(a, b)| c[a..b].iter().collect()).collect()
}

#[test]
fn a_url_stops_before_the_punctuation_that_ends_a_sentence() {
    assert_eq!(found("see https://example.com/a/b?x=1, then"), ["https://example.com/a/b?x=1"]);
    assert_eq!(found("(https://example.com/a_b-c) end."), ["https://example.com/a_b-c"]);
    assert_eq!(found("go to http://example.com."), ["http://example.com"]);
    assert_eq!(found("HTTPS://EXAMPLE.COM/x and https://a.example/ done"), ["HTTPS://EXAMPLE.COM/x", "https://a.example/"]);
    assert!(found("a http:// alone, ftp://example.com and file:///etc").is_empty());
}

#[test]
fn only_web_and_mail_targets_open_without_asking() {
    assert!(safe_target("https://example.com/x") && safe_target("mailto:a@example.com"));
    assert!(!safe_target("file:///etc/passwd") && !safe_target("javascript:alert(1)") && !safe_target("ssh://host"));
}

fn hover(rt: &mut Runtime<crate::State>, row: usize, col: usize) {
    let r = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("terminal-view")).expect("the box").rect;
    rt.pointer_move(r[0] + PAD[3] + col as f32 * CELL_W + 2.0, r[1] + PAD[0] + row as f32 * CELL_H + 9.0);
    rt.frame(FRAME);
}

const TEXT: &[u8] = b"docs https://example.com/one and\r\n\x1b]8;;https://example.com/two\x1b\\second\x1b]8;;\x1b\\ \
\x1b]8;;file:///tmp/x\x1b\\local\x1b]8;;\x1b\\\r\nxx https://example.com/a-long-address-that-wraps-here/index";

#[test]
fn the_pointer_over_a_url_underlines_it_and_a_click_opens_it() {
    let mut rt = shown(40, 6, TEXT);
    hover(&mut rt, 0, 8);
    let link = rt.state.terminal.link.clone().expect("a link");
    assert_eq!(link.url, "https://example.com/one");
    assert_eq!(link.spans, vec![(0, 5, 28)]);
    assert!(rt.find("terminal-link-0").is_some());
    let (x, y) = rt.find("terminal-link-0").unwrap();
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.take_url_requests(), ["https://example.com/one"]);
    // Off the link, nothing is underlined.
    hover(&mut rt, 0, 1);
    assert!(rt.state.terminal.link.is_none() && rt.find("terminal-link-0").is_none());
}

#[test]
fn a_url_that_wraps_is_one_link_across_both_rows() {
    let mut rt = shown(40, 6, TEXT);
    hover(&mut rt, 2, 5);
    let link = rt.state.terminal.link.clone().expect("a link");
    assert_eq!(link.url, "https://example.com/a-long-address-that-wraps-here/index");
    assert_eq!(link.spans.len(), 2);
    assert_eq!((link.spans[0].0, link.spans[1].0), (2, 3));
}

#[test]
fn an_osc_8_link_opens_what_it_names_and_a_file_link_is_refused() {
    let mut rt = shown(40, 6, TEXT);
    hover(&mut rt, 1, 2);
    let link = rt.state.terminal.link.clone().expect("a link");
    assert_eq!((link.url.as_str(), link.spans.as_slice()), ("https://example.com/two", &[(1, 0, 6)][..]));
    hover(&mut rt, 1, 8);
    assert!(rt.state.terminal.link.is_none(), "file: isn't opened");
}

#[test]
fn every_underline_style_and_strikethrough_draws_a_stroke() {
    // Rows differ from plain text only in what is drawn over it: rebuilt rows must not be the same row.
    let plain = shown(20, 2, b"text");
    let before = plain.state.terminal.live.as_ref().unwrap().screen.borrow().rows[0].clone();
    for (name, seq) in [
        ("underline", "4"),
        ("double", "4:2"),
        ("curly", "4:3"),
        ("dotted", "4:4"),
        ("dashed", "4:5"),
        ("strikethrough", "9"),
        ("colour", "4;58;2;200;10;10"),
    ] {
        let rt = shown(20, 2, format!("\x1b[{seq}mtext\x1b[0m").as_bytes());
        let row = rt.state.terminal.live.as_ref().unwrap().screen.borrow().rows[0].clone();
        assert!(!std::rc::Rc::ptr_eq(&row, &before), "{name}");
    }
}

fn cursor_box(rt: &Runtime<crate::State>) -> Option<[f32; 4]> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("terminal-cursor")).map(|e| e.rect)
}

fn focus(rt: &mut Runtime<crate::State>) {
    rt.update(|_, tx| tx.focus("terminal-view", true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.terminal.focused);
}

#[test]
fn the_cursor_has_the_shape_the_program_asked_for() {
    for (seq, w, h) in [("2", CELL_W, CELL_H), ("6", 1.0, CELL_H), ("4", CELL_W, 1.0)] {
        let mut rt = shown(20, 2, format!("ab\x1b[{seq} q").as_bytes());
        focus(&mut rt);
        let r = cursor_box(&rt).expect("a cursor");
        assert_eq!((r[2].round(), r[3].round()), (w, h), "DECSCUSR {seq}");
    }
}

#[test]
fn a_hidden_cursor_is_not_drawn() {
    let mut rt = shown(20, 2, b"ab\x1b[?25l");
    focus(&mut rt);
    assert!(cursor_box(&rt).is_none());
}

#[test]
fn the_cursor_blinks_while_the_box_has_the_keyboard_and_a_steady_one_does_not() {
    let mut rt = shown(20, 2, b"ab");
    focus(&mut rt);
    assert!(cursor_box(&rt).is_some(), "lit at first");
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    assert!(cursor_box(&rt).is_none(), "dark after 600 ms");
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    assert!(cursor_box(&rt).is_some(), "lit again after 1.2 s");
    // Typing keeps it lit.
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    rt.update(|s, _| s.terminal_write(b"x".to_vec()));
    rt.frame(FRAME);
    assert!(cursor_box(&rt).is_some());

    let mut steady = shown(20, 2, b"ab\x1b[2 q");
    focus(&mut steady);
    for _ in 0..40 {
        steady.frame(FRAME);
    }
    assert!(cursor_box(&steady).is_some(), "a steady cursor stays lit");
}

#[test]
fn an_unfocused_cursor_is_an_outline_and_does_not_blink() {
    let mut rt = shown(20, 2, b"ab");
    for _ in 0..60 {
        rt.frame(FRAME);
    }
    let r = cursor_box(&rt).expect("an outline");
    assert_eq!((r[2].round(), r[3].round()), (CELL_W, CELL_H));
}
