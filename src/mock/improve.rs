//! Scenes and tests for the Improve a skill dialog: the stages, drawn from invented skills and an invented session.
use crate::State;
use crate::fleet::improve::Stage;
use crate::fleet::skills::{BuiltInSkill, SkillProposal, Tokens};
use crate::view::skill_diff::{AFTER, BEFORE};
use rune::TextEdit;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// The form as it opens.
    Fresh,
    /// A note written, the tool calls ticked too.
    Filled,
    /// The whole conversation instead of the parts.
    Whole,
    Asking,
    Proposal,
    Editing,
    /// The server refused: back on the form with its words.
    Refused,
    /// A version kept: the dialog gone, the notice with Undo in its place.
    Kept,
}

const NOTE: &str = "End with the pages you changed, and say so when nothing needed changing.";

pub fn proposal() -> SkillProposal {
    SkillProposal {
        name: "tidy-docs".into(),
        base: BEFORE.into(),
        base_version: None,
        content: AFTER.into(),
        asks: 1,
        tokens: Some(Tokens { total: 2400, from_cache: 1800 }),
    }
}

impl State {
    pub fn show_improve(&mut self, case: Case, dark: bool) {
        self.dark = Some(dark);
        self.show_conversation(crate::mock::tools::of_skills());
        self.skills.list = Some(crate::fleet::skills::mock_skills());
        if case == Case::Kept {
            let saved = BuiltInSkill { name: "tidy-docs".into(), enabled: true, version: Some(1), version_count: 1, ..Default::default() };
            self.improve_announce(&saved, None);
            self.notices.open_next();
            return;
        }
        self.skill_improve_open("tidy-docs", "k1");
        let d = &mut self.skills.dialog;
        if case != Case::Fresh {
            d.note = TextEdit::new(NOTE);
        }
        d.tools = matches!(case, Case::Filled | Case::Whole | Case::Asking);
        d.whole = case == Case::Whole;
        d.stage = if case == Case::Asking { Stage::Asking } else { Stage::Compose };
        if case == Case::Refused {
            d.error = Some("The model didn't change anything. Say more about what should be different.".into());
        }
        if matches!(case, Case::Proposal | Case::Editing) {
            let p = proposal();
            d.draft = TextEdit::new(p.content.clone());
            d.proposal = Some(p);
            d.stage = Stage::Proposal;
            d.editing = case == Case::Editing;
        }
    }
}
