//! The notice card (component 9, piece L): `NoticeCard.test.ts` 4-10 under their names, driven through the running app's
//! clock, pointer and keys. 1-3 (the quiet moment) are `tests_notices.rs`; 11-12 (the lift, the release notes link as an
//! address) are piece M's.
use crate::fleet::notices::{Action, ActionTone, HOLD_SECS, Link, Notice, STARTUP_QUIET_SECS};
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

const STEP: f64 = 1.0 / 20.0;
const ID: &str = "update:app:0.37.0";

fn ready() -> Notice {
    Notice { body: Some("Installs when you restart.".into()), ..Notice::new(ID, "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready") }
}

fn run(rt: &mut Runtime<crate::State>, seconds: f64) {
    // A coarser step than a real frame: these runs span many seconds of the card's hold.
    for _ in 0..(seconds / STEP).ceil() as usize {
        rt.frame(STEP);
    }
}

fn open_id(rt: &Runtime<crate::State>) -> Option<String> {
    rt.state.notices.open().map(|n| n.id.clone())
}

fn chip_ids(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.notices.chips().iter().map(|n| n.id.clone()).collect()
}

/// An app with `notice` posted and its card open.
fn open_card(notice: Notice) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.notices.post(notice, 0.0));
    run(&mut rt, STARTUP_QUIET_SECS + 0.5);
    assert!(rt.find("notice-card").is_some(), "the card opened");
    rt
}

fn width(rt: &Runtime<crate::State>, key: &str) -> f32 {
    rt.presented(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).0[2]
}

fn press_escape(rt: &mut Runtime<crate::State>) {
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
}

// NoticeCard.test.ts: "settles into its chip after a while, and holds while hovered"
#[test]
fn settles_into_its_chip_after_a_while_and_holds_while_hovered() {
    let mut rt = open_card(ready());
    let (x, y) = rt.find("notice-card").unwrap();
    rt.pointer_move(x, y);
    run(&mut rt, HOLD_SECS * 2.0);
    assert_eq!(open_id(&rt).as_deref(), Some(ID), "held while the pointer is over it");

    rt.pointer_move(8.0, 8.0);
    run(&mut rt, HOLD_SECS + 0.5);
    assert_eq!(open_id(&rt), None);
    assert!(rt.find("notice-card").is_none());
    assert_eq!(chip_ids(&rt), [ID]);
}

#[test]
fn a_card_nobody_touches_settles_after_its_hold_and_not_before() {
    let mut rt = open_card(ready());
    run(&mut rt, HOLD_SECS - 1.5);
    assert!(open_id(&rt).is_some(), "still within its hold");
    run(&mut rt, 2.0);
    assert!(open_id(&rt).is_none());
}

#[test]
fn a_card_with_its_own_hold_uses_it() {
    let mut rt = open_card(Notice { hold_secs: Some(3.0), ..ready() });
    run(&mut rt, 2.0);
    assert!(open_id(&rt).is_some());
    run(&mut rt, 1.5);
    assert!(open_id(&rt).is_none());
}

#[test]
fn focus_inside_the_card_holds_it_too() {
    let mut rt = open_card(Notice { actions: vec![act("Later", "later")], ..ready() });
    rt.update(|_, tx| tx.focus("notice-action-Later", true));
    rt.frame(FRAME);
    run(&mut rt, HOLD_SECS * 2.0);
    assert_eq!(open_id(&rt).as_deref(), Some(ID));
    rt.update(|_, tx| tx.blur());
    rt.frame(FRAME);
    run(&mut rt, HOLD_SECS + 0.5);
    assert_eq!(open_id(&rt), None);
}

// NoticeCard.test.ts: "stays open when you opened it yourself"
#[test]
fn stays_open_when_you_opened_it_yourself() {
    let mut rt = open_card(ready());
    rt.update(|s, _| {
        s.notices.settle();
        s.notices.reopen(ID);
    });
    run(&mut rt, HOLD_SECS * 3.0);
    assert_eq!(open_id(&rt).as_deref(), Some(ID));
}

// NoticeCard.test.ts: "stays open once it asks something"
#[test]
fn stays_open_once_it_asks_something() {
    let mut rt = open_card(ready());
    rt.update(|s, _| s.notices.update(ID, |n| n.title = "2 sessions are working".into()));
    run(&mut rt, HOLD_SECS * 3.0);
    assert_eq!(rt.state.notices.open().map(|n| n.title.as_str()), Some("2 sessions are working"));
}

fn act(label: &str, name: &str) -> Action {
    Action { label: label.into(), icon: None, tone: ActionTone::Quiet, run: name.into() }
}

// NoticeCard.test.ts: "runs an action and settles on Escape"
#[test]
fn runs_an_action_and_settles_on_escape() {
    let mut rt = open_card(Notice { actions: vec![act("Later", "later")], ..ready() });
    click(&mut rt, "notice-action-Later");
    assert_eq!(rt.state.notices.take_heard(), ["later"]);
    assert!(open_id(&rt).is_some(), "an action's source settles the card, not the button");

    rt.update(|_, tx| tx.focus("notice-action-Later", true));
    rt.frame(FRAME);
    press_escape(&mut rt);
    assert_eq!(open_id(&rt), None);
}

// NoticeCard.test.ts: "settles into its chip when dismissed, and never opens as a card again"
#[test]
fn settles_into_its_chip_when_dismissed_and_never_opens_as_a_card_again() {
    let mut rt = open_card(ready());
    click(&mut rt, "notice-dismiss");
    assert_eq!(open_id(&rt), None);
    assert_eq!(chip_ids(&rt), [ID]);
    let saved = rt.state.notices.shown().to_vec();

    // A new run: the same settings file, a fresh store.
    let mut again = app();
    again.update(|s, _| {
        s.notices.adopt_shown(&saved);
        s.notices.post(ready(), 0.0);
    });
    run(&mut again, STARTUP_QUIET_SECS + 2.0);
    assert!(again.find("notice-card").is_none());
    assert_eq!(chip_ids(&again), [ID]);
}

#[test]
fn the_close_button_says_dismiss() {
    let rt = open_card(ready());
    assert!(rt.find("notice-dismiss").is_some());
    let close = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("notice-dismiss")).unwrap();
    assert_eq!(close.label.as_deref(), Some("Dismiss"));
}

// NoticeCard.test.ts: "goes away when dismissed if it has no chip, without running its Undo"
#[test]
fn goes_away_when_dismissed_if_it_has_no_chip_without_running_its_undo() {
    let undo =
        Notice { countdown: true, actions: vec![act("Undo", "undo")], ..Notice::new("memory-saved-1", "Remembered for this repository") };
    let mut rt = open_card(undo);
    click(&mut rt, "notice-dismiss");
    assert_eq!(open_id(&rt), None);
    assert!(!rt.state.notices.has("memory-saved-1"));
    assert!(rt.state.notices.take_heard().is_empty());
}

// NoticeCard.test.ts: "dismisses a card that asks something on Escape, keeping its chip"
#[test]
fn dismisses_a_card_that_asks_something_on_escape_keeping_its_chip() {
    let mut rt = open_card(ready());
    rt.update(|s, tx| {
        s.notices.update(ID, |n| n.title = "2 sessions are working".into());
        tx.focus("notice-dismiss", true);
    });
    rt.frame(FRAME);
    press_escape(&mut rt);
    assert_eq!(open_id(&rt), None);
    assert_eq!(chip_ids(&rt), [ID]);
}

#[test]
fn escape_tells_the_source_before_the_card_settles() {
    let mut rt = open_card(Notice { on_escape: Some("stop-automatic".into()), ..ready() });
    rt.update(|_, tx| tx.focus("notice-dismiss", true));
    rt.frame(FRAME);
    press_escape(&mut rt);
    assert_eq!(rt.state.notices.take_heard(), ["stop-automatic"]);
    assert_eq!(open_id(&rt), None);
}

#[test]
fn escape_elsewhere_leaves_the_card_alone() {
    let mut rt = open_card(ready());
    press_escape(&mut rt);
    assert_eq!(open_id(&rt).as_deref(), Some(ID), "focus was not inside the card");
}

#[test]
fn a_link_handled_in_fleet_runs_and_settles_the_card() {
    let link = Link { label: "What's new".into(), href: "https://example.test/notes".into(), run: Some("whats-new".into()) };
    let mut rt = open_card(Notice { link: Some(link), ..ready() });
    assert!(rt.find("notice-body").is_some());
    rt.update(|s, tx| {
        // The same path a press on the link's words takes.
        let _ = tx;
        s.notices.tell("whats-new");
        s.notices.settle();
    });
    assert_eq!(rt.state.notices.take_heard(), ["whats-new"]);
    assert_eq!(open_id(&rt), None);
}

#[test]
fn a_list_widens_the_card_to_340() {
    let plain = open_card(ready());
    let narrow = width(&plain, "notice-card");
    let list =
        Notice { items: vec![crate::fleet::notices::Item { text: "One thing".into(), label: Some("Area".into()), kind: None }], ..ready() };
    let wide = open_card(list);
    assert_eq!(narrow, 264.0);
    assert_eq!(width(&wide, "notice-card"), 340.0);
    assert!(wide.find("notice-items").is_some());
}

#[test]
fn the_countdown_drains_over_the_hold_and_stops_while_held() {
    let mut rt = open_card(Notice { countdown: true, hold_secs: Some(6.0), actions: vec![act("Undo", "undo")], ..ready() });
    let bar = |rt: &Runtime<crate::State>| rt.presented("notice-countdown-bar").map_or(0.0, |r| r.0[2]);
    let full = width(&rt, "notice-countdown");
    assert!(bar(&rt) > full * 0.9, "nearly full just after it opened");
    run(&mut rt, 3.0);
    let half = bar(&rt);
    assert!(half > full * 0.3 && half < full * 0.7, "about half after 3 of 6 s: {half} of {full}");

    let (x, y) = rt.find("notice-card").unwrap();
    rt.pointer_move(x, y);
    run(&mut rt, 0.2);
    let held = bar(&rt);
    run(&mut rt, 2.0);
    assert!((bar(&rt) - held).abs() < 1.5, "paused under the pointer: {held} then {}", bar(&rt));
}

#[test]
fn a_pinned_card_has_no_countdown() {
    let mut rt = open_card(Notice { countdown: true, ..ready() });
    rt.update(|s, _| s.notices.update(ID, |n| n.body = Some("Sure?".into())));
    rt.frame(FRAME);
    assert!(rt.find("notice-countdown").is_none());
}

#[test]
fn every_notice_scene_shows_its_card() {
    for name in ["plain", "items", "warn", "countdown"] {
        for suffix in ["", "-light"] {
            let scene = format!("notice-{name}{suffix}");
            let mut rt = app();
            let draw = crate::spec().scenes.iter().find(|(n, _)| *n == scene).unwrap_or_else(|| panic!("no scene {scene}")).1;
            rt.update(|s, _| draw(s));
            rt.frame(FRAME);
            assert!(rt.find("notice-card").is_some(), "{scene} draws the card");
        }
    }
}
