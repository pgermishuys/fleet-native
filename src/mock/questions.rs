//! The question card's scenes (component 4, piece C): a `question` call written as the harness sends it (invented),
//! read by the same reader the live path uses, shown under a short conversation.
use super::tools::{call, message};
use crate::State;
use crate::fleet::reducer;
use crate::mock::Status;
use rune::TextEdit;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// What the scripted model's `ask-me` raises: one question, two choices.
    Single,
    /// The same, with the second choice picked.
    Picked,
    /// A multiple question with two picks.
    Multiple,
    /// Words typed into "Something else…".
    Typed,
    /// Two questions, the second with a sub-heading.
    Two,
    /// A long text with line breaks and a web address, a long choice name and a long description.
    Long,
    /// A question that takes no typed answer.
    NoCustom,
    /// Answered: the row with the question, the answer and a tick.
    Answered,
    /// Two questions answered, the second with two answers.
    AnsweredTwo,
    /// Answered, with a question and an answer too long for the row.
    AnsweredLong,
    /// Skipped (the call ended in an error).
    Skipped,
    /// The agent no longer holds the question: Fleet said so after Send answer.
    Gone,
    /// Fleet refused the answer for another reason.
    Failed,
    /// A request on its way: Sending… and the choices dimmed.
    Sending,
    /// Finished with no recorded answers, and not an error: nothing draws but the message.
    Bare,
    /// Words made from coloured shell output, terminal codes and all (Fleet #1036): waiting, answered and skipped.
    Coloured,
    ColouredAnswered,
    ColouredSkipped,
}

fn questions(case: Case) -> Vec<Value> {
    let db = |extra: Value| {
        let mut q = json!({"header": "Database", "question": "Which database should the cache use?",
            "options": [{"label": "SQLite", "description": "Local file"}, {"label": "Redis", "description": "Shared server"}]});
        q.as_object_mut().expect("an object").extend(extra.as_object().expect("an object").clone());
        q
    };
    match case {
        Case::Single
        | Case::Picked
        | Case::Typed
        | Case::Answered
        | Case::Skipped
        | Case::Gone
        | Case::Failed
        | Case::Sending
        | Case::Bare => {
            vec![db(json!({}))]
        }
        Case::NoCustom => vec![db(json!({"custom": false}))],
        Case::Coloured | Case::ColouredAnswered | Case::ColouredSkipped => vec![json!({"header": "\u{1b}[1mBuild\u{1b}[0m",
            "question": "\u{1b}[31mTests failed\u{1b}[0m: retry?",
            "options": [{"label": "\u{1b}[32mRetry\u{1b}[0m", "description": "\u{1b}[2mRuns again\u{1b}[0m"}]})],
        Case::Multiple => vec![json!({"header": "Checks", "question": "Which checks should run before each merge?", "multiple": true,
            "options": [
                {"label": "Unit tests", "description": "Fast, run on every push"},
                {"label": "Type check", "description": ""},
                {"label": "Lint", "description": "Style and common mistakes"},
                {"label": "Browser tests", "description": "Slow: a few minutes"}]})],
        Case::AnsweredLong => vec![db(
            json!({"question": "Which of the two storage engines we compared last week, the embedded file store or the shared network cache, should the image cache use from now on?"}),
        )],
        Case::Two | Case::AnsweredTwo => vec![
            db(json!({})),
            json!({"header": "Expiry", "question": "How long should an entry live?",
                "options": [{"label": "5 minutes", "description": ""}, {"label": "1 hour", "description": "The default"}, {"label": "1 day", "description": ""}]}),
        ],
        Case::Long => vec![json!({"header": "Hosting for the review environment and the long-running staging copy",
            "question": "The preview builds are heavy, so I'd like to move them off the shared runner (see https://example.test/docs/runners?plan=team&region=eu for what each plan gives us).\n\nTwo things decide it:\n- cost per build\n- how long a build waits in the queue",
            "options": [
                {"label": "Dedicated runner for preview builds, kept warm between pushes so nothing waits", "description": "Costs more.\nQueue time drops to nothing, and builds start at once even at the end of the day when everyone pushes."},
                {"label": "Shared runner", "description": "No change"}]})],
    }
}

/// The call's state as the harness sends it.
fn state(case: Case) -> (&'static str, Value) {
    let (status, extra) = match case {
        Case::Answered => ("completed", json!({"metadata": {"answers": [["Redis"]]}})),
        Case::AnsweredTwo => ("completed", json!({"metadata": {"answers": [["Redis"], ["1 hour", "1 day"]]}})),
        Case::AnsweredLong => (
            "completed",
            json!({"metadata": {"answers": [["Dedicated runner for preview builds, kept warm so nothing waits, billed by the minute"]]}}),
        ),
        Case::ColouredAnswered => ("completed", json!({"metadata": {"answers": [["Retry"]]}})),
        Case::Skipped | Case::ColouredSkipped => ("error", json!({})),
        Case::Bare => ("completed", json!({})),
        _ => ("pending", json!({})),
    };
    (status, extra)
}

impl State {
    /// The open session waits on `case`'s question, under a short conversation: the call is a part of the agent's
    /// message, as the server sends it, and the card is made from it by the same path a live session uses.
    pub fn show_question(&mut self, case: Case) {
        self.set_open((0, 0));
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        let (status, extra) = state(case);
        let waiting = status == "pending";
        // The scripted model's keyword, so the picture sits beside Fleet's own.
        let messages = vec![
            message("u1", "user", 1_000, vec![text("u1-t", "ask-me")]),
            message("a1", "assistant", 2_000, vec![call("ask-1", "question", status, json!({"questions": questions(case)}), extra)]),
        ];
        let activity = if waiting { "waiting_input" } else { "idle" };
        let state = reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": activity}));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = if waiting { Status::NeedsInput } else { Status::Idle };
        }
        self.now_ms = Some(super::failure::NOW);
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
        let Some(card) = self.questions.cards.iter_mut().find(|c| c.id == "call-ask-1") else { return };
        match case {
            Case::Picked | Case::Sending => card.sel.picked[0] = vec!["Redis".into()],
            Case::Multiple => card.sel.picked[0] = vec!["Unit tests".into(), "Lint".into()],
            Case::Typed => card.typed[0] = TextEdit::new("Postgres, we already run it"),
            Case::Gone => {
                card.sel.picked[0] = vec!["Redis".into()];
                (card.error, card.gone) = (Some(crate::fleet::questions::NOT_WAITING.to_string()), true);
            }
            Case::Failed => {
                card.sel.picked[0] = vec!["Redis".into()];
                card.error = Some(crate::fleet::questions::failure(crate::fleet::questions::Sent::Answer, 500).0);
            }
            _ => {}
        }
        if case == Case::Sending {
            card.busy = true;
        }
    }
}
