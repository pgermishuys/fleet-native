//! Writing into a session's draft from outside its composer, and forgetting a deleted session's drafts (rows 2.160-2.165 of
//! docs/parity/06-session-list.md). `use-draft-state.ts` has no tests at weave-fleet's commit; these are written from its code.
use crate::State;
use crate::composer::drafts::{session_draft_key, side_draft_key, side_main_draft_key, with_reference, with_text};
use crate::tests::{FRAME, app};
use rune::Runtime;

fn open_id(rt: &Runtime<State>) -> String {
    rt.state.draft_owner().expect("an open session")
}

#[test]
fn append_draft_reference_adds_the_reference_and_a_space_with_a_space_before_it_unless_the_text_ends_in_whitespace() {
    assert_eq!(with_reference("", "@a.rs:3"), "@a.rs:3 ");
    assert_eq!(with_reference("look at", "@a.rs:3"), "look at @a.rs:3 ");
    assert_eq!(with_reference("look at ", "@a.rs:3"), "look at @a.rs:3 ");
    assert_eq!(with_reference("look\n", "@a.rs:3"), "look\n@a.rs:3 ");
}

#[test]
fn append_draft_text_makes_a_blank_draft_the_text_and_otherwise_follows_the_old_text_after_a_blank_line() {
    assert_eq!(with_text("", "more"), "more");
    assert_eq!(with_text("  \n", "more"), "more");
    assert_eq!(with_text("first  \n", "more"), "first\n\nmore");
}

#[test]
fn appending_to_the_open_session_changes_the_box_and_to_another_only_its_stored_draft() {
    let mut rt = app();
    let open = open_id(&rt);
    rt.update(|s, _| s.append_draft_text(&open.clone(), "first"));
    rt.update(|s, _| s.append_draft_reference(&open.clone(), "@a.rs:3"));
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "first @a.rs:3 ");
    assert_eq!(rt.state.drafts.text_of(&open), "first @a.rs:3 ", "the box is written through on the next frame");
    rt.update(|s, _| s.append_draft_text("other", "elsewhere"));
    assert_eq!(rt.state.drafts.text_of("other"), "elsewhere");
    assert_eq!(rt.state.draft.text, "first @a.rs:3 ", "the open box is left alone");
    rt.update(|s, _| s.clear_draft_text("other"));
    assert!(rt.state.drafts.keys("session.other").is_empty(), "clearing forgets the key");
    rt.update(|s, _| s.clear_draft_text(&open.clone()));
    rt.frame(FRAME);
    assert_eq!(rt.state.draft.text, "");
    assert_eq!(rt.state.drafts.text_of(&open), "");
}

#[test]
fn a_deleted_sessions_drafts_and_side_drafts_are_forgotten() {
    let mut rt = app();
    let id = "gone";
    rt.update(|s, _| {
        for key in [session_draft_key(id), side_draft_key(id, "x"), side_main_draft_key(id), session_draft_key("kept")] {
            s.drafts.write(&key, "text", 0.0);
        }
        s.forget_drafts_of(id);
    });
    assert_eq!(rt.state.drafts.keys(""), [session_draft_key("kept")]);
}

#[test]
fn deleting_the_open_session_empties_the_box_and_forgets_its_text() {
    let mut rt = app();
    let open = open_id(&rt);
    rt.update(|s, _| s.append_draft_text(&open.clone(), "half a thought"));
    rt.frame(FRAME);
    rt.update(|s, tx| s.forget_session(&open.clone(), tx));
    rt.frame(FRAME);
    assert!(rt.state.drafts.keys(&format!("session.{open}")).is_empty());
    assert_ne!(rt.state.draft.text, "half a thought");
}
