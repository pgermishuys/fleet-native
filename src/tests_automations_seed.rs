//! Component 19, piece O: Repeat on a schedule… from a session. Fleet's O case of `AutomationsPage.test.ts` under its name
//! ("starts “Repeat on a schedule…” from the session's first message and folder"), against a fake Fleet; then what the
//! native app keeps: a failed read is the form error, a worktree session starts a new worktree of its repository, and the
//! row menu's item opens Automations on the draft. Another machine's row has no such item: `tests_machines` covers that.
use crate::fleet::automations::nav::{SEED_FAILED, ViewMode};
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::tests::{FRAME, app, click};
use crate::tests_automations_composer::{KETTLE, page, plan};
use crate::tests_automations_page::Rig;
use crate::{Rail, State};
use serde_json::json;

const SEED: &str = "/api/automations/draft-from-session/s1";

fn focused(r: &Rig) -> Option<String> {
    r.rt.elements().iter().find(|e| e.focused).and_then(|e| e.key.clone())
}

/// `startCreateFromSession("s1")` from the session list, then the wait for the seed's read to land.
fn repeat(r: &mut Rig) {
    r.rt.update(|s, tx| s.repeat_session_on_schedule("s1", None, tx));
    let heard = r.home.handle();
    r.wait("the seed to be asked for", move |_| heard("GET", SEED) > 0);
    r.wait("the seed's answer", |s| s.automations_nav.seed_session.is_none());
    r.frames();
}

fn form_error(r: &Rig) -> String {
    r.texts_of("automation-form-error-text").concat()
}

// ─── AutomationsPage.test.ts: "Automations screen" (the O case) ─────────────────────────────────────────────────────

#[test]
fn starts_repeat_on_a_schedule_from_the_sessions_first_message_and_folder() {
    let mut r = page(vec![], false);
    *r.home.draft.lock().unwrap() = Some((
        "200 X".into(),
        json!({"prompt": "Go through the open pull requests and summarise each one.", "folder": KETTLE, "isolation": "existing"}),
    ));
    repeat(&mut r);

    assert_eq!(r.rt.state.automations_nav.draft_box.text, "Go through the open pull requests and summarise each one.");
    let plan = plan(&r);
    assert!(plan.contains("When should it run?"), "{plan}");
    assert!(plan.contains(&format!("in {KETTLE} as it is.")), "{plan}");
    // Native: the page is Automations in create mode, the seed read once, and the box has the keys.
    assert_eq!((r.rt.state.rail, r.rt.state.automations_nav.mode), (Rail::Automations, ViewMode::Create));
    assert_eq!(r.home.calls("GET", SEED), 1);
    assert_eq!(focused(&r).as_deref(), Some("automation-message"));
}

// ─── What the native app keeps ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_failed_read_is_the_form_error_and_the_draft_stays_fresh() {
    let mut r = page(vec![], false);
    *r.home.draft.lock().unwrap() = Some(("404 X".into(), json!({"error": "That session is gone."})));
    repeat(&mut r);

    assert_eq!(form_error(&r), "That session is gone.");
    assert_eq!(r.rt.state.automations_nav.draft.text, "");
    assert_eq!(r.rt.state.automations_nav.mode, ViewMode::Create);
}

#[test]
fn with_no_fleet_and_no_first_message_the_form_says_it_couldnt_read_that_session() {
    let mut rt = app();
    rt.update(|s, tx| s.repeat_session_on_schedule("s1", None, tx));
    rt.frame(FRAME);
    assert_eq!(rt.state.automations_nav.form_error.as_deref(), Some(SEED_FAILED));
    assert_eq!(SEED_FAILED, "Couldn't read that session.");
}

#[test]
fn a_session_that_ran_in_a_worktree_starts_a_new_worktree_of_its_repository() {
    // The fake's own answer: "Tidy the changelog" in a worktree of kettle.
    let mut r = page(vec![], false);
    repeat(&mut r);

    let draft = &r.rt.state.automations_nav.draft;
    assert_eq!(draft.folder, Some(Folder::Repository(KETTLE.into())));
    assert!(draft.has_chosen_folder);
    assert_eq!(draft.workspace, Workspace::New);
    assert!(plan(&r).contains(&format!("in a new worktree of {KETTLE}")), "{}", plan(&r));
}

#[test]
fn the_row_menus_item_opens_automations_on_the_sessions_first_message() {
    let mut rt = app();
    let first = crate::fleet::automations::nav::invented_draft(&rt.state.projects[0].sessions[1]).expect("an invented first message");
    let (x, y) = rt.find("session-0-1").expect("the row");
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "session-repeat-on-schedule");
    rt.frame(FRAME);
    rt.frame(FRAME);

    let s: &State = &rt.state;
    assert_eq!((s.rail, s.automations_nav.mode), (Rail::Automations, ViewMode::Create));
    assert_eq!(s.automations_nav.draft.text, first.prompt);
    assert!(s.automations_nav.form_error.is_none());
    assert!(rt.find("ctx-menu").is_none(), "the menu closed");
}
