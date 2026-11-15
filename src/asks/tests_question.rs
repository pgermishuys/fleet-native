//! `lib/__tests__/linkify-text.test.ts`, the selection half of `QuestionCard.test.ts` and the reading of
//! `lib/question-types.ts`, under Fleet's names (as snake_case, with its sentence in a comment).
use super::question::*;
use serde_json::{Value, json};

fn call(extra: Value) -> Value {
    let mut state = json!({"status": "pending", "input": {"questions": [{"header": "Database", "question": "Which database should the cache use?",
        "options": [{"label": "SQLite", "description": "Local file"}, {"label": "Redis", "description": "Shared server"}]}]}});
    state.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    state
}

fn input_of(state: &Value) -> QuestionInput {
    question_input("question", state).expect("a question")
}

fn two(multiple: bool) -> QuestionInput {
    let q = |multiple| QuestionInfo {
        header: String::new(),
        question: "Pick".into(),
        options: ["a", "b", "c"].iter().map(|l| QuestionOption { label: l.to_string(), description: String::new() }).collect(),
        multiple,
        custom: true,
    };
    QuestionInput { questions: vec![q(multiple), q(multiple)] }
}

#[test]
fn reads_a_question_call_and_nothing_else() {
    let input = input_of(&call(json!({})));
    assert_eq!(input.questions[0].header, "Database");
    assert!(input.questions[0].custom && !input.questions[0].multiple, "Fleet's defaults: typed answers allowed, one pick");
    assert_eq!(input.questions[0].options[1], QuestionOption { label: "Redis".into(), description: "Shared server".into() });
    assert_eq!(question_input("bash", &call(json!({}))), None, "not the question tool");
    assert_eq!(question_input("question", &json!({"status": "pending"})), None, "no input");
    assert_eq!(question_input("question", &json!({"input": {"questions": "x"}})), None, "a malformed list");
    assert_eq!(question_input("question", &Value::Null), None);
    // The tool registry knows the name in any case and under any alias.
    assert!(question_input("Question", &call(json!({}))).is_some());
}

#[test]
fn says_what_state_a_call_is_in() {
    let status = |s: &str| question_status(&json!({"status": s}));
    assert_eq!(
        [status("pending"), status("running"), status("completed"), status("error"), status("other"), question_status(&Value::Null)],
        [
            QuestionStatus::Pending,
            QuestionStatus::Running,
            QuestionStatus::Completed,
            QuestionStatus::Error,
            QuestionStatus::Unknown,
            QuestionStatus::Unknown
        ]
    );
    assert!(QuestionStatus::Pending.is_active() && QuestionStatus::Running.is_active() && !QuestionStatus::Completed.is_active());
}

#[test]
fn reads_what_a_completed_call_answered() {
    let done = json!({"status": "completed", "metadata": {"answers": [["Redis"], []]}});
    assert_eq!(question_answers("question", &done), Some(vec![vec!["Redis".to_string()], vec![]]));
    assert_eq!(question_answers("question", &json!({"status": "running", "metadata": {"answers": [["x"]]}})), None);
    assert_eq!(question_answers("question", &json!({"status": "completed"})), None);
    let answers = question_answers("question", &done).unwrap();
    assert_eq!((answer_text(answers.first()), answer_text(answers.get(1)), answer_text(answers.get(2))), ("Redis".into(), "No answer".into(), "No answer".into()));
    assert_eq!(answer_text(Some(&vec!["a".to_string(), "b".to_string()])), "a, b");
}

#[test]
fn asks_for_an_answer_and_marks_itself_as_needing_you_hint() {
    // The footer's hint: two choices and "Something else…" make three keys.
    assert_eq!(key_hint(&input_of(&call(json!({})))), "1\u{2013}3 to pick \u{b7} Enter to send");
    // One choice (and no typed answer) leaves nothing to pick between.
    let one = json!({"status": "pending", "input": {"questions": [{"header": "", "question": "Go?", "options": [{"label": "Yes", "description": ""}], "custom": false}]}});
    assert_eq!(key_hint(&input_of(&one)), "Enter to send");
    // Ten options still say 9: only digits 1-9 pick.
    let options: Vec<Value> = (0..10).map(|i| json!({"label": format!("o{i}"), "description": ""})).collect();
    let many = json!({"status": "pending", "input": {"questions": [{"header": "", "question": "Which?", "options": options}]}});
    assert_eq!(key_hint(&input_of(&many)), "1\u{2013}9 to pick \u{b7} Enter to send");
}

#[test]
fn sends_the_picked_choice() {
    // The selection half: a pick is what `submit` sends.
    let input = input_of(&call(json!({})));
    let mut sel = Selection::new(1);
    assert!(!sel.has_any_answer(&[""]));
    assert!(sel.toggle(0, "Redis", false));
    assert!(sel.is_picked(0, "Redis") && !sel.is_picked(0, "SQLite"));
    assert!(sel.has_any_answer(&[""]));
    assert_eq!(sel.answers(input.questions.len(), &[""]), vec![vec!["Redis".to_string()]]);
}

#[test]
fn sends_typed_text_instead_of_a_choice() {
    let mut sel = Selection::new(1);
    sel.toggle(0, "Redis", false);
    // Typing into a single-answer question drops its pick.
    sel.typed(0, false, "  Postgres  ");
    assert!(!sel.is_picked(0, "Redis"));
    assert!(sel.has_any_answer(&["  Postgres  "]));
    assert_eq!(sel.answers(1, &["  Postgres  "]), vec![vec!["Postgres".to_string()]]);
    // Blanks alone are no answer, and don't drop a pick.
    sel.toggle(0, "Redis", false);
    sel.typed(0, false, "   ");
    assert!(sel.is_picked(0, "Redis"));
    let blank = Selection::new(1);
    assert!(!blank.has_any_answer(&["   "]));
}

#[test]
fn a_single_answer_is_a_choice_or_typed_text_not_both() {
    let mut sel = Selection::new(2);
    assert!(sel.toggle(0, "a", false), "picking tells the card to clear what was typed");
    assert!(sel.toggle(0, "b", false) && !sel.is_picked(0, "a") && sel.is_picked(0, "b"), "a second pick replaces the first");
    assert!(!sel.toggle(0, "b", false) && !sel.is_picked(0, "b"), "a picked option again unpicks it");
    assert!(sel.picked[1].is_empty(), "the other question is its own");
}

#[test]
fn a_multiple_question_combines_picks_and_typed_text() {
    let input = two(true);
    let mut sel = Selection::new(2);
    assert!(!sel.toggle(0, "a", true) && !sel.toggle(0, "c", true), "picks never clear the typed words");
    sel.typed(0, true, "d");
    assert_eq!(sel.picked[0], ["a", "c"], "typed words keep the picks");
    assert_eq!(sel.answers(input.questions.len(), &["d", ""]), vec![vec!["a".to_string(), "c".to_string(), "d".to_string()], vec![]]);
    sel.toggle(0, "a", true);
    assert_eq!(sel.picked[0], ["c"], "a picked option again unpicks it");
}

#[test]
fn picks_a_choice_by_its_number_key() {
    // The number-key half: 1-N pick that option, N+1 goes to "Something else…", the rest do nothing.
    let input = input_of(&call(json!({})));
    assert_eq!(digit_choice(&input, 0, 2), Some(Digit::Pick("Redis".into())));
    assert_eq!(digit_choice(&input, 0, 3), Some(Digit::Custom));
    assert_eq!(digit_choice(&input, 0, 4), None);
    assert_eq!(digit_choice(&input, 0, 0), None);
    assert_eq!(digit_choice(&input, 1, 1), None, "no such question");
    let closed = input_of(&call(json!({"input": {"questions": [{"header": "", "question": "q", "options": [{"label": "x", "description": ""}], "custom": false}]}})));
    assert_eq!(digit_choice(&closed, 0, 2), None, "no typed answer to go to");
}

fn linked(text: &str) -> Vec<&str> {
    link_ranges(text).into_iter().map(|r| &text[r]).collect()
}

#[test]
fn links_a_bare_web_address_and_keeps_the_text_around_it() {
    let text = "Open https://example.com/a?b=1&c=2 now";
    assert_eq!(linked(text), ["https://example.com/a?b=1&c=2"]);
    assert_eq!(link_ranges(text), vec![5..34]);
}

#[test]
fn escapes_markup_in_the_text_and_in_the_address() {
    // As runs, nothing is escaped or dropped: markup stays literal text, and a run never leaves the text.
    let text = "<img src=x onerror=alert(1)> https://example.com/\"><script>";
    let ranges = link_ranges(text);
    assert_eq!(ranges.len(), 1);
    assert!(ranges[0].start >= text.find("https").unwrap() && ranges[0].end <= text.len());
    assert!(!text[..ranges[0].start].contains("https"));
}

#[test]
fn reads_no_markdown_and_links_no_scheme_but_http_s() {
    assert!(linked("a *b* _c_ javascript:alert(1) mailto:x@example.com").is_empty());
}

#[test]
fn leaves_text_without_an_address_as_escaped_text() {
    assert!(link_ranges("1 < 2 & 3").is_empty());
}
