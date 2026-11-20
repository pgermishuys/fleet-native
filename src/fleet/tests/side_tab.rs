//! Component 15, piece D: the tab, the peek and the Undo toast. The view halves of `SideConversationPanel.test.ts`' "minimizes into
//! a tab, keeps the conversation mounted, and saves it on the server", "comes back minimized after a reload", "says Thinking while
//! it works, then New answer when one lands while it's folded, until it's opened", "discards at once, offers Undo, and Undo brings
//! it back as it was", "stops offering Undo after its window" and "offers Undo again for the time the server says is left"; the
//! state halves are in `side.rs`.
use super::side::{Fx, side_json};
use crate::State;
use crate::fleet::side::Progress;
use crate::fleet::side_tab::{TAB_ENTER_SECS, Word, dot_opacity};
use crate::tests::{FRAME, click};
use rune::Runtime;

fn run(rt: &mut Runtime<State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

fn drawn(rt: &Runtime<State>, key: &str) -> bool {
    rt.presented(key).is_some()
}

fn opacity(rt: &Runtime<State>, key: &str) -> f32 {
    rt.presented(key).map(|p| p.1).unwrap_or_else(|| panic!("{key} is drawn"))
}

fn report(fx: &mut Fx, working: bool, answer: &str, id: &str) {
    let (session, answer, id) = (fx.id.clone(), answer.to_string(), id.to_string());
    fx.rt.update(move |s, _| {
        s.report_side_progress(&session, Progress { working, latest_answer: Some(answer), latest_answer_id: Some(id) })
    });
    fx.rt.frame(FRAME);
}

fn word(fx: &Fx) -> Word {
    fx.st().word()
}

// "minimizes into a tab, keeps the conversation mounted, and saves it on the server"
#[test]
fn minimizes_into_a_tab_and_opens_from_it() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    assert!(!drawn(&fx.rt, "side-tab"), "no tab while the card is open");
    let session = fx.id.clone();
    fx.rt.update(move |s, tx| s.set_side_minimized(&session, true, tx));
    run(&mut fx.rt, 0.4);
    assert!(drawn(&fx.rt, "side-tab") && drawn(&fx.rt, "side-open") && drawn(&fx.rt, "side-tab-close"));
    assert!(fx.rt.presented("side-fold").is_some_and(|p| p.0[3] < 2.0), "the card has folded away");
    click(&mut fx.rt, "side-open");
    assert!(!fx.st().minimized(), "a click on the tab opens the panel");
    run(&mut fx.rt, 0.4);
    assert!(!drawn(&fx.rt, "side-tab"));
}

// "comes back minimized after a reload"
#[test]
fn comes_back_minimized_as_a_tab() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    fx.rt.frame(FRAME);
    assert_eq!(fx.rt.state.side_open_share(), 0.0, "found folded when it loads: no card to fold away");
    run(&mut fx.rt, 0.4);
    assert!(drawn(&fx.rt, "side-tab"), "found folded: the tab, no card");
    assert!(opacity(&fx.rt, "side-tab") > 0.99, "settled after its 260 ms entrance");
}

#[test]
fn the_tab_enters_over_260_ms() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    let session = fx.id.clone();
    fx.rt.update(move |s, tx| s.set_side_minimized(&session, true, tx));
    fx.rt.frame(FRAME);
    fx.rt.frame(FRAME);
    assert!(fx.rt.state.side_tab_enter().is_some(), "entering");
    run(&mut fx.rt, TAB_ENTER_SECS + 0.1);
    assert!(fx.rt.state.side_tab_enter().is_none(), "arrived");
}

// "says Thinking while it works, then New answer when one lands while it's folded, until it's opened"
#[test]
fn the_tab_says_thinking_then_new_answer_with_its_dot_until_its_opened() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    report(&mut fx, false, "The first answer.", "a1");
    let session = fx.id.clone();
    fx.rt.update(move |s, tx| s.set_side_minimized(&session, true, tx));
    run(&mut fx.rt, 0.4);
    assert_eq!(word(&fx), Word::Answered);
    assert!(!drawn(&fx.rt, "side-tab-dots") && !drawn(&fx.rt, "side-tab-unread"));
    report(&mut fx, true, "The first answer.", "a1");
    assert_eq!(word(&fx), Word::Thinking);
    assert!(drawn(&fx.rt, "side-tab-dots"), "three dots while it works");
    report(&mut fx, false, "The second answer.", "a2");
    assert_eq!(word(&fx), Word::NewAnswer { pulse: true });
    assert!(drawn(&fx.rt, "side-tab-unread") && !drawn(&fx.rt, "side-tab-dots"));
    assert!(fx.rt.state.side_pulse_phase().is_some(), "the dot pulses for an answer that landed on this run");
    run(&mut fx.rt, 3.3);
    assert!(fx.rt.state.side_pulse_phase().is_none(), "twice, then it stops");
    assert_eq!(word(&fx), Word::NewAnswer { pulse: true }, "still unread");
    click(&mut fx.rt, "side-open");
    run(&mut fx.rt, 0.4);
    assert!(!drawn(&fx.rt, "side-tab"));
}

#[test]
fn a_new_answer_found_after_a_restart_does_not_pulse() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    let session = fx.id.clone();
    fx.rt.update(move |s, _| {
        s.report_side_progress(
            &session,
            Progress { working: false, latest_answer: Some("Found.".into()), latest_answer_id: Some("a9".into()) },
        )
    });
    run(&mut fx.rt, 0.4);
    assert_eq!(word(&fx), Word::NewAnswer { pulse: false });
    assert!(fx.rt.state.side_pulse_phase().is_none());
}

#[test]
fn the_dots_blink_a_fifth_of_a_second_apart() {
    assert!((dot_opacity(0.0, 0) - 0.25).abs() < 1e-4);
    assert!((dot_opacity(0.48, 0) - 1.0).abs() < 1e-3, "full at 40 % of 1.2 s");
    assert!((dot_opacity(0.2, 1) - 0.25).abs() < 1e-4, "the second starts .2 s later");
    assert!((dot_opacity(1.1, 0) - 0.25).abs() < 1e-4, "low again from 80 %");
    assert!((dot_opacity(0.68, 1) - 1.0).abs() < 1e-3);
}

#[test]
fn says_nothing_for_a_tab_with_no_answer() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    run(&mut fx.rt, 0.4);
    assert_eq!(word(&fx), Word::Nothing);
    assert_eq!(fx.st().peek_text(), "No answer yet.");
    assert!(!drawn(&fx.rt, "side-tab-state") || fx.rt.presented("side-tab-state").is_some_and(|p| p.0[2] < 1.0));
}

#[test]
fn the_peek_shows_over_the_tab_on_hover_and_fades_in_160_ms() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    report(&mut fx, false, "It is in the config file.", "a1");
    run(&mut fx.rt, 0.4);
    assert!(!drawn(&fx.rt, "side-peek"), "hidden until the pointer is over the tab");
    let (x, y) = fx.rt.find("side-open").expect("the tab");
    fx.rt.pointer_move(x, y);
    fx.rt.frame(FRAME);
    fx.rt.frame(FRAME);
    assert!(opacity(&fx.rt, "side-peek") < 1.0, "fading in");
    run(&mut fx.rt, 0.2);
    assert!(opacity(&fx.rt, "side-peek") > 0.99);
    let (peek, tab) = (fx.rt.presented("side-peek").unwrap().0, fx.rt.presented("side-tab").unwrap().0);
    assert!(peek[1] + peek[3] <= tab[1] - 7.0, "its bottom edge is 8 px over the tab: {peek:?} {tab:?}");
    assert!((peek[0] - tab[0] + 14.0).abs() < 1.5 || peek[0] > tab[0] - 20.0, "left-aligned with the tab");
    fx.rt.pointer_move(5.0, 5.0);
    run(&mut fx.rt, 0.3);
    assert!(!fx.rt.state.side_ui.tab.hover && fx.rt.state.side_peek_amount() == 0.0, "gone when the pointer leaves");
}

#[test]
fn the_peek_says_thinking_while_it_works() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    report(&mut fx, true, "An earlier answer.", "a1");
    assert_eq!(fx.st().peek_text(), "Thinking about your question\u{2026}");
    report(&mut fx, false, "The answer.", "a2");
    assert_eq!(fx.st().peek_text(), "The answer.");
}

// "discards at once, offers Undo, and Undo brings it back as it was"
#[test]
fn the_toast_offers_undo_and_undo_brings_it_back() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    run(&mut fx.rt, 0.4);
    click(&mut fx.rt, "side-tab-close");
    run(&mut fx.rt, 0.3);
    assert!(!drawn(&fx.rt, "side-tab") && drawn(&fx.rt, "side-undo-toast"), "the tab goes at once and the toast shows");
    assert!(opacity(&fx.rt, "side-undo-toast") > 0.95 && fx.rt.state.side_toast_enter() >= 1.0, "settled after its 180 ms entrance");
    fx.wait("the DELETE", |f| !f.called("DELETE /side").is_empty());
    click(&mut fx.rt, "side-undo");
    fx.wait("the restore", |f| f.st().side.is_some());
    run(&mut fx.rt, 0.4);
    assert!(!drawn(&fx.rt, "side-undo-toast"), "Undo is gone");
    assert!(drawn(&fx.rt, "side-tab"), "back as it was: folded, its tab");
}

#[test]
fn the_toast_counts_down_and_its_bar_drains_over_the_window() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    let session = fx.id.clone();
    fx.rt.update(move |s, _| s.discard_side(&session));
    fx.rt.frame(FRAME);
    let full = fx.rt.presented("side-undo-drain").unwrap().0[2];
    assert!(full > 300.0, "full at the start: {full}");
    run(&mut fx.rt, 4.0);
    let half = fx.rt.presented("side-undo-drain").unwrap().0[2];
    assert!((half / full - 0.5).abs() < 0.05, "half left after 4 of 8 s: {half} of {full}");
    assert_eq!(fx.st().undo_seconds_left(fx.rt.state.clock), Some(4));
}

// "stops offering Undo after its window"
#[test]
fn the_toast_goes_after_its_window() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    let session = fx.id.clone();
    fx.rt.update(move |s, _| s.discard_side(&session));
    run(&mut fx.rt, 0.3);
    assert!(drawn(&fx.rt, "side-undo-toast"));
    fx.rt.frame(8.0);
    fx.rt.frame(FRAME);
    assert!(!drawn(&fx.rt, "side-undo-toast"));
}

// "offers Undo again for the time the server says is left" ("5 s")
#[test]
fn the_toast_shows_the_time_the_server_says_is_left() {
    let mut fx = Fx::start(None, Some((side_json(true, None), 5_000)));
    fx.wait("the toast", |f| f.rt.presented("side-undo-left").is_some());
    let at = fx.rt.state.side_drain();
    assert!(at > 0.9, "its bar starts from what the server says is left, of its own window: {at}");
    assert_eq!(fx.st().undo_seconds_left(fx.rt.state.clock), Some(5));
}

// A press on the tab that starts as the pointer arrives (the peek comes in between the move and the press) still opens it.
#[test]
fn a_press_on_the_tab_after_the_peek_came_in_opens_it() {
    let mut fx = Fx::loaded(Some(side_json(true, None)));
    report(&mut fx, false, "An answer.", "a1");
    run(&mut fx.rt, 0.4);
    let (x, y) = fx.rt.find("side-open").expect("the tab");
    fx.rt.pointer_move(x, y);
    run(&mut fx.rt, 0.3);
    assert!(drawn(&fx.rt, "side-peek"));
    fx.rt.pointer_down(x, y);
    fx.rt.pointer_up(x, y);
    fx.rt.frame(FRAME);
    assert!(!fx.st().minimized(), "opened");
}

#[test]
fn the_fold_settles_to_nothing_on_a_drawn_frame() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    let session = fx.id.clone();
    fx.rt.update(move |s, tx| s.set_side_minimized(&session, true, tx));
    run(&mut fx.rt, 0.31);
    assert_eq!(fx.rt.state.side_open_share(), 0.0);
    assert!(fx.rt.presented("side-fold").is_none_or(|p| p.0[3] < 0.5 || p.1 < 0.01), "nothing of the card is left");
}
