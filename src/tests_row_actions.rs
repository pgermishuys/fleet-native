//! A session row's rename in place, and the two questions its menu asks (piece H of docs/parity/06-session-list.md):
//! `SessionItem.test.ts` ("renames in place on double click"), `use-session-actions.test.ts` ("optimistically renames and rolls
//! back on failure"), `ConfirmAction.test.ts` and `SessionItemClearAttachments.test.ts`, under their names, and the rest from
//! the source where Fleet has no test. The invented list answers a change at once, so no server is needed.
use crate::fleet::actions::{Call, attachments_path, failure_text, rename_body, session_path};
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::json;

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn type_title(rt: &mut Runtime<crate::State>, title: &str) {
    press(rt, Key::Text(title.into()));
}

fn right_click(rt: &mut Runtime<crate::State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn double_click(rt: &mut Runtime<crate::State>, key: &str) {
    click(rt, key);
    click(rt, key);
    rt.frame(FRAME);
}

fn title(rt: &Runtime<crate::State>, p: usize, i: usize) -> String {
    rt.state.projects[p].sessions[i].title.clone()
}

fn id(rt: &Runtime<crate::State>, p: usize, i: usize) -> String {
    rt.state.projects[p].sessions[i].id.clone()
}

fn focused(rt: &Runtime<crate::State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

fn start(rt: &mut Runtime<crate::State>, p: usize, i: usize) {
    double_click(rt, &format!("session-{p}-{i}"));
    assert!(rt.state.row_renaming(p, i), "the rename started");
}

// ---- the requests ---------------------------------------------------------------------------------------------------

#[test]
fn the_requests_are_the_ones_fleet_sends() {
    assert_eq!(rename_body("Docs"), json!({ "title": "Docs" }));
    assert_eq!(session_path("s1"), "/api/sessions/s1");
    assert_eq!(attachments_path("s1"), "/api/sessions/s1/attachments");
}

#[test]
fn a_refusal_says_what_the_server_said_else_the_status() {
    let clear = Call::ClearAttachments { id: "s1".into() };
    assert_eq!(failure_text(&clear, 409, r#"{"error":"The session is busy"}"#), "The session is busy");
    assert_eq!(failure_text(&clear, 500, "{}"), "Failed to clear attachments: HTTP 500");
    assert_eq!(failure_text(&clear, 502, "not json"), "Failed to clear attachments: HTTP 502");
    assert_eq!(failure_text(&clear, 500, r#"{"error":""}"#), "Failed to clear attachments: HTTP 500");
    let delete = Call::Delete { id: "s1".into() };
    assert_eq!(failure_text(&delete, 404, r#"{"detail":"No such session"}"#), "No such session");
    assert_eq!(failure_text(&delete, 500, ""), "HTTP 500");
}

// ---- rename ----------------------------------------------------------------------------------------------------------------

#[test]
fn sessionitem_renames_in_place_on_double_click() {
    let mut rt = app();
    let raw = title(&rt, 0, 1);
    start(&mut rt, 0, 1);
    rt.frame(FRAME);
    assert!(rt.find("session-rename").is_some(), "the field is where the title was (aria-label 'Session name')");
    assert!(rt.find("session-title-0-1").is_none());
    assert_eq!(rt.state.act.edit.text, raw);
    assert_eq!(rt.state.act.edit.range(), (0, raw.len()), "all of it selected, so typing replaces it");
    assert!(rt.state.act.edit.focused, "and focused");
}

#[test]
fn an_editing_row_has_no_status_word_and_no_hover() {
    let mut rt = app();
    start(&mut rt, 0, 1);
    for kind in ["status", "running-chip", "pr", "progress"] {
        assert!(rt.find(&format!("session-{kind}-0-1")).is_none(), "{kind} is gone while renaming");
    }
    assert!(rt.find("session-0-1").is_some(), "the row keeps its place");
}

#[test]
fn f2_on_a_focused_row_starts_the_rename() {
    let mut rt = app();
    rt.update(|_, tx| tx.focus("session-0-2", true));
    rt.frame(FRAME);
    assert_eq!(focused(&rt).as_deref(), Some("session-0-2"), "a row takes focus");
    press(&mut rt, Key::Function(2));
    assert!(rt.state.row_renaming(0, 2));
}

#[test]
fn enter_commits_the_trimmed_title_at_once() {
    let mut rt = app();
    start(&mut rt, 0, 1);
    type_title(&mut rt, "  Grove notes  ");
    press(&mut rt, Key::Enter);
    assert!(!rt.state.row_renaming(0, 1), "edit mode closes");
    assert_eq!(title(&rt, 0, 1), "Grove notes");
    assert!(rt.find("session-rename").is_none());
    assert_eq!(focused(&rt).as_deref(), Some("session-0-1"), "focus is back on the row");
}

#[test]
fn escape_cancels_and_changes_nothing() {
    let mut rt = app();
    let before = title(&rt, 0, 1);
    start(&mut rt, 0, 1);
    type_title(&mut rt, "Grove notes");
    press(&mut rt, Key::Escape);
    assert!(!rt.state.row_renaming(0, 1));
    assert_eq!(title(&rt, 0, 1), before);
}

#[test]
fn an_empty_or_unchanged_title_closes_the_edit_and_changes_nothing() {
    let mut rt = app();
    let before = title(&rt, 0, 1);
    start(&mut rt, 0, 1);
    press(&mut rt, Key::Enter);
    assert_eq!(title(&rt, 0, 1), before);
    start(&mut rt, 0, 1);
    type_title(&mut rt, "   ");
    press(&mut rt, Key::Enter);
    assert_eq!(title(&rt, 0, 1), before);
    // The old title with spaces around it is the same title.
    start(&mut rt, 0, 1);
    type_title(&mut rt, &format!("  {before} "));
    press(&mut rt, Key::Enter);
    assert_eq!(title(&rt, 0, 1), before);
}

#[test]
fn leaving_the_field_commits_the_title() {
    let mut rt = app();
    start(&mut rt, 0, 1);
    type_title(&mut rt, "Grove notes");
    click(&mut rt, "filter");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!rt.state.row_renaming(0, 1), "blur ended the edit");
    assert_eq!(title(&rt, 0, 1), "Grove notes");
}

#[test]
fn only_the_first_end_of_an_edit_counts() {
    let mut rt = app();
    start(&mut rt, 0, 1);
    type_title(&mut rt, "Grove notes");
    press(&mut rt, Key::Enter);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(title(&rt, 0, 1), "Grove notes");
    assert!(rt.state.act.rename.is_none());
    // Escape after an Enter is nothing either.
    press(&mut rt, Key::Escape);
    assert_eq!(title(&rt, 0, 1), "Grove notes");
}

#[test]
fn the_field_starts_from_the_raw_title_not_the_untitled_fallback() {
    let mut rt = app();
    rt.update(|s, _| s.projects[0].sessions[1].title = "   ".into());
    rt.frame(FRAME);
    start(&mut rt, 0, 1);
    assert_eq!(rt.state.act.edit.text, "   ");
    press(&mut rt, Key::Escape);
    assert_eq!(title(&rt, 0, 1), "   ", "nothing was saved");
}

#[test]
fn starting_on_another_row_saves_the_first() {
    let mut rt = app();
    start(&mut rt, 0, 1);
    type_title(&mut rt, "First");
    rt.update(|s, tx| s.start_row_rename(0, 2, tx));
    rt.frame(FRAME);
    assert_eq!(title(&rt, 0, 1), "First");
    assert!(rt.state.row_renaming(0, 2) && !rt.state.row_renaming(0, 1));
}

#[test]
fn the_menus_rename_starts_it_and_closes_the_menu() {
    let mut rt = app();
    right_click(&mut rt, "session-0-1");
    assert!(rt.find("ctx-menu").is_some());
    click(&mut rt, "session-context-rename");
    assert!(rt.state.row_renaming(0, 1));
    assert!(rt.find("ctx-menu").is_none(), "the menu closed before the field opened");
}

#[test]
fn useactions_optimistically_renames_and_rolls_back_on_failure() {
    let mut rt = app();
    let (row_id, was) = (id(&rt, 0, 1), title(&rt, 0, 1));
    // The title shows at once; a refusal that comes later puts the old one back and says nothing.
    rt.update(|s, tx| {
        s.set_title(&row_id, "Renamed session");
        s.session_call_done(
            &Call::Rename { id: row_id.clone(), title: "Renamed session".into() },
            Some(was.clone()),
            Err("rename failed".into()),
            tx,
        );
    });
    assert_eq!(title(&rt, 0, 1), was);
    assert!(rt.state.ui.toast.is_none(), "Fleet swallows the error");
    // Not over a title that was changed since.
    rt.update(|s, tx| {
        s.set_title(&row_id, "Typed since");
        s.session_call_done(&Call::Rename { id: row_id.clone(), title: "Renamed session".into() }, Some(was.clone()), Err("x".into()), tx);
    });
    assert_eq!(title(&rt, 0, 1), "Typed since");
}

// ---- the two questions -----------------------------------------------------------------------------------------------------

fn ask_from_menu(rt: &mut Runtime<crate::State>, p: usize, i: usize, item: &str) {
    right_click(rt, &format!("session-{p}-{i}"));
    click(rt, item);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn sessionitem_clear_attachments_is_in_the_menu() {
    let mut rt = app();
    right_click(&mut rt, "session-0-1");
    assert!(rt.find("session-context-clear-attachments").is_some());
    assert!(rt.state.ui.confirm.is_none(), "the menu item asks, it doesn't act");
}

#[test]
fn sessionitem_clear_attachments_asks_first_then_clears_the_sessions_attachments() {
    let mut rt = app();
    let name = title(&rt, 0, 1);
    ask_from_menu(&mut rt, 0, 1, "session-context-clear-attachments");
    let c = rt.state.ui.confirm.as_ref().expect("a question");
    assert_eq!(c.title, "Clear attachments?");
    assert_eq!(c.subject, name);
    assert_eq!(c.body, "This deletes the files attached to this session. The agent can't read them again. The conversation stays.");
    assert_eq!(c.confirm_label, "Clear attachments");
    assert_eq!(c.call, Call::ClearAttachments { id: id(&rt, 0, 1) });
    assert!(rt.find("ctx-menu").is_none(), "the menu closed before the dialog opened");
    assert!(rt.find("confirm-dialog").is_some());
    click(&mut rt, "confirm-action");
    assert!(rt.state.ui.confirm.is_none(), "taken: it closes");
    assert_eq!(title(&rt, 0, 1), name, "the conversation stays");
}

#[test]
fn sessionitem_clear_attachments_keeps_the_dialog_open_and_says_why_when_the_server_refuses() {
    let mut rt = app();
    ask_from_menu(&mut rt, 0, 1, "session-context-clear-attachments");
    rt.update(|s, tx| {
        s.ui.confirm.as_mut().unwrap().busy = true;
        s.confirm_done(Err("The session is archived".into()), tx);
    });
    rt.frame(FRAME);
    assert!(rt.find("confirm-dialog").is_some(), "still open");
    assert!(rt.find("confirm-error").is_some(), "with the reason");
    assert_eq!(rt.state.ui.confirm.as_ref().unwrap().error.as_deref(), Some("The session is archived"));
    assert!(!rt.state.ui.confirm.as_ref().unwrap().busy, "the buttons are back, to try again or cancel");
}

#[test]
fn sessionitem_delete_permanently_asks_first_naming_the_session_then_deletes_it() {
    let mut rt = app();
    let (name, gone) = (title(&rt, 0, 1), id(&rt, 0, 1));
    let rows = rt.state.projects[0].sessions.len();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    let c = rt.state.ui.confirm.as_ref().expect("a question");
    assert_eq!((c.title.as_str(), c.subject.as_str()), ("Delete this session?", name.as_str()));
    assert_eq!(c.body, "It and its session data are deleted for good.");
    assert_eq!(c.confirm_label, "Delete session");
    assert_eq!(rt.state.projects[0].sessions.len(), rows, "nothing is deleted before the answer");
    click(&mut rt, "confirm-action");
    assert!(rt.state.ui.confirm.is_none());
    assert_eq!(rt.state.projects[0].sessions.len(), rows - 1, "the row left the list");
    assert!(rt.state.row_of(&gone).is_none());
}

#[test]
fn deleting_the_open_session_opens_another() {
    let mut rt = app();
    let crate::fleet::store::Route::Session(open) = rt.state.route.clone() else { panic!("a session is open") };
    let (p, i) = rt.state.row_of(&open).unwrap();
    ask_from_menu(&mut rt, p, i, "session-context-delete");
    click(&mut rt, "confirm-action");
    assert!(rt.state.row_of(&open).is_none());
    assert!(rt.state.has_session(), "the window isn't left with a session that is gone");
}

#[test]
fn sessionitem_delete_permanently_keeps_the_dialog_open_and_says_why_when_the_delete_fails() {
    let mut rt = app();
    let rows = rt.state.projects[0].sessions.len();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    rt.update(|s, tx| {
        s.ui.confirm.as_mut().unwrap().busy = true;
        s.confirm_done(Err("Session is running".into()), tx);
    });
    rt.frame(FRAME);
    assert!(rt.find("confirm-error").is_some());
    assert_eq!(rt.state.projects[0].sessions.len(), rows, "the session is still listed");
}

// ---- ConfirmAction ---------------------------------------------------------------------------------------------------------

#[test]
fn confirmaction_draws_the_question_the_subject_what_happens_and_a_button_that_says_the_action() {
    let mut rt = app();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    for key in ["confirm-dialog", "confirm-subject", "confirm-body", "confirm-cancel", "confirm-action"] {
        assert!(rt.find(key).is_some(), "{key} is drawn");
    }
    assert!(rt.find("confirm-close").is_none(), "an alert has no close button");
}

#[test]
fn confirmaction_resolves_false_on_cancel() {
    let mut rt = app();
    let rows = rt.state.projects[0].sessions.len();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    click(&mut rt, "confirm-cancel");
    assert!(rt.state.ui.confirm.is_none());
    assert_eq!(rt.state.projects[0].sessions.len(), rows);
}

#[test]
fn confirmaction_resolves_false_on_esc() {
    let mut rt = app();
    let rows = rt.state.projects[0].sessions.len();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    press(&mut rt, Key::Escape);
    assert!(rt.state.ui.confirm.is_none());
    assert_eq!(rt.state.projects[0].sessions.len(), rows);
}

// "starts focus on Cancel for a destructive action, so Enter doesn't take it by accident" and "starts focus on the action when it
// isn't destructive": the button is marked `autofocus` (`ui::confirm::view`); where the modal puts focus on open can't be read
// headless (a headless modal focuses itself), so it is looked at in a window (the pull request says how).

#[test]
fn confirmaction_a_second_ask_cancels_the_first() {
    let mut rt = app();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    rt.update(|s, tx| s.ask_clear_attachments(0, 2, tx));
    rt.frame(FRAME);
    let c = rt.state.ui.confirm.as_ref().unwrap();
    assert_eq!(c.title, "Clear attachments?", "one at a time, the newer one");
}

#[test]
fn confirmaction_holds_the_dialog_open_and_busy_while_the_action_runs() {
    let mut rt = app();
    rt.update(|s, _| s.show_confirm(true, 0, 1, true, None));
    rt.frame(FRAME);
    rt.frame(FRAME);
    press(&mut rt, Key::Escape);
    assert!(rt.state.ui.confirm.is_some(), "Esc does nothing while it runs");
    click(&mut rt, "confirm-cancel");
    assert!(rt.state.ui.confirm.is_some(), "neither does Cancel");
    let busy = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("confirm-action")).expect("the action");
    assert!(!busy.focusable, "the action waits");
    assert!(rt.state.ui.confirm.as_ref().unwrap().busy);
}

#[test]
fn confirmaction_keeps_the_dialog_open_with_the_error_when_the_action_fails_to_try_again_or_cancel() {
    let mut rt = app();
    let rows = rt.state.projects[0].sessions.len();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    rt.update(|s, tx| {
        s.ui.confirm.as_mut().unwrap().busy = true;
        s.confirm_done(Err("nope".into()), tx);
    });
    rt.frame(FRAME);
    assert!(rt.find("confirm-error").is_some());
    // Try again: the error clears while it runs, and the answer closes it.
    click(&mut rt, "confirm-action");
    assert!(rt.state.ui.confirm.is_none(), "the retry went through");
    assert_eq!(rt.state.projects[0].sessions.len(), rows - 1);
}

#[test]
fn confirmaction_resolves_false_when_cancelled_after_a_failure() {
    let mut rt = app();
    let rows = rt.state.projects[0].sessions.len();
    ask_from_menu(&mut rt, 0, 1, "session-context-delete");
    rt.update(|s, tx| {
        s.ui.confirm.as_mut().unwrap().busy = true;
        s.confirm_done(Err("nope".into()), tx);
    });
    rt.frame(FRAME);
    click(&mut rt, "confirm-cancel");
    assert!(rt.state.ui.confirm.is_none());
    assert_eq!(rt.state.projects[0].sessions.len(), rows);
}
