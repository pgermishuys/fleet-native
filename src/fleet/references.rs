//! The draft's references on `State` (component 5, piece L): which `$` skills the open session knows, what the draft
//! tints, the keys that edit a picked reference as one piece, and what goes to the server with a message.
use crate::State;
use crate::composer::draftrefs::{
    Known, RefKey, RefSpan, caret_outside_references, picked_reference_ranges, reference_key_action, reference_spans,
};
use crate::composer::refs::{SkillDelivery, skill_deliveries, skill_names_in, skills_among};
use crate::composer::status::{Mode, mode_of};
use std::ops::Range;

impl State {
    /// The names of the skills the open session lists among its commands.
    fn skill_names(&self) -> Vec<String> {
        let commands = match self.draft_owner().and_then(|o| self.finder.commands.get(&o)) {
            Some(loaded) => loaded.items.clone(),
            None if self.is_mock() => crate::mock::finder::commands(),
            None => Vec::new(),
        };
        skills_among(&commands).into_iter().map(|s| s.name).collect()
    }

    /// The skills a text names and how the open session's harness delivers each (`skillDeliveries`).
    fn skill_mentions(&self, text: &str) -> Vec<(String, SkillDelivery)> {
        let names = self.skill_names();
        let known: Vec<&str> = names.iter().map(String::as_str).collect();
        let found = skill_names_in(text, &known);
        let harness = self.session().row.harness_type.clone();
        skill_deliveries(harness.as_deref(), &found).into_iter().map(|m| (m.name, m.delivery)).collect()
    }

    /// Whether the draft holds references at all: not in shell mode, where it is a command as typed.
    fn draft_has_references(&self) -> bool {
        mode_of(&self.draft.text, &self.caps(), self.side_open) != Mode::Shell
    }

    /// The references in the draft, to tint: the picks of this draft, the skills it names, the caret where it is.
    pub fn draft_references(&self) -> Vec<RefSpan> {
        let Some(owner) = self.draft_owner().filter(|_| self.draft_has_references()) else { return Vec::new() };
        let picks = self.auto.picks_of(&owner);
        let sessions: Vec<&str> = picks.map(|p| p.remembered_session_tokens()).unwrap_or_default();
        let files: Vec<&str> = picks.map(|p| p.files().iter().map(String::as_str).collect()).unwrap_or_default();
        let mentions = self.skill_mentions(&self.draft.text);
        let skills: Vec<(&str, SkillDelivery)> = mentions.iter().map(|(n, d)| (n.as_str(), *d)).collect();
        let (_, caret) = self.draft.range();
        reference_spans(&self.draft.text, Some(caret), &Known { sessions: &sessions, files: &files, skills: &skills, plain: &[] })
    }

    /// Where the picked references are, which edit as one piece.
    fn picked_ranges(&self) -> Vec<Range<usize>> {
        let Some(owner) = self.draft_owner().filter(|_| self.draft_has_references()) else { return Vec::new() };
        let Some(picks) = self.auto.picks_of(&owner) else { return Vec::new() };
        let sessions = picks.remembered_session_tokens();
        let files: Vec<&str> = picks.files().iter().map(String::as_str).collect();
        picked_reference_ranges(&self.draft.text, &sessions, &files)
    }

    /// Backspace and Delete next to a picked reference select all of it and leave the delete to the field, so it goes in
    /// one piece and undo brings it back; the left and right arrows step over it (`handleReferenceKey`). True when
    /// the key was dealt with.
    pub fn reference_key(&mut self, ev: &rune::KeyEvent) -> bool {
        use rune::Key;
        let key = match ev.key {
            Key::Backspace => RefKey::Backspace,
            Key::Delete => RefKey::Delete,
            Key::Left => RefKey::Left,
            Key::Right => RefKey::Right,
            _ => return false,
        };
        let is_delete = matches!(key, RefKey::Backspace | RefKey::Delete);
        // Shift+arrows select and Cmd+Backspace deletes the line: those stay the field's.
        if ev.mods.cmd || (!is_delete && (ev.mods.shift || ev.mods.alt || ev.mods.ctrl)) {
            return false;
        }
        let (anchor, focus) = self.draft.range();
        if anchor != focus {
            return false;
        }
        let Some(action) = reference_key_action(key, focus, &self.picked_ranges()) else { return false };
        self.draft.set_selection(action.selection.0, action.selection.1);
        action.prevent_default
    }

    /// A caret left inside a picked reference (by a click, say) moves to its nearer edge (`keepCaretOutside`).
    pub fn keep_caret_outside_references(&mut self) {
        let (anchor, focus) = self.draft.range();
        if anchor != focus {
            return;
        }
        let moved = caret_outside_references(focus, &self.picked_ranges());
        if moved != focus {
            self.draft.set_caret(moved);
        }
    }

    /// What the draft's picks and the open session's skills add to a message: the sessions whose tokens still stand
    /// alone in `text`, in order, and the skills it names (`sessionReferencesIn`, `skillNamesIn`).
    pub fn add_references(&self, pick: &mut super::http::Pick, text: &str) {
        let Some(owner) = self.draft_owner() else { return };
        if let Some(picks) = self.auto.picks_of(&owner) {
            pick.session_references = picks.session_references_in(text).into_iter().map(|r| (r.token, r.session_id)).collect();
        }
        pick.skills = self.skill_mentions(text).into_iter().map(|(name, _)| name).collect();
    }

    /// Files and folders picked and then deleted from the draft are forgotten; the draft's picks are kept with it.
    pub fn keep_picks(&mut self, owner: &str, now: f64) {
        let text = self.draft.text.clone();
        let picks = self.auto.picks_mut(owner);
        picks.forget_files_not_in(&text);
        let picks = picks.clone();
        self.drafts.set_picks(owner, &picks, now);
    }
}
