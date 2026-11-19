//! Picking lines in a file tab and the lines bar (component 11, piece K): `line-pick.test.ts` and `FileCanvas.test.ts` "picked lines
//! show in the bar; Copy link and Quote copy them, and × clears them" and "Add to message puts a line reference in the draft and
//! focuses the composer", driven by the pointer and keys on the invented `src/paginate.py` (no server). The rules under them are in
//! `src/fleet/file_lines.rs`.
use crate::State;
use crate::fleet::file_buffers::{Kind, Request};
use crate::fleet::file_lines::LineRange;
use crate::mock::file_tab::{Case, PATH};
use crate::tests::{FRAME, app_tall, click};
use rune::{Key, KeyEvent, Mods, PhysicalInput, Runtime};

/// The file tab open on `text` (read again over the mock file, as the agent's edit would be), in a window tall enough for all of it.
fn open_with(text: &str) -> Runtime<State> {
    let mut rt = app_tall();
    let text = text.to_string();
    rt.update(move |s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Code, true);
        disk(s, &text, "h2");
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

/// The file on disk is now `text`.
fn disk(s: &mut State, text: &str, hash: &str) {
    let id = s.canvas_session();
    let machine = s.machine_key_of(&id).to_string();
    let request = Request { machine, session: id, path: PATH.to_string(), kind: Kind::Refresh { unavailable: false } };
    let body = serde_json::json!({"content": text, "hash": hash, "isBinary": false, "isTruncated": false}).to_string();
    s.file_buffers.finish(&request, Ok((200, body)));
}

fn thirty() -> String {
    (1..=30).map(|i| format!("const n{i} = {i};")).collect::<Vec<_>>().join("\n")
}

fn picked(rt: &Runtime<State>) -> Option<LineRange> {
    let s = &rt.state;
    let id = s.canvas_session();
    s.file_pick(s.machine_key_of(&id), &id, PATH)
}

fn range(first: u32, last: u32) -> Option<LineRange> {
    Some(LineRange { first, last })
}

fn el(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element {key}"))
}

fn said(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text).unwrap_or_default()
}

/// Fills fade in and out: read them once the frames settle.
fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(FRAME);
    }
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn shift(rt: &mut Runtime<State>, held: bool) {
    rt.physical_input(PhysicalInput::Modifiers(Mods { shift: held, ..Mods::default() }));
}

fn press_number(rt: &mut Runtime<State>, n: u32) {
    click(rt, &format!("code-no-{n}"));
}

fn shift_press_number(rt: &mut Runtime<State>, n: u32) {
    shift(rt, true);
    press_number(rt, n);
    shift(rt, false);
}

/// The middle of a line's number, where a drag down the numbers passes.
fn number_at(rt: &Runtime<State>, n: u32) -> (f32, f32) {
    rt.find(&format!("code-no-{n}")).unwrap_or_else(|| panic!("no number {n}"))
}

/// A drag across the code from the start of line `from` to the start of line `to`: a text selection.
fn select_text(rt: &mut Runtime<State>, from: (u32, f32), to: (u32, f32)) {
    let at = |rt: &Runtime<State>, (n, along): (u32, f32)| {
        let r = el(rt, &format!("code-text-{n}")).rect;
        (r[0] + along, r[1] + r[3] / 2.0)
    };
    let (a, b) = (at(rt, from), at(rt, to));
    rt.pointer_move(a.0, a.1);
    rt.pointer_down(a.0, a.1);
    rt.frame(FRAME);
    rt.pointer_move((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    rt.frame(FRAME);
    rt.pointer_move(b.0, b.1);
    rt.frame(FRAME);
    rt.pointer_up(b.0, b.1);
    rt.frame(FRAME);
}

fn draft(rt: &Runtime<State>) -> String {
    let s = &rt.state;
    let id = s.canvas_session();
    if s.draft_of.as_deref() == Some(id.as_str()) { s.draft.text.clone() } else { s.drafts.text_of(&id).to_string() }
}

// line-pick.test.ts
#[test]
fn a_click_on_a_line_number_picks_that_line_and_highlights_it() {
    let mut rt = open_with(&thirty());
    press_number(&mut rt, 18);
    assert_eq!(picked(&rt), range(18, 18));
    settle(&mut rt);
    let t = crate::theme::Theme::of(true);
    assert_eq!(el(&rt, "code-line-18").fill, t.accent.alpha(0.14), "the line is filled");
    assert_eq!(el(&rt, "code-no-18").fill, t.mix(t.accent, t.panel_bg, 0.14), "and its number");
    assert_eq!(el(&rt, "code-line-17").fill.a, 0.0, "only that line");
}

// line-pick.test.ts
#[test]
fn shift_click_extends_from_where_the_pick_started_either_way() {
    let mut rt = open_with(&thirty());
    press_number(&mut rt, 18);
    shift_press_number(&mut rt, 27);
    assert_eq!(picked(&rt), range(18, 27));
    shift_press_number(&mut rt, 12);
    assert_eq!(picked(&rt), range(12, 18));
}

// line-pick.test.ts
#[test]
fn a_plain_click_starts_a_new_pick() {
    let mut rt = open_with(&thirty());
    press_number(&mut rt, 18);
    shift_press_number(&mut rt, 27);
    press_number(&mut rt, 5);
    assert_eq!(picked(&rt), range(5, 5));
}

// line-pick.test.ts
#[test]
fn dragging_down_the_numbers_extends_the_pick_until_the_button_comes_up() {
    let mut rt = open_with(&thirty());
    let (x, y) = number_at(&rt, 4);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.frame(FRAME);
    let (_, y9) = number_at(&rt, 9);
    rt.pointer_move(x, (y + y9) / 2.0);
    rt.frame(FRAME);
    rt.pointer_move(x, y9);
    rt.frame(FRAME);
    assert_eq!(picked(&rt), range(4, 9));
    rt.pointer_up(x, y9);
    rt.frame(FRAME);
    let (_, y20) = number_at(&rt, 20);
    rt.pointer_move(x, y20);
    rt.frame(FRAME);
    assert_eq!(picked(&rt), range(4, 9));
}

// line-pick.test.ts
#[test]
fn esc_clears_the_pick() {
    let mut rt = open_with(&thirty());
    press_number(&mut rt, 3);
    shift_press_number(&mut rt, 6);
    assert!(has(&rt, "lines-bar"));
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert_eq!(picked(&rt), None);
    assert!(!has(&rt, "lines-bar"));
    settle(&mut rt);
    assert_eq!(el(&rt, "code-line-4").fill.a, 0.0, "the fill goes with it");
}

// line-pick.test.ts
#[test]
fn survives_a_click_in_the_text_but_not_selecting_text() {
    let mut rt = open_with(&thirty());
    press_number(&mut rt, 3);
    shift_press_number(&mut rt, 6);
    click(&mut rt, "code-text-10");
    assert_eq!(picked(&rt), range(3, 6));
    // Scrolling doesn't take it either.
    rt.update(|_, tx| tx.scroll_to("file-code", 0.0, 100.0));
    rt.frame(FRAME);
    assert_eq!(picked(&rt), range(3, 6));
    select_text(&mut rt, (1, 0.5), (1, 40.0));
    assert_eq!(picked(&rt), None);
}

// line-pick.test.ts
#[test]
fn moves_with_lines_added_above_it() {
    let mut rt = open_with(&thirty());
    press_number(&mut rt, 3);
    shift_press_number(&mut rt, 6);
    rt.update(|s, _| disk(s, &format!("new\nnew\n{}", thirty()), "h3"));
    rt.frame(FRAME);
    assert_eq!(picked(&rt), range(5, 8));
}

// line-pick.test.ts, `selectedLines`
#[test]
fn spans_the_selection_s_lines_not_the_line_it_ends_at_the_start_of() {
    let mut rt = open_with("one\ntwo\nthree\nfour");
    assert_eq!(rt.state.file_lines(), None);
    select_text(&mut rt, (2, 0.5), (4, 0.5));
    assert_eq!(rt.state.file_lines().map(|(_, r)| r), range(2, 3));
    select_text(&mut rt, (2, 8.0), (2, 16.0));
    assert_eq!(rt.state.file_lines().map(|(_, r)| r), range(2, 2));
}

// FileCanvas.test.ts
#[test]
fn picked_lines_show_in_the_bar_copy_link_and_quote_copy_them_and_x_clears_them() {
    let mut rt = open_with(&thirty());
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let copied = || clipboard.text().as_deref().map(String::to_string);
    assert!(!has(&rt, "lines-bar"));

    press_number(&mut rt, 18);
    shift_press_number(&mut rt, 27);
    assert_eq!(said(&rt, "line-bar-label"), "L18\u{2013}27");
    assert_eq!(said(&rt, "line-bar-count"), "10 lines");
    assert_eq!(el(&rt, "lines-bar").label.as_deref(), Some("Picked lines"));
    assert!(!has(&rt, "line-bar-file"), "the file tab's bar doesn't name the file");

    // Scrolling the file doesn't hide it.
    rt.update(|_, tx| tx.scroll_to("file-code", 0.0, 80.0));
    rt.frame(FRAME);
    assert!(has(&rt, "lines-bar"));

    assert_eq!(el(&rt, "copy-link").help.as_deref(), Some("Copy src/paginate.py:18-27"));
    click(&mut rt, "copy-link");
    assert_eq!(copied().as_deref(), Some("src/paginate.py:18-27"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Copied src/paginate.py:18-27")));
    assert!(has(&rt, "file-toast"), "the toast shows, lifted over the bar");

    press_number(&mut rt, 2);
    shift_press_number(&mut rt, 3);
    click(&mut rt, "copy-quote");
    assert_eq!(copied().as_deref(), Some("src/paginate.py:2-3\n```py\nconst n2 = 2;\nconst n3 = 3;\n```\n"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Copied L2\u{2013}3 as a quote")));

    press_number(&mut rt, 7);
    assert_eq!(said(&rt, "line-bar-label"), "L7");
    assert_eq!(said(&rt, "line-bar-count"), "1 line");
    click(&mut rt, "copy-link");
    assert_eq!(copied().as_deref(), Some("src/paginate.py:7"));

    click(&mut rt, "line-bar-clear");
    assert!(!has(&rt, "lines-bar"));
    assert_eq!(picked(&rt), None);

    // The toast goes after 2.6 s.
    for _ in 0..(2.7 / FRAME) as usize {
        rt.frame(FRAME);
    }
    assert!(!has(&rt, "file-toast"));
}

// FileCanvas.test.ts
#[test]
fn add_to_message_puts_a_line_reference_in_the_draft_and_focuses_the_composer() {
    let mut rt = open_with("one\ntwo\nthree\nfour\n");
    // A text selection shows in the bar too, as the lines it spans.
    select_text(&mut rt, (2, 0.5), (4, 0.5));
    assert_eq!(said(&rt, "line-bar-label"), "L2\u{2013}3");
    click(&mut rt, "add-to-message");
    rt.frame(FRAME);
    assert_eq!(draft(&rt), "@src/paginate.py:2-3 ");
    assert!(el(&rt, "composer").focused, "the composer has the keyboard");
    assert!(!has(&rt, "lines-bar"));
}

#[test]
fn the_bar_isn_t_there_while_the_file_is_rendered() {
    let mut rt = app_tall();
    rt.update(|s, _| s.show_file(Case::Markdown, true));
    rt.frame(FRAME);
    rt.update(|s, _| {
        let id = s.canvas_session();
        let machine = s.machine_key_of(&id).to_string();
        assert!(
            s.file_buffers.set_pick(&machine, &id, "README.md", Some(crate::fleet::file_lines::LinePick { anchor: 1, head: 2 })),
            "the Markdown file is open"
        );
    });
    rt.frame(FRAME);
    assert!(!has(&rt, "lines-bar"));
}
