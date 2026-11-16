//! The New session draft's row in the session list (`DraftSessionRow.vue`, rows 2.70-2.79 of docs/parity/06-session-list.md).
//! Fleet has no test for the component; these are written from its code. The page's own cases are in `tests_new_session.rs`.
use crate::tests::{FRAME, app, click};

#[test]
fn the_draft_row_has_the_sessions_row_box_and_says_draft_then_starting() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    let row = rt.state.new_draft.row().expect("the page has a draft");
    assert_eq!((row.title.as_str(), row.is_starting), ("", false), "empty: the row reads New session / Draft");
    assert!(rt.find("new-session-draft-row").is_some(), "the page being open is what draws the empty row");
    rt.state.new_draft.draft_mut().unwrap().is_starting = true;
    rt.frame(FRAME);
    assert!(rt.state.new_draft.row().unwrap().is_starting, "Starting… once sent");
}

#[test]
fn the_draft_row_is_active_only_while_the_page_is_open() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    assert!(rt.state.on_new_session_page());
    rt.state.new_draft.draft_mut().unwrap().message = "Fix it".into();
    click(&mut rt, "session-0-1");
    assert!(!rt.state.on_new_session_page());
    assert!(rt.find("new-session-draft-row").is_some());
}

#[test]
fn the_session_started_from_a_draft_keeps_the_drafts_row_key() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    let key = rt.state.new_draft.row().unwrap().key;
    assert!(!key.is_empty());
    rt.state.new_draft.draft_mut().unwrap().message = "Fix it".into();
    rt.state.new_draft.hand_off("ses-new");
    assert_eq!(rt.state.new_draft.row_key_of("ses-new"), Some(key.as_str()));
    assert!(rt.state.new_draft.row().is_none(), "the draft's own row is gone: the session's row takes its place");
}
