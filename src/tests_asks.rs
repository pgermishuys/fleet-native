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
    rt.update(|s, _| s.asks.clear());
    rt.frame(FRAME);
    assert!(!has(&rt, "permission-card"));
}

#[test]
fn opens_permission_settings() {
    let mut rt = shown(Case::Command);
    click(&mut rt, "permission-settings");
    assert_eq!(rt.state.settings_section, Some("permissions"));
    let mut rt = shown(Case::Handoff);
    click(&mut rt, "permission-settings");
    assert_eq!(rt.state.settings_section, Some("machines"));
}
