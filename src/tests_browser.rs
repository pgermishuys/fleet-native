//! Browser steps under a call (component 3, piece O): the card, its head, its order and its live updates, driven headless
//! like the rest of `tests.rs`. Fleet has no test of `BrowserSteps.vue` alone; these cover rows 5.8 to 5.10.
use crate::State;
use crate::fleet::events::{Event, Kind};
use crate::mock::browser::Case;
use crate::tests::{FRAME, app};
use rune::Runtime;
use serde_json::json;

fn shown(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_browser(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn a_call_that_used_the_browser_lists_its_steps_in_order() {
    let rt = shown(Case::Done);
    assert!(rt.find("steps-call-c1").is_some(), "the card");
    let at: Vec<f32> = (1..=4).map(|n| rt.find(&format!("step-call-c1-{n}")).expect("a step").1).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "oldest first: {at:?}");
    // A failed step says why, under its summary; the others don't.
    assert!(rt.find("step-call-c1-4-summary").is_some());
}

#[test]
fn the_head_says_used_and_how_many_once_the_call_is_done() {
    // Headless can't read a drawn text: the wording is `conv::browser::heading`'s (its tests); here, what it is given.
    let rt = shown(Case::Done);
    assert!(rt.find("steps-head-call-c1").is_some());
    assert_eq!(rt.state.browser.count("scene", "call-c1"), 4);
}

#[test]
fn the_head_says_using_and_says_step_for_one_while_the_call_runs() {
    let rt = shown(Case::Running);
    assert!(rt.find("steps-head-call-c1").is_some());
    assert_eq!(rt.state.browser.count("scene", "call-c1"), 1);
    assert!(rt.find("step-call-c1-2").is_none());
}

#[test]
fn a_screenshot_step_shows_its_picture_under_the_step() {
    let rt = shown(Case::Done);
    assert!(rt.find("shot-call-c1-3").is_some(), "the thumbnail of step 3");
    assert!(rt.find("shot-call-c1-1").is_none());
}

#[test]
fn a_call_with_no_steps_draws_no_card() {
    let mut rt = shown(Case::Done);
    rt.update(|s, _| s.browser = Default::default());
    rt.update(|s, _| s.sync_open());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("steps-call-c1").is_none());
    assert!(rt.find("tool-c1").is_some(), "the call's row stays");
}

#[test]
fn a_step_event_adds_a_step_once_and_the_card_grows() {
    let mut rt = shown(Case::Running);
    let event = |seq: i64| {
        Event::Other(
            Kind::BrowserStep,
            json!({"sessionId": "scene", "seq": seq, "kind": "click", "summary": format!("Step {seq}"), "detail": "click · 10 ms", "ok": true, "callId": "call-c1"}),
        )
    };
    rt.update(|s, tx| s.on_stream_events("scene", vec![event(2), event(2), event(3)], tx));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("step-call-c1-3").is_some(), "the new steps are drawn");
    assert_eq!(rt.state.browser.count("scene", "call-c1"), 3);
    // A step of another session isn't this one's.
    rt.update(|s, tx| {
        let other = Event::Other(Kind::BrowserStep, json!({"sessionId": "elsewhere", "seq": 9, "callId": "call-c1", "ok": true}));
        s.on_stream_events("scene", vec![other], tx)
    });
    assert_eq!(rt.state.browser.count("scene", "call-c1"), 3);
}
