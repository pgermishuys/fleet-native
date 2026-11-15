//! The composer's other modes (docs/parity/05-composer.md, piece M): the message history list on ArrowUp (`use-input-history.ts`,
//! `Composer.vue` `handleKeydown`), shell mode (`!ls`, `use-run-shell-command.ts`) and the `/btw` side question, whose panel is
//! component 15's.
use super::http::{self, Pick};
use super::queue::error_message;
use crate::State;
use crate::composer::status::{Mode, mode_of};
use rune::{Key, KeyEvent, Tx};
use serde_json::Value;

/// What takes a side question. Component 15 builds the panel; until then it is missing, and the composer says so and keeps
/// the draft. The other two stand in for the panel in tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SidePanel {
    #[default]
    Missing,
    Accepts,
    Refuses,
}

/// A question given to the side conversation: its text and the choice picked in the composer (none picked means the session's own).
#[derive(Debug, Clone, PartialEq)]
pub struct SideAsk {
    pub session: String,
    pub question: String,
    pub pick: Pick,
}

pub const NO_QUESTION: &str = "Type a question after /btw.";
pub const SIDE_MISSING: &str = "The side conversation isn't in the native app yet (component 15 of the port).";

impl State {
    /// The list is open for the open session: its entries newest first and the highlighted one.
    pub fn history_view(&self) -> Option<(&[String], usize)> {
        let owner = self.draft_owner()?;
        let h = self.drafts.history(&owner)?;
        (h.open && !self.composer_disabled()).then(|| (h.entries.as_slice(), h.selected.unwrap_or(0)))
    }

    fn history_of(&mut self) -> Option<&mut crate::composer::history::History> {
        let owner = self.draft_owner()?;
        Some(self.drafts.history_mut(&owner))
    }

    /// Puts a text in the text area as one edit, the caret at its end.
    pub fn set_draft_text(&mut self, text: &str) {
        let end = self.draft.text.len();
        self.draft.set_selection(0, end);
        self.draft.insert(text);
    }

    /// A key in the text area, before the popup and the editor (`handleKeydown`). While the list is open the arrows, Enter and
    /// Esc are its own; any other key closes it and goes on. ArrowUp in an empty draft opens it. Esc in shell mode drops the `!`.
    pub fn history_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if self.composer_disabled() {
            return false;
        }
        let open = self.history_view().is_some();
        if open {
            match ev.key {
                Key::Up => self.history_move(true, tx),
                Key::Down => self.history_move(false, tx),
                Key::Enter => {
                    let picked = self.history_of().and_then(|h| h.confirm());
                    if let Some(text) = picked {
                        self.set_draft_text(&text);
                    }
                }
                Key::Escape => {
                    if let Some(h) = self.history_of() {
                        h.close();
                    }
                }
                _ => {
                    if let Some(h) = self.history_of() {
                        h.close();
                    }
                    return self.history_after_close(ev);
                }
            }
            return true;
        }
        self.history_after_close(ev)
    }

    /// The keys that act with the list closed.
    fn history_after_close(&mut self, ev: &KeyEvent) -> bool {
        if self.draft.is_composing() {
            return false;
        }
        if ev.key == Key::Up && self.draft.text.trim().is_empty() {
            if let Some(h) = self.history_of() {
                h.open();
            }
            return true;
        }
        if ev.key == Key::Escape && self.mode() == Mode::Shell {
            let rest = self.draft.text[1..].to_string();
            self.set_draft_text(&rest);
            return true;
        }
        false
    }

    /// A `!` draft before the harness list has arrived: it isn't known whether the harness runs commands, so it is held
    /// rather than sent as a prompt (`shellSupportPending`). Without a server there is nothing to wait for.
    pub fn shell_support_pending(&self) -> bool {
        self.link.is_some()
            && !self.pickers.harnesses_loaded
            && crate::composer::route::is_shell_draft(&self.draft.text)
            && !self.caps().supports_shell
    }

    pub fn mode(&self) -> Mode {
        mode_of(&self.draft.text, &self.caps(), self.side_open)
    }

    fn history_move(&mut self, older: bool, tx: &mut Tx) {
        let Some(h) = self.history_of() else { return };
        if older {
            h.older();
        } else {
            h.newer();
        }
        if let Some(i) = h.selected {
            tx.scroll_into_view_in("history-scroll", format!("history-row{i}"));
        }
    }

    /// A click on an entry: it goes in the text area, the text area keeps its focus (`mousedown.prevent`).
    pub fn history_pick(&mut self, index: usize, tx: &mut Tx) {
        let Some(h) = self.history_of() else { return };
        let text = h.entries.get(index).cloned();
        h.close();
        if let Some(text) = text {
            self.set_draft_text(&text);
        }
        tx.focus("composer", false);
    }

    /// The pointer over an entry highlights it.
    pub fn history_hover(&mut self, index: usize) {
        if let Some(h) = self.history_of()
            && h.open
            && index < h.entries.len()
        {
            h.selected = Some(index);
        }
    }

    /// Runs the shell-mode draft's command (`runShellDraft`): the draft clears at once and comes back if the server refuses
    /// it, unless something new was typed. The command and its output reach the conversation as events.
    pub fn run_shell(&mut self, tx: &mut Tx, command: String) {
        let Some(owner) = self.draft_owner() else { return };
        let typed = self.draft.text.clone();
        let now = self.clock;
        self.drafts.push_history(&owner, typed.trim(), now);
        self.composer_error = None;
        self.draft.clear();
        tx.focus("composer", false);
        let (Some(link), Some(session)) = (&self.link, self.open_id()) else { return };
        let asked = session.clone();
        link.off_thread(move |base| http::shell(base, &asked, &command), move |s, _, result| s.on_shell_answer(&session, typed, result));
    }

    pub fn on_shell_answer(&mut self, session: &str, typed: String, result: Result<(u16, Value), String>) {
        let why = match result {
            Ok((status, _)) if (200..300).contains(&status) => return,
            Ok((status, body)) => error_message(&body, format!("The command couldn't be run (HTTP {status}).")),
            Err(why) if why.trim().is_empty() => "The command couldn't be run.".to_string(),
            Err(why) => why,
        };
        if self.draft_owner().as_deref() != Some(session) {
            return;
        }
        self.composer_error = Some(why);
        if self.draft.text.is_empty() {
            self.draft = rune::TextEdit::new(typed);
        }
    }

    /// A `/btw` question, or anything typed while the side conversation is open (`handleSideSend`). It never waits for the
    /// session's turn. A bare `/btw` is an error and sends nothing. The draft clears at once and comes back if the panel
    /// refuses it; with no panel yet, a toast says so and the draft stays.
    pub fn ask_side_question(&mut self, tx: &mut Tx, question: String) {
        let Some(owner) = self.draft_owner() else { return };
        let typed = self.draft.text.clone();
        let now = self.clock;
        self.drafts.push_history(&owner, typed.trim(), now);
        self.composer_error = None;
        if question.is_empty() {
            self.composer_error = Some(NO_QUESTION.into());
            return;
        }
        // The pick is the composer's agent, model and effort; the terminal lines join it with piece O.
        let pick = self.prompt_pick(None);
        match self.side_panel {
            SidePanel::Missing => self.toast(SIDE_MISSING.into(), None, 4.0, tx),
            panel => {
                self.draft.clear();
                self.side_asked.push(SideAsk { session: owner, question, pick });
                if panel == SidePanel::Refuses {
                    if self.draft.text.is_empty() {
                        self.draft = rune::TextEdit::new(typed);
                    }
                    self.composer_error = Some("The side question couldn't be asked.".into());
                }
                tx.focus("composer", false);
            }
        }
    }

    /// Each side keeps its own draft (row 9.6): when the side conversation opens or minimizes, the text area's text is put away
    /// (under `side-main` or `side.<id>`) and the other comes back. Only a change of mode of one and the same side counts.
    pub fn keep_side_drafts(&mut self, owner: &str, now: f64) {
        let current = self.side_id.clone().map(|id| (id, self.side_open));
        let seen = std::mem::replace(&mut self.side_seen, current.clone());
        let (Some((was_id, was_open)), Some((id, open))) = (seen, current) else { return };
        if was_id != id || was_open == open {
            return;
        }
        use crate::composer::drafts::{side_draft_key, side_main_draft_key};
        let (away, back) = if open {
            (side_main_draft_key(owner), side_draft_key(owner, &id))
        } else {
            (side_draft_key(owner, &id), side_main_draft_key(owner))
        };
        let text = self.draft.text.clone();
        self.drafts.write(&away, &text, now);
        let brought = self.drafts.read(&back).unwrap_or("").to_string();
        self.drafts.write(&back, "", now);
        self.set_draft_text(&brought);
        // The text area now holds this text under the session's own key too.
    }
}
