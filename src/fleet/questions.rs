//! The question cards on screen and what the user has picked in each (`QuestionCard.vue`'s selection state, rows
//! 1.12, 1.16 and 1.17 of `docs/parity/04-questions-and-permissions.md`). The rules are `asks::question`; this holds
//! them per card, with the text fields Rune draws. Piece D fills the cards from the open session's `question` tool
//! parts and sends the answer; until then a scene puts a card here.
use crate::State;
use crate::asks::question::{QuestionInput, Selection};
use rune::TextEdit;

/// One waiting question: its call, the questions, and what is picked and typed.
pub struct QuestionCard {
    /// The tool part's `callId`, which is also the request id the answer routes take.
    pub id: String,
    pub input: QuestionInput,
    pub sel: Selection,
    /// The typed "Something else…" of each question.
    pub typed: Vec<TextEdit>,
}

impl QuestionCard {
    pub fn new(id: impl Into<String>, input: QuestionInput) -> QuestionCard {
        let n = input.questions.len();
        QuestionCard { id: id.into(), input, sel: Selection::new(n), typed: (0..n).map(|_| TextEdit::new("")).collect() }
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
    pub fn keep_questions(&mut self) -> bool {
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
