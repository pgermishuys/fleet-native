//! The question card driven headless over its scenes (`components/session/__tests__/QuestionCard.test.ts`, the drawing
//! half that piece C of docs/parity/04-questions-and-permissions.md ports). The rules are `asks::tests_question`.
use crate::mock::questions::Case;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods};

fn shown(case: Case) -> rune::Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_question(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &rune::Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn picked(rt: &rune::Runtime<crate::State>, qi: usize) -> Vec<String> {
    rt.state.questions.cards[0].sel.picked[qi].clone()
}

#[test]
fn asks_for_an_answer_and_marks_itself_as_needing_you() {
    let rt = shown(Case::Single);
    for key in [
        "question-card",
        "question-heading",
        "question-needs-you",
        "question-text-0",
        "question-pill-SQLite",
        "question-pill-Redis",
        "question-other-0",
        "question-other-input-0",
        "question-submit-button",
        "question-dismiss-button",
        "question-key-hint",
    ] {
        assert!(has(&rt, key), "no {key}");
    }
    assert!(!has(&rt, "question-hint-0") && !has(&rt, "question-subhead-1"));
}

#[test]
fn sends_the_picked_choice() {
    // The selection half: a click picks, and what would be sent is that label.
    let mut rt = shown(Case::Single);
    click(&mut rt, "question-pill-Redis");
    assert_eq!(picked(&rt, 0), ["Redis"]);
    assert_eq!(rt.state.questions.cards[0].answers(), vec![vec!["Redis".to_string()]]);
    click(&mut rt, "question-pill-SQLite");
    assert_eq!(picked(&rt, 0), ["SQLite"], "a second pick replaces the first");
    click(&mut rt, "question-pill-SQLite");
    assert!(picked(&rt, 0).is_empty() && !rt.state.questions.cards[0].has_any_answer(), "a picked option again unpicks it");
}

#[test]
fn sends_typed_text_instead_of_a_choice() {
    let mut rt = shown(Case::Single);
    click(&mut rt, "question-pill-Redis");
    click(&mut rt, "question-other-0");
    rt.key(KeyEvent { key: Key::Text("Postgres".into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.questions.cards[0].typed[0].text, "Postgres");
    assert!(picked(&rt, 0).is_empty(), "typing drops the pick");
    assert_eq!(rt.state.questions.cards[0].answers(), vec![vec!["Postgres".to_string()]]);
    // And picking drops what was typed.
    click(&mut rt, "question-pill-Redis");
    assert_eq!(rt.state.questions.cards[0].typed[0].text, "");
    assert_eq!(rt.state.questions.cards[0].answers(), vec![vec!["Redis".to_string()]]);
}

#[test]
fn a_multiple_question_keeps_every_pick_and_says_so() {
    let mut rt = shown(Case::Multiple);
    assert!(has(&rt, "question-hint-0"), "Pick any that apply.");
    assert_eq!(picked(&rt, 0), ["Unit tests", "Lint"]);
    click(&mut rt, "question-pill-Type check");
    assert_eq!(picked(&rt, 0), ["Unit tests", "Lint", "Type check"]);
    click(&mut rt, "question-pill-Lint");
    assert_eq!(picked(&rt, 0), ["Unit tests", "Type check"]);
}

#[test]
fn two_questions_stack_with_their_sub_headings() {
    let mut rt = shown(Case::Two);
    assert!(has(&rt, "question-subhead-1") && !has(&rt, "question-subhead-0"), "only a later question has a sub-heading");
    click(&mut rt, "question-pill-1 hour");
    assert_eq!(picked(&rt, 1), ["1 hour"]);
    assert!(picked(&rt, 0).is_empty(), "each question keeps its own pick");
    assert!(rt.state.questions.cards[0].has_any_answer());
}

#[test]
fn a_question_that_takes_no_typed_answer_has_no_field() {
    let rt = shown(Case::NoCustom);
    assert!(!has(&rt, "question-other-0") && has(&rt, "question-pill-Redis"));
}

#[test]
fn keeps_the_line_breaks_agents_put_in_the_question_and_option_descriptions() {
    // The text keeps its "\n" all the way to the drawn run; the picture shows them as lines.
    let rt = shown(Case::Long);
    let card = &rt.state.questions.cards[0];
    assert!(card.input.questions[0].question.contains("\n\nTwo things decide it:\n- cost per build"));
    assert!(card.input.questions[0].options[0].description.contains("Costs more.\nQueue time"));
    assert!(has(&rt, "question-text-0"));
}

#[test]
fn a_card_is_only_built_when_there_is_a_question() {
    let mut rt = shown(Case::Single);
    assert!(has(&rt, "question-card"));
    rt.update(|s, _| s.questions.cards.clear());
    rt.frame(FRAME);
    assert!(!has(&rt, "question-card"));
}
