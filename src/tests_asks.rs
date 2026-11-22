//! The permission card driven headless over its scenes (`components/session/__tests__/PermissionCard.test.ts`, the
//! view half that piece A of docs/parity/04-questions-and-permissions.md ports). What the words say is `asks::tests`.
use crate::mock::asks::Case;
use crate::tests::{FRAME, app, click};

fn shown(case: Case) -> rune::Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_ask(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &rune::Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

#[test]
fn shows_the_command_and_that_the_session_needs_you() {
    let rt = shown(Case::Command);
    for key in [
        "permission-card",
        "permission-heading",
        "permission-cmd",
        "permission-needs-you",
        "permission-allow-once",
        "permission-allow-always",
        "permission-deny-input",
        "permission-deny",
        "permission-undo-hint",
        "permission-settings",
    ] {
        assert!(has(&rt, key), "no {key}");
    }
    assert!(has(&rt, "permission-always-code"), "the harness's pattern is named beside Don't ask again");
    assert!(!has(&rt, "permission-subagent"));
}

#[test]
fn shows_an_edits_diff_without_the_file_headers_with_what_it_adds_and_removes() {
    let rt = shown(Case::Edit);
    assert!(has(&rt, "permission-file") && has(&rt, "permission-lines"));
    assert!(has(&rt, "permission-add") && has(&rt, "permission-del"));
    assert!(!has(&rt, "permission-cmd"), "an edit shows a file, not a command");
}

#[test]
fn shows_no_lines_for_a_write_that_changes_nothing() {
    let rt = shown(Case::EditEmpty);
    assert!(has(&rt, "permission-file") && has(&rt, "permission-path"));
    assert!(!has(&rt, "permission-lines") && !has(&rt, "permission-add") && !has(&rt, "permission-del"));
}

#[test]
fn says_when_a_subagent_asked() {
    assert!(has(&shown(Case::Subagent), "permission-subagent"));
}

#[test]
fn anything_else_with_a_title_shows_the_plain_block() {
    let rt = shown(Case::Path);
    assert!(has(&rt, "permission-cmd") && !has(&rt, "permission-file"));
}

#[test]
fn says_what_goes_there_and_under_whose_permission_level_with_always_and_not_for_that_machine() {
    let rt = shown(Case::Handoff);
    for key in
        ["permission-handoff", "permission-handoff-level", "permission-allow-once", "permission-allow-always", "permission-handoff-never"]
    {
        assert!(has(&rt, key), "no {key}");
    }
    assert!(!has(&rt, "permission-deny-input"), "a hand-off has nothing to say to the agent");
    assert!(!has(&rt, "permission-undo-hint"));
}

#[test]
fn a_card_is_only_built_when_there_is_an_ask() {
    let mut rt = shown(Case::Command);
    assert!(has(&rt, "permission-card"));
    rt.update(|s, _| s.asks.set("mock.0.0", Vec::new()));
    rt.frame(FRAME);
    assert!(!has(&rt, "permission-card"));
}

#[test]
fn opens_permission_settings() {
    let mut rt = shown(Case::Command);
    click(&mut rt, "permission-settings");
    assert_eq!(rt.state.settings.page, crate::fleet::settings_nav::Page::Permissions);
    let mut rt = shown(Case::Handoff);
    click(&mut rt, "permission-settings");
    assert_eq!(rt.state.settings.page, crate::fleet::settings_nav::Page::Machines);
}

#[test]
fn the_choices_take_a_press_and_go_when_there_is_no_server() {
    let mut rt = shown(Case::Command);
    click(&mut rt, "permission-allow-once");
    assert!(!has(&rt, "permission-card"), "with no server the card is simply taken away");
}

#[test]
fn a_card_that_failed_to_answer_shows_its_error_line() {
    let mut rt = shown(Case::Command);
    rt.update(|s, _| {
        s.ask_acts.insert("per_1".into(), crate::fleet::retry::Act { busy: false, error: Some("The harness is restarting.".into()) });
    });
    rt.frame(FRAME);
    assert!(has(&rt, "permission-card") && has(&rt, "permission-error"));
}

#[test]
fn the_arrow_rules_wrap_and_know_only_the_buttons() {
    use crate::asks::focus::{next_choice, step_of, takes_focus};
    use rune::Key;
    let two = ["a", "b"];
    assert_eq!(next_choice(&two, "a", 1), Some("b"));
    assert_eq!(next_choice(&two, "b", 1), Some("a"));
    assert_eq!(next_choice(&two, "a", -1), Some("b"));
    assert_eq!(next_choice(&two, "deny", 1), None);
    assert_eq!(
        (step_of(&Key::Down), step_of(&Key::Right), step_of(&Key::Up), step_of(&Key::Left), step_of(&Key::Enter)),
        (Some(1), Some(1), Some(-1), Some(-1), None)
    );
    // PermissionCard.test.ts "leaves focus alone while the user types in the composer", and QuestionCard's "still takes
    // focus from an empty composer that autofocused".
    assert!(!takes_focus(Some("composer"), "composer", 15));
    assert!(takes_focus(Some("composer"), "composer", 0));
    assert!(takes_focus(None, "composer", 0));
}

// Row 8.4: a waiting card doesn't lock the composer; what is typed goes to the draft and the card stays.
#[test]
fn typing_while_a_card_waits_is_the_composers() {
    let mut rt = shown(Case::Command);
    crate::tests::typed(&mut rt, "hello");
    assert_eq!(rt.state.draft.text, "hello");
    assert!(has(&rt, "permission-card"), "the card is still waiting");
}

// ─── PermissionCard terminal codes ──────────────────────────────────────────────

/// Every word the frame draws, and every name and tooltip it gives.
fn drawn(rt: &rune::Runtime<crate::State>) -> String {
    rt.elements().iter().flat_map(|e| [e.text.clone(), e.label.clone(), e.help.clone()]).flatten().collect::<Vec<_>>().join("\n")
}

#[test]
fn shows_a_command_made_from_coloured_output_without_the_codes() {
    let words = drawn(&shown(Case::Coloured));
    assert!(words.contains("echo failed") && words.contains("/repo"), "{words}");
    assert!(!words.contains("[31m") && !words.contains('\u{1b}'), "{words}");
}

#[test]
fn shows_the_file_of_an_edit_ask_without_the_codes() {
    let words = drawn(&shown(Case::ColouredEdit));
    assert!(words.contains("src/app.ts"), "{words}");
    assert!(!words.contains("[1m") && !words.contains('\u{1b}'), "{words}");
}
