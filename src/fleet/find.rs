//! Find in this conversation on `State`: the units the search reads, the hits, the background paging of older history while
//! the bar is open, the Ctrl/Cmd+F key and the jump to a match. The decisions are `conv::find`; the bar is `view::find`.
//! Ports the wiring of `ActivityStream.vue` (units, reveal) and `use-conversation-find.ts`.
use crate::Rail;
use crate::State;
use crate::conv::find::{self, FindUnit, Focus, Paging, Press, Scope, Seen};
use crate::conv::fold::toggle_turn;
use crate::conv::output::parse_tool_output_cached;
use crate::view::conversation::{Part, Role};
use rune::{KeyEvent, Tx};

/// The output of each of a message's calls, as the card draws it.
fn tool_text(m: &crate::view::conversation::Message) -> Vec<String> {
    m.parts
        .iter()
        .filter_map(|p| match p {
            Part::Tool { body, .. } if !body.output.is_empty() => Some(parse_tool_output_cached(&body.output).plain.clone()),
            _ => None,
        })
        .collect()
}

impl State {
    /// One unit per message, oldest first (`ActivityStream.vue`): the text the message shows (its text parts joined by a
    /// blank line; thinking is never in it) and the output its calls hold. Folded turns count, as do rows that aren't built.
    pub fn find_units(&self, tools: bool) -> Vec<FindUnit> {
        let turns = &self.fold_plan.turns;
        self.session()
            .messages
            .iter()
            .filter(|m| m.role != Role::Shell)
            .map(|m| FindUnit {
                id: m.id.clone(),
                turn_id: turns.get(&m.id).cloned(),
                text: m.text(),
                // What the card draws (terminal codes gone), so a count agrees with the marks piece Q will draw; Fleet counts
                // the raw string. Only read when the chip asks for it: parsing every output isn't free.
                tool_text: if tools { tool_text(m) } else { Vec::new() },
            })
            .collect()
    }

    /// A number that moves when a message does: what the hits were worked out from.
    fn find_stamp(&self) -> (usize, usize) {
        let messages = &self.session().messages;
        (messages.len(), messages.iter().map(|m| m.version()).fold(0, |a, (n, w)| a.wrapping_add(n).wrapping_add(w)))
    }

    /// The conversation can take Ctrl/Cmd+F: the sessions view is showing one.
    fn find_visible(&self) -> bool {
        self.rail == Rail::Sessions && !self.gallery && self.has_session()
    }

    /// Where focus is, for the key's rules.
    fn find_focus(&self, focused: Option<&str>) -> Focus {
        if self.ui.open.is_some() || self.images.lightbox.is_some() {
            return Focus::Dialog;
        }
        find::focus_of_key(focused)
    }

    /// The key, from the window's `on_key`. True when it took it.
    pub fn find_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !find::is_find_shortcut(ev, cfg!(target_os = "macos")) {
            return false;
        }
        let focus = self.find_focus(tx.focused());
        match find::press(self.find.open, focus, true, self.find_visible()) {
            Press::Ignore => false,
            Press::Open => {
                self.find_open(tx);
                true
            }
            Press::SelectField => {
                self.find_select_field();
                true
            }
        }
    }

    /// `requestConversationFind`: a menu's Edit → Find in Conversation asks the conversation on screen to open its bar.
    pub fn find_request(&mut self, tx: &mut Tx) {
        if self.find_visible() {
            self.find_open(tx);
        }
    }

    fn find_select_field(&mut self) {
        let end = self.find.query.text.len();
        self.find.query.set_selection(0, end);
    }

    /// `open()`: the field takes focus with its text selected, and older history starts to load.
    pub fn find_open(&mut self, tx: &mut Tx) {
        self.find.open();
        self.find.at = self.open;
        self.find_sync();
        self.find_select_field();
        tx.focus(find::FIELD, false);
    }

    /// `close()`: no hits, the composer has focus again.
    pub fn find_close(&mut self, tx: &mut Tx) {
        if !self.find.open {
            return;
        }
        self.find.close();
        tx.focus("composer", false);
    }

    /// The field's text changed: the search starts at the first match.
    pub fn find_query_changed(&mut self, tx: &mut Tx) {
        self.find.current = 0;
        self.find_sync();
        self.find_jump(tx);
    }

    pub fn find_set_scope(&mut self, scope: Scope, tx: &mut Tx) {
        self.find.scope = scope;
        self.find.current = 0;
        self.find_sync();
        self.find_jump(tx);
    }

    /// The "+ Tool output" chip toggles; "Messages" always sets its own.
    pub fn find_toggle_tools(&mut self, tx: &mut Tx) {
        let next = if self.find.scope == Scope::Tools { Scope::Messages } else { Scope::Tools };
        self.find_set_scope(next, tx);
    }

    /// Next (1) or previous (-1) match, wrapping.
    pub fn find_step(&mut self, direction: i32, tx: &mut Tx) {
        if self.find.step(direction).is_some() {
            self.find_jump(tx);
        }
    }

    /// Stop: no more older pages for the search, until the bar is closed and opened again.
    pub fn find_stop_older(&mut self) {
        self.find.paging_stopped = true;
    }

    /// Try again: asks for the page that failed; the paging carries on from there.
    pub fn find_retry_older(&mut self) {
        self.load_older();
    }

    /// What the bar's second row says of older history.
    pub fn find_paging(&self) -> Paging {
        let (has_more, failed) = self.stream.as_ref().map_or((false, false), |s| (s.has_more, s.load_older_failed));
        self.find.older(has_more, failed)
    }

    /// `jump()`: the current match's turn opens if it was folded, and the message is brought into view on the next frame,
    /// once the list has built its row.
    fn find_jump(&mut self, tx: &mut Tx) {
        let Some(hit) = self.find.current_hit().cloned() else { return };
        if let Some(turn) = hit.turn_id.filter(|t| !self.turns_open.contains(t)) {
            toggle_turn(&mut self.turns_open, &turn);
            self.refold();
        }
        self.find.jump = Some(hit.unit_id);
        tx.wake_after(std::time::Duration::from_millis(16));
    }

    /// Works the hits out again when the query, the scope or a message changed. True when they did.
    fn find_sync(&mut self) -> bool {
        if !self.find.open {
            return false;
        }
        let seen = Seen { query: self.find.query.text.clone(), scope: self.find.scope, stamp: self.find_stamp() };
        if self.find.seen.as_ref() == Some(&seen) {
            return false;
        }
        let had = !self.find.hits.is_empty();
        let hits = find::find_hits(&self.find_units(seen.scope == Scope::Tools), &seen.query, seen.scope);
        let has = !hits.is_empty();
        self.find.set_hits(hits);
        self.find.seen = Some(seen);
        // The first match arriving (a page, a streamed word) is jumped to as a typed query is.
        if has && !had {
            self.find.jump = self.find.current_hit().map(|h| h.unit_id.clone());
        }
        true
    }

    /// From `poll`: closes on another session, follows the messages, pages older history one page after another while the
    /// bar is open, and brings the match into view. True when the bar must redraw.
    pub fn find_poll(&mut self, tx: &mut Tx) -> bool {
        if !self.find.open {
            return false;
        }
        if self.find.at != self.open {
            self.find.close();
            return true;
        }
        let mut changed = self.find_sync();
        if self.find_paging() == Paging::Searching && self.stream.as_ref().is_some_and(|s| !s.is_loading_older) {
            self.load_older();
            changed |= self.stream.as_ref().is_some_and(|s| s.is_loading_older);
        }
        if let Some(id) = self.find.jump.take() {
            changed |= self.show_message(Some(&id), None, tx);
        }
        changed
    }
}

impl State {
    /// A scene: the bar open on the open session with a query, as typed (`case` says which of its states).
    pub fn show_find(&mut self, query: &str, scope: Scope, current: usize, older: Option<crate::view::conversation::Older>) {
        self.find.open();
        self.find.at = self.open;
        self.find.query = rune::TextEdit::new(query);
        self.find.scope = scope;
        self.find_sync();
        self.find.current = current.min(self.find.hits.len().saturating_sub(1));
        self.find.query.focused = true;
        if let Some(older) = older {
            let stream = self.stream.get_or_insert_with(|| crate::fleet::reducer::create_state(&serde_json::json!({"messages": []})));
            stream.has_more = true;
            stream.cursor = Some("scene-cursor".into());
            stream.is_loading_older = older == crate::view::conversation::Older::Loading;
            stream.load_older_failed = older == crate::view::conversation::Older::Failed;
        }
    }
}
