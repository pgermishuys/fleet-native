//! The conversation's scroll behaviour on `State`: what `use-stream-scroll.ts` and `ActivityStream.vue` do with the
//! scroll box. The decisions are `conv::pin`; this reads where Rune says the box is, asks for older messages near the
//! top, and answers the session commands (scroll to top, to bottom, show a message).
use crate::State;
use crate::conv::pin::{Edges, Shown};
use crate::view::conversation::{Older, Role};
use rune::Tx;
use std::time::Duration;

/// The scroll box's key, for the requests the scroll commands make.
pub const STREAM: &str = "stream";

impl State {
    pub(crate) fn now_ms(&self) -> f64 {
        self.clock * 1000.0
    }

    /// Follow the end again and put the box there: a prompt was sent, another session opened, or Jump to latest was
    /// pressed (`pinToBottom`, `handleJumpToLatest`).
    pub fn follow_end(&mut self, tx: &mut Tx) {
        let now = self.now_ms();
        self.pin.follow(now);
        tx.scroll_to_bottom(STREAM);
    }

    /// `handleJumpToLatest`: instant, as Fleet's is.
    pub fn jump_to_latest(&mut self, tx: &mut Tx) {
        self.pin.pin_to_bottom();
        tx.scroll_to_bottom_instant(STREAM);
    }

    /// `scroll-top` (`scrollToTop`, smooth). The view leaves the end, so the next move up is the pill's to show.
    pub fn scroll_top(&mut self, tx: &mut Tx) {
        tx.scroll_to(STREAM, 0.0, 0.0);
    }

    /// The box moved (or its content did): feed the pin its edges, and ask for older messages near the top
    /// (`updatePinnedState`). Returns whether the view must redraw. Reads what `on_scroll_offset` left, since that
    /// handler may not change state.
    pub fn keep_scroll(&mut self, tx: &mut Tx) -> bool {
        let now = self.now_ms();
        let mut changed = false;
        if let Some(shown) = &self.shown {
            if shown.active(now) {
                tx.wake_after(Duration::from_millis(100));
            } else {
                self.shown = None;
                changed = true;
            }
        }
        let Some((at, layout)) = self.scroll_probe.get() else { return changed };
        // `poll` runs before the frame's layout, so the first reading after a snapshot is of the box before it held the
        // conversation (empty, or the session left). The frame that draws the new view reports the box again.
        if layout != self.scroll_layout {
            return changed;
        }
        let before = self.scroll_seen.replace(at);
        if before == Some(at) {
            return changed;
        }
        // Wheel and key input don't reach the app, so a move up that the content didn't cause stands for them: the
        // content shrinking is a layout shift (offset and end drop together), anything else is the reader.
        if let Some(b) = before
            && at.offset[1] < b.offset[1] - 1.0
            && at.max[1] >= b.max[1] - 1.0
        {
            self.pin.note_user_intent(now);
        }
        let shown_pill = self.pin.show_jump();
        let edges = Edges { top: at.offset[1], height: at.max[1] + at.viewport[1], viewport: at.viewport[1] };
        let update = self.pin.update(edges, now);
        if update.follow {
            tx.scroll_to_bottom_instant(STREAM);
        }
        // A failed ask waits for "Try again": the failure line moves the content, and asking again on that would loop on
        // a server that keeps refusing (Fleet asks again on the next scroll event).
        if update.load_older && self.older() != Some(Older::Failed) {
            self.load_older();
        }
        changed || shown_pill != self.pin.show_jump()
    }

    /// A new session opened (or its snapshot arrived): the box starts at the end, pinned (the `sessionId` watcher).
    pub fn reset_scroll(&mut self) {
        self.scroll_seen = None;
        self.shown = None;
    }

    /// The row of the message a `show-message` command names: a message id, or the id of a tool call it holds.
    pub fn message_for(&self, id: Option<&str>, call: Option<&str>) -> Option<String> {
        let session = self.session();
        if let Some(id) = id {
            return session.messages.iter().find(|m| m.id == id).map(|m| m.id.clone());
        }
        let call = call?;
        session
            .messages
            .iter()
            .find(|m| m.parts.iter().any(|p| matches!(p, crate::view::conversation::Part::Tool { call_id, .. } if call_id == call)))
            .map(|m| m.id.clone())
    }

    /// `showMessage`: stop following the end, bring the message to the middle and ring it for 1.6 s. A message that
    /// finished turns folded away has no row to bring; its fold row stays where it is. Returns whether it was found.
    pub fn show_message(&mut self, id: Option<&str>, call: Option<&str>, tx: &mut Tx) -> bool {
        let Some(message) = self.message_for(id, call) else { return false };
        if self.fold_plan.hidden.contains(&message) {
            return false;
        }
        self.pin.show_message();
        tx.scroll_center(message.clone());
        let now = self.now_ms();
        self.shown = Some(Shown::new(&message, now));
        tx.wake_after(Duration::from_millis(100));
        true
    }

    /// A scene: the open session's second message wears the ring of a jump, held for the picture.
    pub fn show_ring(&mut self) {
        if let Some(m) = self.session().messages.get(1) {
            self.shown = Some(Shown { id: m.id.clone(), until_ms: f64::INFINITY });
        }
    }

    /// Whether the message wears the ring of a jump.
    pub fn is_shown(&self, id: &str) -> bool {
        self.shown.as_ref().is_some_and(|s| s.id == id && s.active(self.now_ms()))
    }
}

impl State {
    /// `latestAnswerMessage` (row 1.89): the newest assistant message with a body, which the side conversation's tab
    /// shows while it is folded. Only once the snapshot has loaded: `progress` isn't emitted for a conversation that is
    /// still arriving.
    pub fn latest_answer(&self) -> Option<(&str, String)> {
        if self.loading {
            return None;
        }
        let m = self.session().messages.iter().rev().find(|m| m.role == Role::Assistant && !m.text().trim().is_empty())?;
        Some((m.id.as_str(), m.text()))
    }
}
