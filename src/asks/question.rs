//! What a `question` tool call holds and how the card that waits on it answers (`lib/question-types.ts`, the selection
//! rules and `submit` of `components/session/QuestionCard.vue`, and `lib/linkify-text.ts`; rows 1.4-1.17, 2.3 and 2.6
//! of `docs/parity/04-questions-and-permissions.md`). Plain Rust, no Rune: the view in `view::question` draws it and
//! piece D sends it.
use crate::conv::tools::{Category, get_tool};
use serde_json::Value;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// `QuestionInfo`: one question of the call.
#[derive(Clone, Debug, PartialEq)]
pub struct QuestionInfo {
    /// A very short heading (30 characters at most); may be empty.
    pub header: String,
    pub question: String,
    pub options: Vec<QuestionOption>,
    pub multiple: bool,
    /// May the user type an answer of their own? Fleet's default is yes.
    pub custom: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuestionInput {
    pub questions: Vec<QuestionInfo>,
}

/// `getQuestionStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionStatus {
    Pending,
    Running,
    Completed,
    Error,
    Unknown,
}

impl QuestionStatus {
    /// A card waits on the user while the call is pending or running (`isActive`).
    pub fn is_active(self) -> bool {
        matches!(self, QuestionStatus::Pending | QuestionStatus::Running)
    }
}

fn text(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or_default().to_string()
}

fn info(q: &Value) -> QuestionInfo {
    let options = q
        .get("options")
        .and_then(Value::as_array)
        .map(|o| o.iter().map(|o| QuestionOption { label: text(o.get("label")), description: text(o.get("description")) }).collect())
        .unwrap_or_default();
    QuestionInfo {
        header: text(q.get("header")),
        question: text(q.get("question")),
        options,
        multiple: q.get("multiple") == Some(&Value::Bool(true)),
        custom: q.get("custom") != Some(&Value::Bool(false)),
    }
}

/// `getQuestionInput`: the questions of a call of the question tool; `None` for any other tool, for a state that isn't
/// an object, and for one whose input has no `questions` list.
pub fn question_input(tool: &str, state: &Value) -> Option<QuestionInput> {
    if get_tool(tool).category != Category::Question {
        return None;
    }
    let questions = state.as_object()?.get("input")?.get("questions")?.as_array()?;
    Some(QuestionInput { questions: questions.iter().map(info).collect() })
}

/// `getQuestionAnswers`: what a completed call recorded, one list of labels per question.
pub fn question_answers(tool: &str, state: &Value) -> Option<Vec<Vec<String>>> {
    if get_tool(tool).category != Category::Question {
        return None;
    }
    let state = state.as_object()?;
    if state.get("status")?.as_str() != Some("completed") {
        return None;
    }
    let answers = state.get("metadata")?.get("answers")?.as_array()?;
    Some(
        answers
            .iter()
            .map(|a| a.as_array().map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_string)).collect()).unwrap_or_default())
            .collect(),
    )
}

pub fn question_status(state: &Value) -> QuestionStatus {
    match state.as_object().and_then(|s| s.get("status")).and_then(Value::as_str) {
        Some("pending") => QuestionStatus::Pending,
        Some("running") => QuestionStatus::Running,
        Some("completed") => QuestionStatus::Completed,
        Some("error") => QuestionStatus::Error,
        _ => QuestionStatus::Unknown,
    }
}

/// `answerText`: the answers joined by ", ", or "No answer" when there are none.
pub fn answer_text(answers: Option<&Vec<String>>) -> String {
    match answers {
        Some(a) if !a.is_empty() => a.join(", "),
        _ => "No answer".to_string(),
    }
}

/// `keyHint`: what the footer says about the keys, from the first question's choice count (options and the typed one).
pub fn key_hint(input: &QuestionInput) -> String {
    let count = input.questions.first().map_or(0, |q| q.options.len() + usize::from(q.custom));
    if count < 2 { "Enter to send".to_string() } else { format!("1\u{2013}{} to pick \u{b7} Enter to send", count.min(9)) }
}

/// What a digit does on the card (`handleCardKeydown`): pick that option, or put the caret in "Something else…".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Digit {
    Pick(String),
    Custom,
}

/// The digit 1-9 for question `qi`; `None` past the last choice, or for a question that takes no typed answer.
pub fn digit_choice(input: &QuestionInput, qi: usize, digit: usize) -> Option<Digit> {
    let q = input.questions.get(qi)?;
    match q.options.get(digit.checked_sub(1)?) {
        Some(o) => Some(Digit::Pick(o.label.clone())),
        None if digit == q.options.len() + 1 && q.custom => Some(Digit::Custom),
        None => None,
    }
}

/// What has been picked, for each question: the labels, in the order they were picked. What was typed lives in the
/// card's text fields; the rules that tie the two together take it as an argument.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub picked: Vec<Vec<String>>,
}

impl Selection {
    pub fn new(questions: usize) -> Selection {
        Selection { picked: vec![Vec::new(); questions] }
    }

    pub fn is_picked(&self, qi: usize, label: &str) -> bool {
        self.picked.get(qi).is_some_and(|p| p.iter().any(|l| l == label))
    }

    /// `toggleOption`. Picking a picked option again unpicks it. A single answer is a choice or typed text, not both, so
    /// picking in one clears the typed text: true when the caller must clear it.
    pub fn toggle(&mut self, qi: usize, label: &str, multiple: bool) -> bool {
        let Some(current) = self.picked.get_mut(qi) else { return false };
        if multiple {
            match current.iter().position(|l| l == label) {
                Some(at) => {
                    current.remove(at);
                }
                None => current.push(label.to_string()),
            }
            false
        } else {
            let was = current.iter().any(|l| l == label);
            *current = if was { Vec::new() } else { vec![label.to_string()] };
            !was
        }
    }

    /// `handleCustomInput`: typing (not just blanks) into a single-answer question drops its pick.
    pub fn typed(&mut self, qi: usize, multiple: bool, typed: &str) {
        if !multiple
            && !typed.trim().is_empty()
            && let Some(current) = self.picked.get_mut(qi)
        {
            current.clear();
        }
    }

    /// `hasAnyAnswer`: Send and Enter need a pick or some typed words that aren't only blanks.
    pub fn has_any_answer(&self, typed: &[&str]) -> bool {
        self.picked.iter().any(|p| !p.is_empty()) || typed.iter().any(|t| !t.trim().is_empty())
    }

    /// What `submit` sends: for each question the picked labels, then the trimmed typed text when there is some.
    pub fn answers(&self, questions: usize, typed: &[&str]) -> Vec<Vec<String>> {
        (0..questions)
            .map(|i| {
                let mut out = self.picked.get(i).cloned().unwrap_or_default();
                let custom = typed.get(i).map_or("", |t| t.trim());
                if !custom.is_empty() {
                    out.push(custom.to_string());
                }
                out
            })
            .collect()
    }
}

/// `linkifyText`: the web addresses of `text` as byte ranges. Only `http(s)` links, and no Markdown is read, so the
/// text between the ranges is plain. (The rule is the failure card's, `conv::turn_error::url_ranges`.)
pub fn link_ranges(text: &str) -> Vec<Range<usize>> {
    crate::conv::turn_error::url_ranges(text)
}
