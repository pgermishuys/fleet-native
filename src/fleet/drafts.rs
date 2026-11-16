//! Writing into a session's draft from outside its composer (`use-draft-state.ts`: `appendDraftReference`, `appendDraftText`,
//! `clearDraftText`). The open session's text lives in the text area, the others' in the drafts file's map.
use crate::State;
use crate::composer::drafts::{session_draft_key, with_reference, with_text};

impl State {
    fn rewrite_draft(&mut self, session: &str, change: impl Fn(&str) -> String) {
        if self.draft_of.as_deref() == Some(session) {
            let text = change(&self.draft.text);
            self.set_draft_text(&text);
        } else {
            let text = change(self.drafts.text_of(session));
            let now = self.clock;
            self.drafts.write(&session_draft_key(session), &text, now);
        }
    }

    /// "Reference this line" in the changes view: nothing is sent.
    pub fn append_draft_reference(&mut self, session: &str, reference: &str) {
        self.rewrite_draft(session, |t| with_reference(t, reference));
    }

    pub fn append_draft_text(&mut self, session: &str, add: &str) {
        self.rewrite_draft(session, |t| with_text(t, add));
    }

    /// `clearDraftText`: the text, in the box too when the session is the open one.
    pub fn clear_draft_text(&mut self, session: &str) {
        self.rewrite_draft(session, |_| String::new());
    }

    /// A deleted session's drafts, side drafts and history are forgotten; the box is emptied if it held them.
    pub(crate) fn forget_drafts_of(&mut self, session: &str) {
        if self.draft_of.as_deref() == Some(session) {
            self.draft_of = None;
            self.draft.clear();
        }
        let now = self.clock;
        self.drafts.clear_of(session, now);
    }
}
