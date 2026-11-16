//! The question cards on screen and what the user has picked in each (`QuestionCard.vue`'s selection state, rows
//! 1.12, 1.16 and 1.17 of `docs/parity/04-questions-and-permissions.md`). The rules are `asks::question`; this holds
//! them per card, with the text fields Rune draws. Piece D fills the cards from the open session's `question` tool
//! parts (`sync_questions`, rows 7.9 and 7.10: the server drops `question.asked`, so a waiting question is only ever
//! seen as a tool part), sends the answer or the skip, and says why one failed (rows 1.21-1.23, 7.1-7.3).
use super::http;
use super::model::PartData;
use crate::State;
use crate::asks::question::{QuestionInput, QuestionPart, Selection};
use crate::conv::tools::{Category, get_tool};
use rune::{TextEdit, Tx};
use serde_json::Value;

/// `NOT_WAITING`: what the card says when Fleet no longer holds the question.
pub const NOT_WAITING: &str =
    "The agent isn't waiting for this question any more. It was answered somewhere else, or Fleet or the agent restarted.";

/// Which request a card sent: the failure text names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    Answer,
    Skip,
}

/// `Response.statusText` for the codes a Fleet route answers with (HTTP/1.1 reason phrases; HTTP/2 sends none).
pub fn reason(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
    }
}

/// `failure` and the catch of `submit` and `dismiss`: the error line and whether the question is gone. 404 and 409
/// mean the agent no longer holds it; anything else is Fleet's status text.
pub fn failure(sent: Sent, status: u16) -> (String, bool) {
    if status == 404 || status == 409 {
        return (NOT_WAITING.to_string(), true);
    }
    let verb = if sent == Sent::Answer { "answer" } else { "reject" };
    (format!("Failed to {verb} question: {}", reason(status)), false)
}

/// One waiting question: its call, the questions, and what is picked and typed.
pub struct QuestionCard {
    /// The tool part's `callId`, which is also the request id the answer routes take.
    pub id: String,
    /// The session the question was asked in: the routes name it.
    pub session: String,
    /// A request on its way (`loading`): the card ignores keys and its buttons dim.
    pub busy: bool,
    pub error: Option<String>,
    /// Fleet said the agent no longer holds the question (`gone`): the card is a leftover, and Dismiss hides it here.
    pub gone: bool,
    /// Dismissed here (`hiddenHere`): the card is drawn as a skipped row.
    pub hidden: bool,
    pub input: QuestionInput,
    pub sel: Selection,
    /// The typed "Something else…" of each question.
    pub typed: Vec<TextEdit>,
}

impl QuestionCard {
    pub fn new(id: impl Into<String>, input: QuestionInput) -> QuestionCard {
        let n = input.questions.len();
        QuestionCard {
            id: id.into(),
            session: String::new(),
            busy: false,
            error: None,
            gone: false,
            hidden: false,
            input,
            sel: Selection::new(n),
            typed: (0..n).map(|_| TextEdit::new("")).collect(),
        }
    }

    pub fn typed_texts(&self) -> Vec<&str> {
        self.typed.iter().map(|t| t.text.as_str()).collect()
    }

    /// `hasAnyAnswer`.
    pub fn has_any_answer(&self) -> bool {
        self.sel.has_any_answer(&self.typed_texts())
    }

    /// What `submit` would send.
    pub fn answers(&self) -> Vec<Vec<String>> {
        self.sel.answers(self.input.questions.len(), &self.typed_texts())
    }
}

#[derive(Default)]
pub struct Questions {
    pub cards: Vec<QuestionCard>,
    /// Cards that appeared and have not yet taken the keyboard (`onMounted`).
    fresh: Vec<String>,
    /// What an edit lands in when its card has gone while the edit was on its way.
    gone: TextEdit,
}

impl Questions {
    pub fn card(&self, id: &str) -> Option<&QuestionCard> {
        self.cards.iter().find(|c| c.id == id)
    }

    pub fn typed_mut(&mut self, id: &str, qi: usize) -> &mut TextEdit {
        match self.cards.iter().position(|c| c.id == id).filter(|&at| qi < self.cards[at].typed.len()) {
            Some(at) => &mut self.cards[at].typed[qi],
            None => &mut self.gone,
        }
    }
}

impl State {
    /// A click on a choice (`toggleOption`): picking in a single-answer question drops what was typed there.
    pub fn pick_choice(&mut self, id: &str, qi: usize, label: &str) {
        let Some(card) = self.questions.cards.iter_mut().find(|c| c.id == id) else { return };
        let Some(multiple) = card.input.questions.get(qi).map(|q| q.multiple) else { return };
        if card.sel.toggle(qi, label, multiple) {
            card.typed[qi] = TextEdit::new("");
        }
    }

    /// Typing into "Something else…" (`handleCustomInput`): a single answer is a choice or words, so typed words
    /// drop the pick. Rune has no change hook on a field, so this runs once a frame; true when it changed a pick.
    pub fn keep_questions(&mut self, tx: &mut Tx) -> bool {
        self.focus_new_questions(tx);
        let mut changed = false;
        for card in &mut self.questions.cards {
            for (qi, q) in card.input.questions.iter().enumerate() {
                if !q.multiple && !card.sel.picked[qi].is_empty() && !card.typed[qi].text.trim().is_empty() {
                    card.sel.typed(qi, q.multiple, &card.typed[qi].text);
                    changed = true;
                }
            }
        }
        changed
    }
}

impl State {
    /// Makes a card for every question the open session's conversation is waiting on (rows 7.9, 7.10). A card that
    /// has one already keeps its picks; one for a question that has been answered stays until the session goes, so a
    /// card dismissed here stays a skipped row.
    pub fn sync_questions(&mut self) {
        let Some(stream) = &self.stream else { return };
        // An invented session has no id a server knows; its cards are made all the same, for scenes.
        let session = self.open_id().unwrap_or_default();
        let mut made = Vec::new();
        for part in stream.messages.iter().flat_map(|m| &m.parts) {
            let PartData::Tool { id, tool, call_id, state } = part else { continue };
            if get_tool(tool).category != Category::Question {
                continue;
            }
            let Some(QuestionPart { status, input: Some(input), .. }) = QuestionPart::read(tool, state) else { continue };
            // History written by older servers has no call id; the part's id stands in for it.
            let call = if call_id.is_empty() { id } else { call_id };
            if status.is_active() && self.questions.card(call).is_none() {
                let mut card = QuestionCard::new(call.clone(), input);
                card.session = session.clone();
                made.push(card);
            }
        }
        for card in made {
            self.questions.fresh.push(card.id.clone());
            self.questions.cards.push(card);
        }
    }

    /// `onMounted`: a card that just appeared takes the keyboard on its first choice (else itself), unless the user is
    /// typing elsewhere (`focusBlockingCard`). True when it asked for focus.
    pub fn focus_new_questions(&mut self, tx: &mut Tx) -> bool {
        let Some(id) = self.questions.fresh.first().cloned() else { return false };
        // Wait for the card to be drawn: a question seen before its message is on screen keeps its turn.
        if !self.questions.card(&id).is_some_and(|c| self.drawn_question(c)) {
            return false;
        }
        self.questions.fresh.remove(0);
        if crate::asks::focus::takes_focus(tx.focused(), "composer", self.draft.text.len())
            && let Some(card) = self.questions.card(&id)
        {
            let first = card.input.questions.first().and_then(|q| q.options.first()).map(|o| format!("question-pill-{}", o.label));
            tx.focus(first.unwrap_or_else(|| "question-card".to_string()), false);
        }
        true
    }

    /// Is this card on the open session's screen as a waiting card (not a row)?
    pub fn drawn_question(&self, card: &QuestionCard) -> bool {
        !card.hidden && self.open_id().unwrap_or_default() == card.session && self.question_is_active(&card.id)
    }

    /// Does the open session's conversation still say the question waits (pending or running)?
    pub fn question_is_active(&self, call: &str) -> bool {
        let Some(stream) = &self.stream else { return false };
        stream.messages.iter().flat_map(|m| &m.parts).any(|p| match p {
            PartData::Tool { id, tool, call_id, state } => {
                (if call_id.is_empty() { id } else { call_id }) == call
                    && QuestionPart::read(tool, state).is_some_and(|q| q.status.is_active())
            }
            _ => false,
        })
    }

    /// The card the keys act on: the oldest one waiting on the open session.
    pub fn active_question(&self) -> Option<&QuestionCard> {
        self.questions.cards.iter().find(|c| self.drawn_question(c))
    }

    /// `submit`: sends what is picked and typed. Ignored while a request is on its way, and with nothing answered.
    pub fn submit_question(&mut self, id: &str) {
        let Some(card) = self.questions.cards.iter_mut().find(|c| c.id == id) else { return };
        if card.busy || !card.has_any_answer() {
            return;
        }
        let answers = card.answers();
        let (session, request) = (card.session.clone(), card.id.clone());
        self.send_question(id, Sent::Answer, move |base| http::question_answer(base, &session, &request, &answers));
    }

    /// `dismiss`: a leftover card (Fleet said it's gone) is only hidden here and sends nothing; otherwise Skip rejects.
    pub fn skip_question(&mut self, id: &str) {
        let Some(card) = self.questions.cards.iter_mut().find(|c| c.id == id) else { return };
        if card.busy {
            return;
        }
        if card.gone {
            card.hidden = true;
            return;
        }
        let (session, request) = (card.session.clone(), card.id.clone());
        self.send_question(id, Sent::Skip, move |base| http::question_reject(base, &session, &request));
    }

    fn send_question(&mut self, id: &str, sent: Sent, call: impl FnOnce(&str) -> Result<(u16, Value), String> + Send + 'static) {
        let Some(link) = &self.link else { return };
        if let Some(card) = self.questions.cards.iter_mut().find(|c| c.id == id) {
            card.busy = true;
            card.error = None;
        }
        let id = id.to_string();
        link.off_thread(call, move |s, _, result| s.on_question_sent(&id, sent, result));
    }

    /// The answer to a request: 2xx leaves the card for the tool part's new status to turn into a row; 404 and 409
    /// mean the agent isn't waiting; anything else is the error line with the card kept and its buttons back.
    pub fn on_question_sent(&mut self, id: &str, sent: Sent, result: Result<(u16, Value), String>) {
        let Some(card) = self.questions.cards.iter_mut().find(|c| c.id == id) else { return };
        card.busy = false;
        match result {
            Ok((status, _)) if (200..300).contains(&status) => {}
            Ok((status, _)) => {
                let (error, gone) = failure(sent, status);
                (card.error, card.gone) = (Some(error), gone);
            }
            Err(why) => {
                card.gone = false;
                card.error = Some(if why.trim().is_empty() { "Failed to submit answer".into() } else { why });
            }
        }
    }
}
