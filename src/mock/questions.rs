//! The question card's scenes (component 4, piece C): a `question` call written as the harness sends it (invented),
//! read by the same reader the live path uses, shown under a short conversation.
use super::tools::message;
use crate::State;
use crate::asks::question::question_input;
use crate::fleet::questions::QuestionCard;
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
}

fn input(case: Case) -> Value {
    let db = |extra: Value| {
        let mut q = json!({"header": "Database", "question": "Which database should the cache use?",
            "options": [{"label": "SQLite", "description": "Local file"}, {"label": "Redis", "description": "Shared server"}]});
        q.as_object_mut().expect("an object").extend(extra.as_object().expect("an object").clone());
        q
    };
    let questions = match case {
        Case::Single | Case::Picked | Case::Typed => vec![db(json!({}))],
        Case::NoCustom => vec![db(json!({"custom": false}))],
        Case::Multiple => vec![json!({"header": "Checks", "question": "Which checks should run before each merge?", "multiple": true,
            "options": [
                {"label": "Unit tests", "description": "Fast, run on every push"},
                {"label": "Type check", "description": ""},
                {"label": "Lint", "description": "Style and common mistakes"},
                {"label": "Browser tests", "description": "Slow: a few minutes"}]})],
        Case::Two => vec![
            db(json!({})),
            json!({"header": "Expiry", "question": "How long should an entry live?",
                "options": [{"label": "5 minutes", "description": ""}, {"label": "1 hour", "description": "The default"}, {"label": "1 day", "description": ""}]}),
        ],
        Case::Long => vec![json!({"header": "Hosting for the review environment and the long-running staging copy",
            "question": "The preview builds are heavy, so I'd like to move them off the shared runner (see https://example.test/docs/runners?plan=team&region=eu for what each plan gives us).\n\nTwo things decide it:\n- cost per build\n- how long a build waits in the queue",
            "options": [
                {"label": "Dedicated runner for preview builds, kept warm between pushes so nothing waits", "description": "Costs more.\nQueue time drops to nothing, and builds start at once even at the end of the day when everyone pushes."},
                {"label": "Shared runner", "description": "No change"}]})],
    };
    json!({"status": "pending", "input": {"questions": questions}})
}

/// The card as the reader makes it, with the case's picks and typed words already in.
pub fn read(case: Case) -> QuestionCard {
    let state = input(case);
    let mut card = QuestionCard::new("call-ask-1", question_input("question", &state).expect("the scene's question can be read"));
    match case {
        Case::Picked => card.sel.picked[0] = vec!["Redis".into()],
        Case::Multiple => card.sel.picked[0] = vec!["Unit tests".into(), "Lint".into()],
        Case::Typed => card.typed[0] = TextEdit::new("Postgres, we already run it"),
        _ => {}
    }
    card
}

impl State {
    /// The open session waits on `case`'s question, under a short conversation.
    pub fn show_question(&mut self, case: Case) {
        self.set_open((0, 0));
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        // The scripted model's keyword, so the picture sits beside Fleet's own.
        let messages = vec![message("u1", "user", 1_000, vec![text("u1-t", "ask-me")])];
        let state =
            reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "waiting_input"}));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::NeedsInput;
        }
        self.questions.cards = vec![read(case)];
        self.now_ms = Some(super::failure::NOW);
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
    }
}
