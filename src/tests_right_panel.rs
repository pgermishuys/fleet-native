//! The right panel's shell (`SessionsV2RightPanel.vue`, `CollapsedRightRail.vue`, `AppShell.vue`; piece Q of
//! docs/parity/06-session-list.md). Fleet's `SessionsV2RightPanel.test.ts` is about the diffs instance the panel shares with
//! its route (component 10's); the width, collapse and empty-session rules have no unit test there, so the names say what
//! `AppShell.vue` does.
use crate::fleet::right_panel::{self as rp, RightPanel};
use crate::fleet::settings::Sidebar;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn open() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn drag_gutter(rt: &mut Runtime<crate::State>, dx: f32) {
    let (x, y) = rt.find("right-gutter").expect("the gutter");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + dx / 2.0, y);
    rt.pointer_move(x + dx, y);
    rt.pointer_up(x + dx, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn the_panel_starts_360_wide_and_shows_a_gutter_only_while_it_is_an_inline_column() {
    let rt = open();
    assert_eq!(rt.state.right.width, 360.0);
    assert_eq!(rt.state.right_shown_w(), 360.0);
    assert!(rt.find("right-gutter").is_some());
    let mut rt = app();
    assert!(rt.find("right-gutter").is_none(), "collapsed: the rail has no gutter");
    assert!(rt.find("content-rail").is_some());
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("content-rail").is_none());
}

#[test]
fn dragging_the_gutter_left_grows_the_panel_and_right_shrinks_it_to_280() {
    let mut rt = open();
    // With the list folded the sheet has room to grow into.
    rt.update(|s, _| s.sidebar_open = false);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let room = rp::inline_room(rt.state.sheet_w);
    assert!(room > 460.0, "the test window leaves room: {room}");
    drag_gutter(&mut rt, -60.0);
    assert!((rt.state.right.width - 420.0).abs() < 2.0, "grew by the drag: {}", rt.state.right.width);
    drag_gutter(&mut rt, 400.0);
    assert_eq!(rt.state.right.width, rp::MIN_W, "never narrower than 280");
}

#[test]
fn the_width_never_passes_half_the_app_or_the_room_beside_the_conversation() {
    let mut p = RightPanel::default();
    p.drag(360.0, -2000.0, 1000.0, 1200.0);
    assert_eq!(p.width, 500.0, "half of the app (1000)");
    p.drag(360.0, -2000.0, 2000.0, 700.0);
    assert_eq!(p.width, rp::MIN_W, "the room (700 - 480 = 220) is under the minimum");
    p.drag(600.0, 0.0, 1000.0, 1400.0);
    assert_eq!(p.width, 600.0, "a width already past half stays as it was");
    p.drag(360.0, -2000.0, 4000.0, 1000.0);
    assert_eq!(p.width, 520.0, "1000 - 480");
}

#[test]
fn the_panel_is_a_column_only_while_the_conversation_keeps_480_beside_it() {
    assert!(rp::fits(760.0));
    assert!(!rp::fits(759.0));
    assert!(rp::fits(0.0), "unmeasured counts as roomy");
    assert_eq!(rp::inline_room(900.0), 420.0);
}

#[test]
fn widen_gives_the_panel_760_or_what_the_room_allows_without_losing_the_dragged_width() {
    let mut p = RightPanel { widened: true, ..Default::default() };
    assert_eq!(p.shown(1400.0), 760.0);
    assert_eq!(p.shown(1000.0), 520.0, "the room is 520");
    p.width = 800.0;
    assert_eq!(p.shown(1600.0), 800.0, "a wider dragged width wins");
    p.widened = false;
    p.width = 360.0;
    assert_eq!(p.shown(1400.0), 360.0, "widen off gives the dragged width back");
}

#[test]
fn review_gives_60_percent_of_the_sheet_until_the_gutter_is_dragged_once() {
    let mut p = RightPanel { review: true, ..Default::default() };
    assert_eq!(p.shown(1300.0), 780.0);
    assert_eq!(p.shown(1000.0), 520.0, "capped by the room");
    let start = p.begin_drag(1300.0);
    assert_eq!(start, 780.0, "the first press takes over from the width on screen");
    assert!(p.user_sized && !p.reviewing());
    assert_eq!(p.shown(1300.0), 780.0);
    p.drag(start, 100.0, 1400.0, 1300.0);
    assert_eq!(p.shown(1300.0), 680.0, "and it is the user's from then on");
}

#[test]
fn the_first_press_while_widened_takes_over_and_turns_widen_off() {
    let mut p = RightPanel { widened: true, ..Default::default() };
    let start = p.begin_drag(1400.0);
    assert_eq!(start, 760.0);
    assert!(!p.widened);
    assert_eq!(p.width, 760.0);
}

#[test]
fn widening_in_the_window_makes_the_panel_wider_and_a_drag_takes_it_over() {
    let mut rt = open();
    rt.update(|s, tx| s.set_widened(true, tx));
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    let room = rp::inline_room(rt.state.sheet_w);
    assert_eq!(rt.state.right_shown_w(), rp::WIDENED_W.min(room));
    drag_gutter(&mut rt, -1.0);
    assert!(!rt.state.right.widened && rt.state.right.user_sized);
}

#[test]
fn collapsing_and_expanding_use_fleets_labels_and_the_rail_shows_the_plan() {
    let mut rt = open();
    click(&mut rt, "close-content");
    assert!(!rt.state.content_open);
    assert!(rt.find("content-rail").is_some(), "the rail replaces the panel");
    click(&mut rt, "content-rail");
    assert!(rt.state.content_open, "a press on the rail expands");
    assert!(rt.find("content-rail").is_none());
}

#[test]
fn a_narrow_window_draws_the_panel_over_the_conversation_with_no_gutter() {
    let mut rt = app();
    rt.update(|s, _| {
        s.sheet_w = 700.0;
        s.content_open = true;
    });
    rt.frame(FRAME);
    assert!(rt.find("right-gutter").is_none());
}

#[test]
fn the_choice_is_per_session_with_the_last_choice_anywhere_as_the_default() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(Some("a"), true);
    s.set_right_collapsed(Some("b"), false);
    assert_eq!(s.remembered_right("a"), Some(true));
    assert_eq!(s.remembered_right("b"), Some(false));
    assert_eq!(s.remembered_right("c"), Some(false), "a session never toggled follows the last choice");
}

#[test]
fn an_empty_session_starts_collapsed_and_opens_by_itself_when_it_first_gets_something() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(None, false);
    s.note_session_content("e", false);
    assert_eq!(s.remembered_right("e"), Some(true), "nothing to show: collapsed even though the last choice was open");
    s.note_session_content("e", true);
    assert_eq!(s.remembered_right("e"), Some(false), "its first change opens it");
    s.set_right_collapsed(Some("e"), true);
    assert_eq!(s.remembered_right("e"), Some(true), "the session's own choice wins");
}

#[test]
fn a_session_that_already_had_content_on_load_does_not_pop_open() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(None, true);
    s.note_session_content("full", true);
    assert_eq!(s.remembered_right("full"), Some(true), "never seen empty, so no first change");
}
