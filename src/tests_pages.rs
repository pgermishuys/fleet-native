//! The page-pick card driven headless over its scenes (component 4, piece F). Fleet has no test for the card itself; these are
//! the cases the checklist adds, and `page-choice.test.ts`'s `isChoiceLive` is in `fleet::tests::pages`.
use crate::mock::pages::Case;
use crate::tests::{FRAME, app, click};

fn shown(case: Case) -> rune::Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_page_pick(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn shows_only_while_a_choice_waits_and_is_live() {
    let rt = shown(Case::Waiting);
    assert!(rt.find("page-pick-card").is_some());
    assert!(rt.find("page-pick-open").is_some());
    let rt = shown(Case::Stale);
    assert!(rt.find("page-pick-card").is_none(), "a restart leaves 'waiting' behind with the session idle");
}

#[test]
fn goes_when_the_session_is_no_longer_stopped_on_you() {
    let mut rt = shown(Case::Waiting);
    rt.update(|s, _| s.projects[0].sessions[0].status = crate::mock::Status::Working);
    rt.frame(FRAME);
    assert!(rt.find("page-pick-card").is_none());
}

#[test]
fn hides_for_a_side_conversation() {
    let rt = shown(Case::Waiting);
    let waiting = rt.state.waiting_page();
    assert!(waiting.is_some());
    assert!(crate::view::pagepick::shown(waiting, false).is_some());
    assert!(crate::view::pagepick::shown(waiting, true).is_none());
}

#[test]
fn open_page_sets_the_target_and_opens_the_panel() {
    let mut rt = shown(Case::Waiting);
    assert_eq!(rt.state.page_target, None);
    click(&mut rt, "page-pick-open");
    assert_eq!(rt.state.page_target.as_deref(), Some("page_1"));
    assert!(rt.state.content_open);
}
