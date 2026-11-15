//! Find in this conversation on `State`: the units the search reads, the hits, the background paging of older history while
//! the bar is open, the Ctrl/Cmd+F key and the jump to a match. The decisions are `conv::find`; the bar is `view::find`.
//! Ports the wiring of `ActivityStream.vue` (units, reveal) and `use-conversation-find.ts`.
use crate::Rail;
use crate::State;
use crate::conv::attached::parse_attached_note;
use crate::conv::bubble::looks_like_markdown;
use crate::conv::card::Toggle;
use crate::conv::find::{self, FindUnit, Focus, HitKind, Marks, Paging, Press, Scope, Seen, plain_ranges};
use crate::conv::fold::toggle_turn;
use crate::conv::output::COLLAPSED_LINES;
use crate::conv::output::parse_tool_output_cached;
use crate::view::conversation::{Message, Part, Role};
use rune::{KeyEvent, Tx};
use rune_kit::markdown::{self, Case};

/// The key of the current mark, which the scroll to a match aims at.
pub const MARK: &str = "find-current";

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

/// A text part as the row draws it: markdown, or plain words (a user's, when they don't look like markdown).
pub enum Drawn {
    Markdown(String),
    Plain(String),
}

/// What a text part draws as text, if anything: the marks and the count of drawn matches read the same words as the view.
pub fn drawn_text(msg: &Message, body: &str) -> Option<Drawn> {
    match msg.role {
        Role::Assistant => Some(Drawn::Markdown(body.to_string())),
        _ if msg.meta.background.is_some() => Some(Drawn::Markdown(body.to_string())),
        // A command's words are its own line, not text of the bubble.
        _ if msg.meta.command.is_some() => None,
        _ => {
            let typed = parse_attached_note(body).0;
            if typed.trim().is_empty() {
                None
            } else if looks_like_markdown(&typed) {
                Some(Drawn::Markdown(typed))
            } else {
                Some(Drawn::Plain(typed))
            }
        }
    }
}

/// Every drawn block of a message that can hold a match, as `(part id, its matches)`: text first, then the output of the
/// calls (with the "+ Tool output" chip), which is the order the hits number them in.
fn blocks_of(msg: &Message, query: &str, tools: bool) -> Vec<(String, usize)> {
    let texts = msg.parts.iter().filter_map(|p| match p {
        Part::Text { id, text } => {
            let n = match drawn_text(msg, text)? {
                Drawn::Markdown(md) => markdown::find(&md, query, Case::Insensitive).len(),
                Drawn::Plain(plain) => plain_ranges(&plain, query).len(),
            };
            Some((id.clone(), n))
        }
        _ => None,
    });
    let outputs = msg.parts.iter().filter_map(|p| match p {
        Part::Tool { id, body, .. } if tools && !body.output.is_empty() => {
            Some((id.clone(), plain_ranges(&parse_tool_output_cached(&body.output).plain, query).len()))
        }
        _ => None,
    });
    texts.chain(outputs).collect()
}

impl State {
    /// The block the current match is in, and which of its matches: `(part id, index)`.
    fn find_target(&self) -> Option<(String, usize)> {
        let hit = self.find.current_hit()?;
        let msg = self.session().messages.iter().find(|m| m.id == hit.unit_id)?;
        let blocks = blocks_of(msg, &self.find.query.text, self.find.scope == Scope::Tools);
        let (block, at) = find::locate(&blocks.iter().map(|b| b.1).collect::<Vec<_>>(), hit.ordinal)?;
        Some((blocks[block].0.clone(), at))
    }

    /// What the block of this part marks: every match, the current one apart. `None` with the bar closed or no query, and
    /// for a call's output unless the chip asks for tool output (the count leaves it out, so the marks do).
    pub fn find_marks(&self, part_id: &str, tool: bool) -> Option<Marks> {
        if !self.find.open || !self.find.has_query() || (tool && self.find.scope != Scope::Tools) {
            return None;
        }
        let current = self.find_target().filter(|(part, _)| part == part_id).map(|(_, at)| at);
        Some(Marks { query: self.find.query.text.clone(), current })
    }

    /// What a row's marks were drawn from, for its memo: the query, the chip, and the current match when it is in this row.
    pub fn find_row_key(&self, msg_id: &str) -> Option<(String, bool, Option<usize>)> {
        if !self.find.open || !self.find.has_query() {
            return None;
        }
        let current = self.find.current_hit().filter(|h| h.unit_id == msg_id).map(|h| h.ordinal);
        Some((self.find.query.text.clone(), self.find.scope == Scope::Tools, current))
    }

    /// The match is in a call's output: its card opens, and shows the lines the match is in.
    fn find_open_card(&mut self) {
        if self.find.current_hit().is_none_or(|h| h.kind != HitKind::Tool) {
            return;
        }
        let Some((part_id, at)) = self.find_target() else { return };
        let query = self.find.query.text.clone();
        let Some(hit) = self.find.current_hit() else { return };
        let Some(Part::Tool { body, .. }) = self
            .session()
            .messages
            .iter()
            .find(|m| m.id == hit.unit_id)
            .and_then(|m| m.parts.iter().find(|p| matches!(p, Part::Tool { id, .. } if *id == part_id)))
        else {
            return;
        };
        let (under, parsed) = (body.initially_collapsed, parse_tool_output_cached(&body.output));
        let line = plain_ranges(&parsed.plain, &query).get(at).map(|r| parsed.plain[..r.start].matches('\n').count());
        let hidden = line.is_some_and(|l| l + COLLAPSED_LINES < parsed.lines.len());
        self.tool_open.insert(part_id.clone(), Toggle { open: true, under });
        if hidden {
            self.tool_wide.insert(part_id);
        }
    }

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
        self.find.at = self.open_at();
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
        self.find_open_card();
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
        if self.find.at != self.open_at() {
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
            // The message is brought to the middle first; the mark is brought there once the row exists.
            self.find.settle = Some(self.now_ms() + 250.0);
            tx.wake_after(std::time::Duration::from_millis(260));
        }
        if self.find.settle.is_some_and(|at| self.now_ms() >= at) {
            self.find.settle = None;
            tx.scroll_center(MARK);
            changed = true;
        }
        changed
    }
}

impl State {
    /// A scene: the bar open on the open session with a query, as typed (`case` says which of its states).
    pub fn show_find(&mut self, query: &str, scope: Scope, current: usize, older: Option<crate::view::conversation::Older>) {
        self.find.open();
        self.find.at = self.open_at();
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

impl State {
    /// A scene: find's marks over a conversation whose first turn folds. `open` opens that turn, as a jump to a match in it does.
    pub fn show_find_marks(&mut self, current: usize, open: bool) {
        self.show_conversation(crate::mock::find::of_marks());
        if open {
            toggle_turn(&mut self.turns_open, "u1");
        }
        self.refold();
        self.show_find("entry", Scope::Messages, current, None);
    }
}
