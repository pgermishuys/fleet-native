//! `maxlength` on a field: Fleet's browser stops typing at the limit, Rune's text field has no such setting, so each field that
//! has one is cut back to it on every poll (what the memory page does for its two fields, `limit_memory_fields`).
use crate::State;
use rune::TextEdit;

/// The report's description (`ProblemReportDialog.vue`).
pub const REPORT_WHAT: usize = 10_000;
/// "What did you expect?".
pub const REPORT_EXPECTED: usize = 2_000;
/// "Want a reply?" and the review's title.
pub const REPORT_SHORT: usize = 200;

/// Cuts `edit` to `max` characters, the caret goes to the end of what is left; true when it cut.
pub fn limit(edit: &mut TextEdit, max: usize) -> bool {
    let Some((at, _)) = edit.text.char_indices().nth(max) else { return false };
    edit.replace_text(edit.text[..at].to_string());
    true
}

impl State {
    /// Called from `poll`: every limited field that is open. True when one was cut, so the frame is drawn again.
    pub fn keep_text_limits(&mut self) -> bool {
        let mut cut = false;
        if self.report.open {
            let f = &mut self.report_form;
            cut |= limit(&mut f.description, REPORT_WHAT);
            cut |= limit(&mut f.expected, REPORT_EXPECTED);
            cut |= limit(&mut f.contact, REPORT_SHORT);
            cut |= limit(&mut f.title, REPORT_SHORT);
        }
        if self.skills.dialog.open {
            cut |= limit(&mut self.skills.dialog.note, super::improve::NOTE_MAX);
        }
        if self.mod_drafts.review.is_some() {
            cut |= limit(&mut self.mod_drafts.dialog.note, super::mods::drafts::NOTE_LIMIT);
        }
        cut
    }
}
